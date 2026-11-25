//! Integración con Linux: X11 y Wayland (gtk-layer-shell).
//!
//! En X11 funciona igual que en Windows: consulta `XQueryPointer`, recorta la región de entrada
//! con `input_shape_combine_region` y mantiene la ventana mapeada sin aceptar foco.
//! En Wayland con soporte de `wlr-layer-shell` (Hyprland, Sway, Niri, KDE), carga
//! `libgtk-layer-shell.so.0` dinámicamente en tiempo de ejecución (sin requerirla al enlazar ni en X11)
//! y crea una superficie layer-shell invisible para la zona de activación junto con la rueda en `LAYER_OVERLAY`.

use std::ffi::{c_char, c_int, c_uint, c_void, CString};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use gtk::cairo::{RectangleInt, Region};
use gtk::gdk;
use gtk::gdk::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use tauri::{AppHandle, PhysicalPosition, PhysicalSize, WebviewWindow};

use crate::geometry::{half_disc_spans, ZoneGeo};

static NATIVE_IN_ZONE: AtomicBool = AtomicBool::new(false);
static NATIVE_IN_WHEEL: AtomicBool = AtomicBool::new(false);
static IS_WAYLAND: OnceLock<bool> = OnceLock::new();
static LAYER_SUPPORTED: OnceLock<bool> = OnceLock::new();
/// Factor de escala del monitor de la zona (bits de un `f64`), para pasar coordenadas lógicas
/// de Hyprland al espacio físico de `ZoneGeo`. Vale 0 hasta la primera geometría.
static ZONE_SCALE: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------- gtk-layer-shell dinámico

#[allow(non_camel_case_types)]
mod layer_shell {
    use super::*;

    pub const LAYER_OVERLAY: c_int = 3;
    pub const EDGE_LEFT: c_int = 0;
    pub const EDGE_TOP: c_int = 2;
    pub const KEYBOARD_MODE_NONE: c_int = 0;

    pub struct Api {
        is_supported: unsafe extern "C" fn() -> c_int,
        init_for_window: unsafe extern "C" fn(*mut c_void),
        set_namespace: unsafe extern "C" fn(*mut c_void, *const c_char),
        set_layer: unsafe extern "C" fn(*mut c_void, c_int),
        set_monitor: unsafe extern "C" fn(*mut c_void, *mut c_void),
        set_anchor: unsafe extern "C" fn(*mut c_void, c_int, c_int),
        set_margin: unsafe extern "C" fn(*mut c_void, c_int, c_int),
        set_keyboard_mode: unsafe extern "C" fn(*mut c_void, c_int),
        set_exclusive_zone: unsafe extern "C" fn(*mut c_void, c_int),
    }

    unsafe impl Send for Api {}
    unsafe impl Sync for Api {}

    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum Unavailable {
        /// No se encontró `libgtk-layer-shell` (o le faltan símbolos).
        MissingLibrary,
        /// La librería está, pero el compositor no implementa `wlr-layer-shell` (p. ej. GNOME).
        Unsupported,
    }

    static API: OnceLock<Result<Api, Unavailable>> = OnceLock::new();

    unsafe fn sym<T>(handle: *mut c_void, name: &[u8]) -> Option<T> {
        let ptr = libc::dlsym(handle, name.as_ptr() as *const c_char);
        if ptr.is_null() {
            None
        } else {
            Some(std::mem::transmute_copy(&ptr))
        }
    }

    /// Rutas a probar con `dlopen`. Dentro de un AppImage la librería va empaquetada en
    /// `$APPDIR/usr/lib` (ver `tauri.linux.conf.json`) y se prueba antes que la del sistema.
    fn candidates() -> Vec<CString> {
        let mut out = Vec::new();
        if let Some(appdir) = std::env::var_os("APPDIR") {
            let p = PathBuf::from(appdir).join("usr/lib/libgtk-layer-shell.so.0");
            if let Ok(c) = CString::new(p.as_os_str().as_encoded_bytes()) {
                out.push(c);
            }
        }
        out.push(c"libgtk-layer-shell.so.0".to_owned());
        out.push(c"libgtk-layer-shell.so".to_owned());
        out
    }

    pub fn load() -> &'static Result<Api, Unavailable> {
        API.get_or_init(|| {
            let mut handle = std::ptr::null_mut();
            for lib in candidates() {
                handle = unsafe { libc::dlopen(lib.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL) };
                if !handle.is_null() {
                    break;
                }
            }
            if handle.is_null() {
                return Err(Unavailable::MissingLibrary);
            }
            unsafe {
                let api = (|| {
                    Some(Api {
                        // La API pública de gtk-layer-shell usa el prefijo `gtk_layer_`.
                        is_supported: sym(handle, b"gtk_layer_is_supported\0")?,
                        init_for_window: sym(handle, b"gtk_layer_init_for_window\0")?,
                        set_namespace: sym(handle, b"gtk_layer_set_namespace\0")?,
                        set_layer: sym(handle, b"gtk_layer_set_layer\0")?,
                        set_monitor: sym(handle, b"gtk_layer_set_monitor\0")?,
                        set_anchor: sym(handle, b"gtk_layer_set_anchor\0")?,
                        set_margin: sym(handle, b"gtk_layer_set_margin\0")?,
                        set_keyboard_mode: sym(handle, b"gtk_layer_set_keyboard_mode\0")?,
                        set_exclusive_zone: sym(handle, b"gtk_layer_set_exclusive_zone\0")?,
                    })
                })()
                .ok_or(Unavailable::MissingLibrary)?;
                if (api.is_supported)() == 0 {
                    return Err(Unavailable::Unsupported);
                }
                Ok(api)
            }
        })
    }

    pub fn get() -> Option<&'static Api> {
        load().as_ref().ok()
    }

    pub fn init_overlay(api: &Api, win: &impl IsA<gtk::Window>, namespace: &str) {
        let ptr = win.as_ref().as_ptr() as *mut c_void;
        let ns = CString::new(namespace).unwrap_or_default();
        unsafe {
            (api.init_for_window)(ptr);
            (api.set_namespace)(ptr, ns.as_ptr());
            (api.set_layer)(ptr, LAYER_OVERLAY);
            (api.set_keyboard_mode)(ptr, KEYBOARD_MODE_NONE);
            (api.set_exclusive_zone)(ptr, -1);
            (api.set_anchor)(ptr, EDGE_LEFT, 1);
            (api.set_anchor)(ptr, EDGE_TOP, 1);
        }
    }

    pub fn position(api: &Api, win: &impl IsA<gtk::Window>, mon: Option<&gdk::Monitor>, x: i32, y: i32) {
        let ptr = win.as_ref().as_ptr() as *mut c_void;
        let mon_ptr = mon.map(|m| m.as_ptr() as *mut c_void).unwrap_or(std::ptr::null_mut());
        unsafe {
            (api.set_monitor)(ptr, mon_ptr);
            (api.set_margin)(ptr, EDGE_LEFT, x);
            (api.set_margin)(ptr, EDGE_TOP, y);
        }
    }
}

fn is_wayland() -> bool {
    IS_WAYLAND.get().copied().unwrap_or(false)
}

fn layer_api() -> Option<&'static layer_shell::Api> {
    if is_wayland() && LAYER_SUPPORTED.get().copied().unwrap_or(false) {
        layer_shell::get()
    } else {
        None
    }
}

pub fn uses_native_trigger() -> bool {
    layer_api().is_some()
}

pub fn native_hover_state() -> (bool, bool) {
    (NATIVE_IN_ZONE.load(Ordering::Relaxed), NATIVE_IN_WHEEL.load(Ordering::Relaxed))
}

fn monitor_for_geo(g: &ZoneGeo) -> Option<gdk::Monitor> {
    let display = gdk::Display::default()?;
    let n = display.n_monitors();
    let sf = if g.sf > 0.0 { g.sf } else { 1.0 };
    let target_x = (g.mon_x as f64 / sf).round() as i32;
    let target_y = (g.mon_y as f64 / sf).round() as i32;
    let mut best = None;
    let mut best_dist = i64::MAX;
    for i in 0..n {
        if let Some(m) = display.monitor(i) {
            let rect = m.geometry();
            let dx = (rect.x() - target_x) as i64;
            let dy = (rect.y() - target_y) as i64;
            let dist = dx * dx + dy * dy;
            if dist < best_dist {
                best_dist = dist;
                best = Some(m);
            }
        }
    }
    best
}

fn apply_wheel_region(gtk_win: &gtk::ApplicationWindow, rgn: Option<&Region>) {
    let empty;
    let region = match rgn {
        Some(r) => r,
        None => {
            empty = Region::create();
            &empty
        }
    };
    gtk_win.input_shape_combine_region(Some(region));
    if let Some(gw) = gtk_win.window() {
        gw.input_shape_combine_region(region, 0, 0);
    }
}

/// En X11 sin compositor (i3/Openbox sin picom…) no hay transparencia real: el fondo de la
/// webview se pinta negro y opaco. Se consulta en cada uso porque el compositor puede
/// arrancarse o pararse con Orbe en marcha.
fn is_composited(win: &impl IsA<gtk::Window>) -> bool {
    GtkWindowExt::screen(win.as_ref()).is_some_and(|s| s.is_composited())
}

/// Recorte visual (XShape bounding) para que, sin compositor, al menos se vea el semicírculo
/// y no un cuadrado negro. Con compositor se quita para no cortar el antialiasing del borde.
fn apply_visual_shape(gtk_win: &gtk::ApplicationWindow, rgn: Option<&Region>) {
    gtk_win.shape_combine_region(rgn);
}

/// Aviso visible para el usuario (stderr + notificación de escritorio si hay `notify-send`).
fn warn_user(msg: &str) {
    eprintln!("[orbe] {msg}");
    if let Ok(mut child) = Command::new("notify-send")
        .args(["-a", "Orbe", "-u", "normal", "Orbe", msg])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

fn warn_if_unsupported_wayland() {
    if !is_wayland() || layer_api().is_some() {
        return;
    }
    let msg = match layer_shell::load() {
        Err(layer_shell::Unavailable::MissingLibrary) => {
            "Falta libgtk-layer-shell: la rueda no puede colocarse ni detectar el cursor en Wayland. \
             Instala libgtk-layer-shell0 (Debian/Ubuntu) o gtk-layer-shell (Fedora/Arch) y reinicia Orbe."
        }
        _ if hyprland_socket_path().is_some() => {
            "Hyprland no ha aceptado la superficie layer-shell: la rueda puede aparecer mal colocada."
        }
        _ => {
            "Este escritorio Wayland no soporta wlr-layer-shell (GNOME, por ejemplo), así que Orbe \
             no puede detectar el cursor y la rueda no se abrirá. Usa una sesión X11 o un compositor \
             compatible (KDE, Hyprland, Sway, Niri)."
        }
    };
    warn_user(msg);
}

fn configure_non_activating(win: &impl IsA<gtk::Window>) {
    let w = win.as_ref();
    w.set_accept_focus(false);
    w.set_focus_on_map(false);
    w.set_keep_above(true);
    w.set_skip_taskbar_hint(true);
    w.set_skip_pager_hint(true);
    w.set_decorated(false);
    w.set_type_hint(gdk::WindowTypeHint::Dock);
    w.stick();
}

pub fn setup_wheel(w: &WebviewWindow) {
    let wayland = *IS_WAYLAND.get_or_init(|| {
        gdk::Display::default().is_some_and(|d| d.backend().is_wayland())
    });
    let _ = LAYER_SUPPORTED.get_or_init(|| wayland && layer_shell::get().is_some());

    let Ok(gtk_win) = w.gtk_window() else { return };
    configure_non_activating(&gtk_win);
    gtk_win.set_app_paintable(true);
    if let Some(screen) = GtkWindowExt::screen(&gtk_win) {
        if let Some(visual) = screen.rgba_visual() {
            gtk_win.set_visual(Some(&visual));
        }
    }

    gtk_win.add_events(gdk::EventMask::ENTER_NOTIFY_MASK | gdk::EventMask::LEAVE_NOTIFY_MASK);
    gtk_win.connect_enter_notify_event(|_, _| {
        NATIVE_IN_WHEEL.store(true, Ordering::Relaxed);
        glib::Propagation::Proceed
    });
    gtk_win.connect_leave_notify_event(|_, ev| {
        if ev.detail() != gdk::NotifyType::Inferior {
            NATIVE_IN_WHEEL.store(false, Ordering::Relaxed);
        }
        glib::Propagation::Proceed
    });

    if let Some(api) = layer_api() {
        layer_shell::init_overlay(api, &gtk_win, "orbe");
    }

    apply_wheel_region(&gtk_win, None);
    // En X11 con compositor la ventana queda mapeada (transparente y sin entrada), como en
    // Windows; sin compositor se vería negra, así que solo se mapea al abrir la rueda.
    if !is_wayland() && is_composited(&gtk_win) {
        gtk_win.show_all();
    }
    configure_non_activating(&gtk_win);
    warn_if_unsupported_wayland();
}

pub fn show_wheel(w: &WebviewWindow, g: &ZoneGeo) {
    let Ok(gtk_win) = w.gtk_window() else { return };
    let sf = if g.sf > 0.0 { g.sf } else { 1.0 };
    let size_logical = g.size_logical.round() as i32;
    let radius_logical = ((g.ps / sf) + 2.0).round() as i32;

    let rgn = Region::create();
    for (x, y, width) in half_disc_spans(size_logical, radius_logical, g.facing) {
        let _ = rgn.union_rectangle(&RectangleInt::new(x, y, width, 1));
    }

    if let Some(api) = layer_api() {
        let mon = monitor_for_geo(g);
        let rel_x = ((g.win_x - g.mon_x) as f64 / sf).round() as i32;
        let rel_y = ((g.win_y - g.mon_y) as f64 / sf).round() as i32;
        layer_shell::position(api, &gtk_win, mon.as_ref(), rel_x, rel_y);
        gtk_win.resize(size_logical, size_logical);
        gtk_win.show_all();
        apply_wheel_region(&gtk_win, Some(&rgn));
    } else {
        let _ = w.set_size(PhysicalSize::new(g.size_phys.max(1) as u32, g.size_phys.max(1) as u32));
        let _ = w.set_position(PhysicalPosition::new(g.win_x, g.win_y));
        if !is_wayland() {
            let composited = is_composited(&gtk_win);
            apply_visual_shape(&gtk_win, (!composited).then_some(&rgn));
        }
        gtk_win.show_all();
        apply_wheel_region(&gtk_win, Some(&rgn));
    }
    configure_non_activating(&gtk_win);
}

pub fn hide_wheel(w: &WebviewWindow) {
    NATIVE_IN_WHEEL.store(false, Ordering::Relaxed);
    if let Ok(gtk_win) = w.gtk_window() {
        apply_wheel_region(&gtk_win, None);
        if is_wayland() || !is_composited(&gtk_win) {
            gtk_win.hide();
        }
    }
}

pub fn sync_trigger_zone(app: &AppHandle, g: &ZoneGeo) {
    ZONE_SCALE.store(g.sf.to_bits(), Ordering::Relaxed);
    let Some(api) = layer_api() else { return };
    let g = *g;
    let _ = app.run_on_main_thread(move || {
        thread_local! {
            static TRIGGER_WIN: std::cell::RefCell<Option<gtk::Window>> = const { std::cell::RefCell::new(None) };
        }
        let win = TRIGGER_WIN.with(|cell| {
            let mut guard = cell.borrow_mut();
            if let Some(w) = guard.as_ref() {
                return w.clone();
            }
            let w = gtk::Window::new(gtk::WindowType::Toplevel);
            w.set_app_paintable(true);
            if let Some(screen) = GtkWindowExt::screen(&w) {
                if let Some(visual) = screen.rgba_visual() {
                    w.set_visual(Some(&visual));
                }
            }
            configure_non_activating(&w);
            w.add_events(gdk::EventMask::ENTER_NOTIFY_MASK | gdk::EventMask::LEAVE_NOTIFY_MASK);
            w.connect_draw(|_, cr| {
                cr.set_source_rgba(0.0, 0.0, 0.0, 0.0);
                cr.set_operator(gtk::cairo::Operator::Source);
                let _ = cr.paint();
                cr.set_operator(gtk::cairo::Operator::Over);
                glib::Propagation::Stop
            });
            w.connect_enter_notify_event(|_, _| {
                NATIVE_IN_ZONE.store(true, Ordering::Relaxed);
                glib::Propagation::Proceed
            });
            w.connect_leave_notify_event(|_, _| {
                NATIVE_IN_ZONE.store(false, Ordering::Relaxed);
                glib::Propagation::Proceed
            });
            layer_shell::init_overlay(api, &w, "orbe-zone");
            *guard = Some(w.clone());
            w
        });

        let sf = if g.sf > 0.0 { g.sf } else { 1.0 };
        let mon = monitor_for_geo(&g);
        let rel_x = ((g.zone_l - g.mon_x as f64) / sf).round() as i32;
        let rel_y = ((g.zone_t - g.mon_y as f64) / sf).round() as i32;
        let w_log = ((g.zone_r - g.zone_l + 1.0) / sf).round().max(1.0) as i32;
        let h_log = ((g.zone_b - g.zone_t + 1.0) / sf).round().max(1.0) as i32;
        layer_shell::position(api, &win, mon.as_ref(), rel_x, rel_y);
        win.resize(w_log, h_log);
        win.show_all();
    });
}

// ---------------------------------------------------------------- X11 (cursor y pantalla completa)

struct X11Conn {
    xlib: x11_dl::xlib::Xlib,
    display: *mut x11_dl::xlib::Display,
    root: x11_dl::xlib::Window,
    atom_active: x11_dl::xlib::Atom,
    atom_wm_state: x11_dl::xlib::Atom,
    atom_fullscreen: x11_dl::xlib::Atom,
    atom_wm_pid: x11_dl::xlib::Atom,
}

unsafe impl Send for X11Conn {}

static X11: OnceLock<Option<Mutex<X11Conn>>> = OnceLock::new();

unsafe extern "C" fn ignore_x11_error(
    _: *mut x11_dl::xlib::Display,
    _: *mut x11_dl::xlib::XErrorEvent,
) -> c_int {
    0
}

fn with_x11<T>(f: impl FnOnce(&X11Conn) -> Option<T>) -> Option<T> {
    if is_wayland() {
        return None;
    }
    let mtx = X11
        .get_or_init(|| unsafe {
            let xlib = x11_dl::xlib::Xlib::open().ok()?;
            (xlib.XSetErrorHandler)(Some(ignore_x11_error));
            let display = (xlib.XOpenDisplay)(std::ptr::null());
            if display.is_null() {
                return None;
            }
            let root = (xlib.XDefaultRootWindow)(display);
            let intern = |name: &[u8]| (xlib.XInternAtom)(display, name.as_ptr() as *const c_char, 0);
            Some(Mutex::new(X11Conn {
                atom_active: intern(b"_NET_ACTIVE_WINDOW\0"),
                atom_wm_state: intern(b"_NET_WM_STATE\0"),
                atom_fullscreen: intern(b"_NET_WM_STATE_FULLSCREEN\0"),
                atom_wm_pid: intern(b"_NET_WM_PID\0"),
                xlib,
                display,
                root,
            }))
        })
        .as_ref()?;
    let guard = mtx.lock().ok()?;
    f(&guard)
}

unsafe fn x11_get_atoms(
    c: &X11Conn,
    win: x11_dl::xlib::Window,
    prop: x11_dl::xlib::Atom,
    req_type: x11_dl::xlib::Atom,
) -> Vec<u64> {
    let mut actual_type = 0;
    let mut actual_format = 0;
    let mut nitems = 0;
    let mut bytes_after = 0;
    let mut data: *mut u8 = std::ptr::null_mut();
    let status = (c.xlib.XGetWindowProperty)(
        c.display,
        win,
        prop,
        0,
        1024,
        0,
        req_type,
        &mut actual_type,
        &mut actual_format,
        &mut nitems,
        &mut bytes_after,
        &mut data,
    );
    if status != 0 || data.is_null() || actual_format != 32 {
        if !data.is_null() {
            (c.xlib.XFree)(data as *mut c_void);
        }
        return Vec::new();
    }
    let slice = std::slice::from_raw_parts(data as *const std::ffi::c_ulong, nitems as usize);
    let out: Vec<u64> = slice.iter().map(|&v| v as u64).collect();
    (c.xlib.XFree)(data as *mut c_void);
    out
}

pub fn cursor_pos() -> Option<(i32, i32)> {
    if let Some(pos) = hyprland_cursor_pos() {
        return Some(pos);
    }
    with_x11(|c| unsafe {
        let mut root_ret = 0;
        let mut child_ret = 0;
        let mut root_x: c_int = 0;
        let mut root_y: c_int = 0;
        let mut win_x: c_int = 0;
        let mut win_y: c_int = 0;
        let mut mask: c_uint = 0;
        let ok = (c.xlib.XQueryPointer)(
            c.display,
            c.root,
            &mut root_ret,
            &mut child_ret,
            &mut root_x,
            &mut root_y,
            &mut win_x,
            &mut win_y,
            &mut mask,
        );
        (ok != 0).then_some((root_x, root_y))
    })
}

pub fn foreground_is_fullscreen() -> bool {
    if let Some(fs) = hyprland_is_fullscreen() {
        return fs;
    }
    with_x11(|c| unsafe {
        let active = x11_get_atoms(c, c.root, c.atom_active, x11_dl::xlib::XA_WINDOW);
        let win = *active.first()? as x11_dl::xlib::Window;
        if win == 0 || win == c.root {
            return Some(false);
        }
        let pids = x11_get_atoms(c, win, c.atom_wm_pid, x11_dl::xlib::XA_CARDINAL);
        if pids.first().is_some_and(|&pid| pid == std::process::id() as u64) {
            return Some(false);
        }
        let states = x11_get_atoms(c, win, c.atom_wm_state, x11_dl::xlib::XA_ATOM);
        Some(states.contains(&(c.atom_fullscreen as u64)))
    })
    .unwrap_or(false)
}

// ---------------------------------------------------------------- Hyprland IPC opcional

fn hyprland_socket_path() -> Option<PathBuf> {
    let sig = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    let rt = std::env::var_os("XDG_RUNTIME_DIR")?;
    let p = PathBuf::from(rt).join("hypr").join(sig).join(".socket.sock");
    p.exists().then_some(p)
}

fn hyprland_query(cmd: &str) -> Option<String> {
    let path = hyprland_socket_path()?;
    let mut stream = UnixStream::connect(path).ok()?;
    stream.write_all(cmd.as_bytes()).ok()?;
    let mut out = String::new();
    stream.read_to_string(&mut out).ok()?;
    Some(out)
}

/// `cursorpos` de Hyprland va en coordenadas lógicas del layout. `ZoneGeo` está en el espacio
/// físico de Tauri, que en Wayland es la geometría lógica de GDK multiplicada por la escala de
/// cada monitor; para puntos del monitor de la zona basta con multiplicar por su escala.
fn parse_hyprland_cursor(resp: &str, scale: f64) -> Option<(i32, i32)> {
    let (x, y) = resp.trim().split_once(',')?;
    let (x, y): (f64, f64) = (x.trim().parse().ok()?, y.trim().parse().ok()?);
    let sf = if scale > 0.0 { scale } else { 1.0 };
    Some(((x * sf).round() as i32, (y * sf).round() as i32))
}

fn hyprland_cursor_pos() -> Option<(i32, i32)> {
    let resp = hyprland_query("cursorpos")?;
    parse_hyprland_cursor(&resp, f64::from_bits(ZONE_SCALE.load(Ordering::Relaxed)))
}

/// `fullscreen` es un entero (0–3) en Hyprland ≥ 0.42 y un booleano en versiones anteriores.
fn parse_hyprland_fullscreen(resp: &str, own_pid: u64) -> Option<bool> {
    let v: serde_json::Value = serde_json::from_str(resp).ok()?;
    let pid = v.get("pid").and_then(|p| p.as_u64()).unwrap_or(0);
    if pid == own_pid {
        return Some(false);
    }
    Some(match v.get("fullscreen") {
        Some(serde_json::Value::Bool(b)) => *b,
        Some(f) => f.as_i64().unwrap_or(0) != 0,
        None => false,
    })
}

fn hyprland_is_fullscreen() -> Option<bool> {
    let resp = hyprland_query("j/activewindow")?;
    parse_hyprland_fullscreen(&resp, std::process::id() as u64)
}

// ---------------------------------------------------------------- bloqueo de sesión

#[derive(Clone, Copy)]
enum LockKind {
    /// Termina enseguida y su código de salida indica si ha funcionado.
    Quick,
    /// Bloqueador que puede quedarse en primer plano hasta desbloquear (hyprlock, swaylock sin
    /// `-f`, slock…): no se espera a que acabe.
    Locker,
}

const FULL_DESKTOPS: &[&str] =
    &["gnome", "kde", "plasma", "xfce", "cinnamon", "mate", "unity", "budgie", "lxqt", "pantheon", "deepin"];

/// Orden de bloqueadores según la sesión. `loginctl lock-session` devuelve éxito aunque nadie
/// escuche la señal (Hyprland/Sway sin hypridle/swayidle, i3…), así que solo va primero en
/// escritorios completos, que sí la atienden.
fn lock_candidates(desktop: &str, wayland: bool, hyprland: bool) -> Vec<(&'static str, &'static [&'static str], LockKind)> {
    use LockKind::*;
    let desktop = desktop.to_ascii_lowercase();
    let full_de = FULL_DESKTOPS.iter().any(|d| desktop.contains(d));
    let mut out: Vec<(&'static str, &'static [&'static str], LockKind)> = Vec::new();
    if full_de {
        out.push(("loginctl", &["lock-session"], Quick));
        out.push(("xdg-screensaver", &["lock"], Quick));
        out.push(("gnome-screensaver-command", &["-l"], Quick));
        return out;
    }
    if hyprland {
        out.push(("hyprlock", &[], Locker));
        out.push(("swaylock", &["-f"], Locker));
    } else if wayland {
        out.push(("swaylock", &["-f"], Locker));
        out.push(("hyprlock", &[], Locker));
    } else {
        out.push(("i3lock", &[], Locker));
        out.push(("xsecurelock", &[], Locker));
        out.push(("slock", &[], Locker));
        out.push(("xdg-screensaver", &["lock"], Quick));
    }
    out.push(("loginctl", &["lock-session"], Quick));
    out
}

fn run_locker(bin: &str, args: &[&str], kind: LockKind) -> bool {
    let mut cmd = Command::new(bin);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    match kind {
        LockKind::Quick => cmd.status().is_ok_and(|s| s.success()),
        LockKind::Locker => {
            let Ok(mut child) = cmd.spawn() else { return false };
            // Si falla (no hay pantalla, otro bloqueo activo…) suele salir al instante.
            std::thread::sleep(Duration::from_millis(400));
            match child.try_wait() {
                Ok(Some(st)) => st.success(), // `swaylock -f`/`i3lock` se desacoplan tras bloquear
                Ok(None) => {
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                    true
                }
                Err(_) => false,
            }
        }
    }
}

pub fn lock_workstation() -> Result<(), String> {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    let wayland = is_wayland() || std::env::var_os("WAYLAND_DISPLAY").is_some();
    let hyprland = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some();
    for (bin, args, kind) in lock_candidates(&desktop, wayland, hyprland) {
        if run_locker(bin, args, kind) {
            return Ok(());
        }
    }
    Err("No se pudo bloquear la sesión: instala hyprlock, swaylock o i3lock (o usa un escritorio con loginctl)".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bins(desktop: &str, wayland: bool, hyprland: bool) -> Vec<&'static str> {
        lock_candidates(desktop, wayland, hyprland).into_iter().map(|(b, _, _)| b).collect()
    }

    #[test]
    fn orden_de_bloqueadores() {
        assert_eq!(bins("ubuntu:GNOME", true, false)[0], "loginctl");
        assert_eq!(bins("KDE", false, false)[0], "loginctl");
        assert_eq!(bins("Hyprland", true, true)[0], "hyprlock");
        assert_eq!(bins("sway", true, false)[0], "swaylock");
        assert_eq!(bins("i3", false, false)[0], "i3lock");
        // loginctl siempre queda como último recurso.
        assert_eq!(*bins("niri", true, false).last().unwrap(), "loginctl");
    }

    #[test]
    fn cursor_hyprland_escalado() {
        assert_eq!(parse_hyprland_cursor("100, 50\n", 2.0), Some((200, 100)));
        assert_eq!(parse_hyprland_cursor("100, 50", 0.0), Some((100, 50)));
        assert_eq!(parse_hyprland_cursor("basura", 1.0), None);
    }

    #[test]
    fn fullscreen_hyprland_int_y_bool() {
        assert_eq!(parse_hyprland_fullscreen(r#"{"pid":1,"fullscreen":2}"#, 9), Some(true));
        assert_eq!(parse_hyprland_fullscreen(r#"{"pid":1,"fullscreen":0}"#, 9), Some(false));
        assert_eq!(parse_hyprland_fullscreen(r#"{"pid":1,"fullscreen":true}"#, 9), Some(true));
        assert_eq!(parse_hyprland_fullscreen(r#"{"pid":9,"fullscreen":true}"#, 9), Some(false));
    }
}

