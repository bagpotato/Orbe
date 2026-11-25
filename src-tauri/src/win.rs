//! Todo lo Win32: estilos de ventana, región, cursor, pantalla completa.
//!
//! La ventana de la rueda nunca se oculta con ShowWindow: se queda "visible" con una
//! región vacía (no pinta ni recibe clics). Así WebView2 no se congela y abrir es instantáneo.

use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    CombineRgn, CreateEllipticRgn, CreateRectRgn, DeleteObject, GetMonitorInfoW, MonitorFromWindow, SetWindowRgn,
    HGDIOBJ, MONITORINFO, MONITOR_DEFAULTTONEAREST, RGN_AND,
};
use windows::Win32::System::Shutdown::LockWorkStation;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetCursorPos, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
    IsZoomed, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, GWL_STYLE, HWND_TOPMOST, SWP_NOACTIVATE, SWP_SHOWWINDOW,
    WS_CAPTION, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
};

pub fn cursor_pos() -> Option<(i32, i32)> {
    let mut p = POINT::default();
    unsafe { GetCursorPos(&mut p) }.ok().map(|_| (p.x, p.y))
}

/// Ventana de herramienta que no se activa al hacer clic ni sale en Alt+Tab.
pub fn make_tool_noactivate(hwnd: HWND) {
    if hwnd.is_invalid() {
        return;
    }
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let add = (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0) as isize;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (ex | add) & !(WS_EX_APPWINDOW.0 as isize));
    }
}

/// Coloca, dimensiona y muestra la ventana encima de todo sin activarla.
pub fn show_no_activate(hwnd: HWND, x: i32, y: i32, w: i32, h: i32) {
    if hwnd.is_invalid() {
        return;
    }
    unsafe {
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), x, y, w, h, SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
}

/// Región vacía: la ventana no pinta nada ni captura el ratón.
pub fn set_empty_region(hwnd: HWND) {
    if hwnd.is_invalid() {
        return;
    }
    unsafe {
        let rgn = CreateRectRgn(0, 0, 0, 0);
        SetWindowRgn(hwnd, Some(rgn), true);
    }
}

/// Recorta la ventana (cuadrada, lado `size`) a la media rueda que mira hacia `facing_deg`.
pub fn set_half_disc_region(hwnd: HWND, size: i32, radius: i32, facing_deg: f64) {
    if hwnd.is_invalid() {
        return;
    }
    let c = size / 2;
    let (l, t, r, b) = match facing_deg.round() as i32 {
        0 => (c - 2, 0, size, size),
        180 => (0, 0, c + 2, size),
        90 => (0, c - 2, size, size),
        _ => (0, 0, size, c + 2),
    };
    unsafe {
        let disc = CreateEllipticRgn(c - radius, c - radius, c + radius + 1, c + radius + 1);
        let half = CreateRectRgn(l, t, r, b);
        CombineRgn(Some(disc), Some(disc), Some(half), RGN_AND);
        let _ = DeleteObject(HGDIOBJ(half.0));
        // El sistema se queda con `disc`: no se borra.
        SetWindowRgn(hwnd, Some(disc), true);
    }
}

/// ¿Hay una app de otro proceso ocupando todo su monitor (juego, vídeo, presentación)?
pub fn foreground_is_fullscreen() -> bool {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return false;
        }
        let mut buf = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut buf);
        if n > 0 {
            let class = String::from_utf16_lossy(&buf[..n as usize]);
            if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
                return false;
            }
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == std::process::id() {
            return false;
        }
        // Una ventana maximizada también cubre el monitor (incluso lo sobrepasa con sus bordes),
        // pero no es pantalla completa: esas tienen barra de título o están "zoomed".
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        if IsZoomed(hwnd).as_bool() || style & WS_CAPTION.0 == WS_CAPTION.0 {
            return false;
        }
        let mut rc = RECT::default();
        if GetWindowRect(hwnd, &mut rc).is_err() || rc.right <= rc.left || rc.bottom <= rc.top {
            return false;
        }
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(mon, &mut mi).as_bool() {
            return false;
        }
        let m = mi.rcMonitor;
        rc.left <= m.left && rc.top <= m.top && rc.right >= m.right && rc.bottom >= m.bottom
    }
}

pub fn lock_workstation() -> Result<(), String> {
    unsafe { LockWorkStation() }.map_err(|e| format!("No se pudo bloquear el equipo: {e}"))
}

fn hwnd_of(w: &tauri::WebviewWindow) -> Option<HWND> {
    w.hwnd().ok().map(|h| HWND(h.0 as _))
}

pub fn setup_wheel(w: &tauri::WebviewWindow) {
    if let Some(h) = hwnd_of(w) {
        make_tool_noactivate(h);
        set_empty_region(h);
        show_no_activate(h, 0, 0, 532, 532);
    }
}

pub fn show_wheel(w: &tauri::WebviewWindow, g: &crate::geometry::ZoneGeo) {
    if let Some(h) = hwnd_of(w) {
        set_half_disc_region(h, g.size_phys, g.ps.round() as i32 + 2, g.facing);
        show_no_activate(h, g.win_x, g.win_y, g.size_phys, g.size_phys);
    }
}

pub fn hide_wheel(w: &tauri::WebviewWindow) {
    if let Some(h) = hwnd_of(w) {
        set_empty_region(h);
    }
}

pub fn uses_native_trigger() -> bool {
    false
}

pub fn native_hover_state() -> (bool, bool) {
    (false, false)
}

pub fn sync_trigger_zone(_app: &tauri::AppHandle, _g: &crate::geometry::ZoneGeo) {}
