mod apps;
mod config;
mod cursor;
mod geometry;
mod icons;
mod launcher;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod win;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(windows)]
use win as platform;

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use serde_json::{json, Value};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent, Wry,
};

use config::Config;
use geometry::ZoneGeo;

pub struct Runtime {
    pub config: Config,
    /// false = pausado desde la bandeja.
    pub enabled: bool,
    /// Ventana "placer" abierta: la zona no se activa.
    pub placing: bool,
    pub placer_index: usize,
    pub geo: Option<ZoneGeo>,
    /// Lo pone `launch_app`; lo consume el hilo del cursor.
    pub force_close: bool,
    pub wheel_open: bool,
}

pub struct AppState(pub Mutex<Runtime>);

struct PauseItem(MenuItem<Wry>);

const CLOSE_ANIM: Duration = Duration::from_millis(260);

fn with_rt<T>(app: &AppHandle, f: impl FnOnce(&mut Runtime) -> T) -> T {
    let st = app.state::<AppState>();
    let mut rt = st.0.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut rt)
}

// ---------------------------------------------------------------- monitores y geometría

fn monitor_rect(m: &Monitor) -> geometry::MonitorRect {
    let p = m.position();
    let s = m.size();
    geometry::MonitorRect { x: p.x, y: p.y, w: s.width, h: s.height, sf: m.scale_factor() }
}

fn monitor_name(m: &Monitor) -> String {
    m.name().cloned().unwrap_or_default()
}

/// Índice del monitor con ese nombre; si no existe (o nombre vacío), el principal.
fn monitor_index(app: &AppHandle, monitors: &[Monitor], name: &str) -> usize {
    if !name.is_empty() {
        if let Some(i) = monitors.iter().position(|m| monitor_name(m) == name) {
            return i;
        }
    }
    let primary = app.primary_monitor().ok().flatten().map(|m| monitor_name(&m));
    primary.and_then(|p| monitors.iter().position(|m| monitor_name(m) == p)).unwrap_or(0)
}

pub(crate) fn refresh_zone_geo(app: &AppHandle) {
    let (zona, n) = with_rt(app, |rt| (rt.config.zona.clone(), rt.config.anillos.len()));
    let monitors = app.available_monitors().unwrap_or_default();
    if monitors.is_empty() {
        return;
    }
    let m = &monitors[monitor_index(app, &monitors, &zona.monitor)];
    let g = geometry::compute(monitor_rect(m), zona.x, zona.y, zona.tamano, &zona.orientacion, n);
    with_rt(app, |rt| rt.geo = Some(g));
    platform::sync_trigger_zone(app, &g);
}

// ---------------------------------------------------------------- ventana de la rueda

pub(crate) fn show_wheel(app: &AppHandle, g: ZoneGeo) {
    with_rt(app, |rt| rt.wheel_open = true);
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(w) = a.get_webview_window("wheel") {
            platform::show_wheel(&w, &g);
        }
        let _ = a.emit_to(
            "wheel",
            "orbe:open",
            json!({ "facing": g.facing, "scale": g.scale, "size": g.size_logical }),
        );
    });
}

pub(crate) fn hide_wheel(app: &AppHandle) {
    with_rt(app, |rt| rt.wheel_open = false);
    let _ = app.emit_to("wheel", "orbe:close", ());
    let a = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(CLOSE_ANIM);
        if with_rt(&a, |rt| rt.wheel_open) {
            return; // se volvió a abrir mientras tanto
        }
        let b = a.clone();
        let _ = a.run_on_main_thread(move || {
            if let Some(w) = b.get_webview_window("wheel") {
                platform::hide_wheel(&w);
            }
        });
    });
}

fn reload(app: &AppHandle, cfg: Config) {
    with_rt(app, |rt| rt.config = cfg);
    refresh_zone_geo(app);
    let _ = app.emit("orbe:config-changed", ());
}

// ---------------------------------------------------------------- otras ventanas

fn open_settings_window(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return Ok(());
    }
    WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("Orbe — Ajustes")
        .inner_size(1100.0, 720.0)
        .min_inner_size(820.0, 560.0)
        .center()
        // Sin esto, en Windows Tauri intercepta el arrastre y el drag & drop HTML5 no funciona.
        .disable_drag_drop_handler()
        .build()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn place_on_monitor(w: &WebviewWindow, m: &Monitor) {
    let _ = w.set_position(PhysicalPosition::new(m.position().x, m.position().y));
    let _ = w.set_size(PhysicalSize::new(m.size().width, m.size().height));
}

fn open_placer(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("placer") {
        let _ = w.set_focus();
        return Ok(());
    }
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    if monitors.is_empty() {
        return Err("No se encontró ningún monitor".into());
    }
    let zona_mon = with_rt(app, |rt| rt.config.zona.monitor.clone());
    let idx = monitor_index(app, &monitors, &zona_mon);
    with_rt(app, |rt| {
        rt.placing = true;
        rt.placer_index = idx;
    });
    let w = WebviewWindowBuilder::new(app, "placer", WebviewUrl::App("placer.html".into()))
        .title("Orbe — Colocar zona")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .visible(false)
        .build()
        .map_err(|e| {
            with_rt(app, |rt| rt.placing = false);
            e.to_string()
        })?;
    place_on_monitor(&w, &monitors[idx]);
    let _ = w.show();
    let _ = w.set_focus();
    Ok(())
}

fn close_placer(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("placer") {
        let _ = w.close();
    }
    with_rt(app, |rt| rt.placing = false);
}

#[derive(Serialize)]
struct PlacerInfo {
    monitor: String,
    x: f64,
    y: f64,
    tamano: u32,
    index: usize,
    count: usize,
}

fn placer_info(app: &AppHandle) -> Result<PlacerInfo, String> {
    let monitors = app.available_monitors().map_err(|e| e.to_string())?;
    if monitors.is_empty() {
        return Err("No se encontró ningún monitor".into());
    }
    let (zona, idx) = with_rt(app, |rt| (rt.config.zona.clone(), rt.placer_index));
    let idx = idx.min(monitors.len() - 1);
    let same = monitor_index(app, &monitors, &zona.monitor) == idx;
    Ok(PlacerInfo {
        monitor: monitor_name(&monitors[idx]),
        x: if same { zona.x } else { 0.0 },
        y: if same { zona.y } else { 0.5 },
        tamano: zona.tamano,
        index: idx,
        count: monitors.len(),
    })
}

// ---------------------------------------------------------------- comandos

fn wheel_data(cfg: &Config) -> Value {
    let anillos: Vec<Value> = cfg
        .anillos
        .iter()
        .map(|a| {
            let apps: Vec<Value> = a
                .apps
                .iter()
                .filter_map(|id| {
                    let app = cfg.apps.get(id)?;
                    Some(json!({
                        "id": id,
                        "nombre": app.nombre,
                        "icono": app.icono,
                        "img": icons::app_image(&app.imagen, &app.ruta),
                    }))
                })
                .collect();
            json!({ "nombre": a.nombre, "apps": apps })
        })
        .collect();
    // validate() garantiza que `tinte` es un color #RRGGBB(AA) válido, utilizable tal cual en CSS.
    json!({
        "anillos": anillos,
        "scroll_paso": cfg.rueda.scroll_paso,
        "invertir": cfg.rueda.invertir,
        "tinte": cfg.aspecto.tinte,
    })
}

fn join_err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
async fn get_wheel_data(state: State<'_, AppState>) -> Result<Value, String> {
    let cfg = state.0.lock().map_err(join_err)?.config.clone();
    tauri::async_runtime::spawn_blocking(move || wheel_data(&cfg)).await.map_err(join_err)
}

#[tauri::command]
async fn launch_app(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let app = {
        let mut rt = state.0.lock().map_err(join_err)?;
        rt.force_close = true;
        rt.config.apps.get(&id).cloned()
    }
    .ok_or_else(|| format!("App desconocida: {id}"))?;
    tauri::async_runtime::spawn_blocking(move || launcher::launch(&app)).await.map_err(join_err)?
}

#[tauri::command]
fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    Ok(state.0.lock().map_err(join_err)?.config.clone())
}

#[tauri::command]
fn save_config(app: AppHandle, mut config: Config) -> Result<(), String> {
    config::validate(&mut config)?;
    config::save(&config)?;
    reload(&app, config);
    Ok(())
}

#[tauri::command]
async fn list_installed_apps() -> Result<Vec<apps::InstalledApp>, String> {
    tauri::async_runtime::spawn_blocking(apps::list_installed).await.map_err(join_err)
}

#[tauri::command]
async fn get_icon(ruta: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || icons::icon_data_url(&ruta)).await.map_err(join_err)
}

#[tauri::command]
async fn get_app_image(imagen: String, ruta: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || icons::app_image(&imagen, &ruta)).await.map_err(join_err)
}

#[tauri::command]
async fn import_image(nombre: String, data: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || icons::import_image(&nombre, &data)).await.map_err(join_err)?
}

// Los comandos que crean ventanas son async: en Windows, crear ventanas desde un comando
// síncrono (que corre en el hilo principal) bloquea la app.
#[tauri::command]
async fn start_placing(app: AppHandle) -> Result<(), String> {
    open_placer(&app)
}

#[tauri::command]
async fn open_settings(app: AppHandle) -> Result<(), String> {
    open_settings_window(&app)
}

#[tauri::command]
fn get_placer_info(app: AppHandle) -> Result<PlacerInfo, String> {
    placer_info(&app)
}

#[tauri::command]
async fn placer_next_monitor(app: AppHandle) -> Result<PlacerInfo, String> {
    let monitors = app.available_monitors().map_err(join_err)?;
    if monitors.is_empty() {
        return Err("No se encontró ningún monitor".into());
    }
    let idx = with_rt(&app, |rt| {
        rt.placer_index = (rt.placer_index + 1) % monitors.len();
        rt.placer_index
    });
    if let Some(w) = app.get_webview_window("placer") {
        place_on_monitor(&w, &monitors[idx]);
    }
    placer_info(&app)
}

#[tauri::command]
fn save_zone(app: AppHandle, monitor: String, x: f64, y: f64) -> Result<(), String> {
    let mut cfg = with_rt(&app, |rt| rt.config.clone());
    cfg.zona.monitor = monitor;
    cfg.zona.x = x;
    cfg.zona.y = y;
    config::validate(&mut cfg)?;
    config::save(&cfg)?;
    close_placer(&app);
    reload(&app, cfg);
    Ok(())
}

#[tauri::command]
fn cancel_placing(app: AppHandle) {
    close_placer(&app);
}

// ---------------------------------------------------------------- arranque

fn toggle_pause(app: &AppHandle) {
    let enabled = with_rt(app, |rt| {
        rt.enabled = !rt.enabled;
        rt.enabled
    });
    if let Some(item) = app.try_state::<PauseItem>() {
        let _ = item.0.set_text(if enabled { "Pausar" } else { "Reanudar" });
    }
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let settings = MenuItem::with_id(app, "settings", "Ajustes…", true, None::<&str>)?;
    let place = MenuItem::with_id(app, "place", "Colocar zona", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pausar", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings, &place, &pause, &sep, &quit])?;
    app.manage(PauseItem(pause));

    let mut tray = TrayIconBuilder::with_id("orbe")
        .tooltip("Orbe")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| match ev.id().as_ref() {
            "settings" => {
                let _ = open_settings_window(app);
            }
            "place" => {
                let _ = open_placer(app);
            }
            "pause" => toggle_pause(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                let _ = open_settings_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn setup_wheel_window(app: &AppHandle) {
    let Some(w) = app.get_webview_window("wheel") else { return };
    platform::setup_wheel(&w);
}

/// `orbe.exe --ajustes` abre los ajustes; `orbe.exe --colocar`, el modo colocar.
/// Sirve tanto al arrancar como cuando ya hay otra instancia en marcha.
fn handle_args(app: &AppHandle, args: &[String]) {
    if args.iter().any(|a| a == "--ajustes") {
        let _ = open_settings_window(app);
    }
    if args.iter().any(|a| a == "--colocar") {
        let _ = open_placer(app);
    }
}

pub fn run() {
    let runtime = Runtime {
        config: config::load(),
        enabled: true,
        placing: false,
        placer_index: 0,
        geo: None,
        force_close: false,
        wheel_open: false,
    };

    tauri::Builder::default()
        // Debe registrarse el primero: una segunda ejecución solo reenvía sus argumentos.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if args.len() <= 1 {
                let _ = open_settings_window(app); // abrir Orbe otra vez = mostrar ajustes
            } else {
                handle_args(app, &args);
            }
        }))
        .manage(AppState(Mutex::new(runtime)))
        .invoke_handler(tauri::generate_handler![
            get_wheel_data,
            launch_app,
            get_config,
            save_config,
            list_installed_apps,
            get_icon,
            get_app_image,
            import_image,
            start_placing,
            open_settings,
            get_placer_info,
            placer_next_monitor,
            save_zone,
            cancel_placing,
        ])
        .on_window_event(|window, event| {
            if window.label() == "placer" && matches!(event, WindowEvent::Destroyed) {
                with_rt(window.app_handle(), |rt| rt.placing = false);
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            setup_wheel_window(&handle);
            setup_tray(&handle)?;
            handle_args(&handle, &std::env::args().collect::<Vec<_>>());
            cursor::spawn(handle);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error al arrancar Orbe");
}
