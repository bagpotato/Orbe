//! Lanzar apps con ShellExecuteW (ejecutables, accesos .lnk, URIs ms-settings:, shell:, https:…).

use crate::config::App;
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY, SW_SHOWNORMAL};

pub const LOCK: &str = "orbe:bloquear";

/// Sustituye `%VAR%` por su valor; si la variable no existe, lo deja tal cual.
pub fn expand_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if end > 0 => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(v) => out.push_str(&v),
                    Err(_) => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            _ => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Bloqueante: llamar fuera del hilo principal.
pub fn launch(app: &App) -> Result<(), String> {
    let ruta = expand_env(app.ruta.trim());
    if ruta.is_empty() {
        return Err(format!("«{}» no tiene ruta", app.nombre));
    }
    if ruta.eq_ignore_ascii_case(LOCK) {
        return crate::win::lock_workstation();
    }
    let file = HSTRING::from(ruta.as_str());
    let args = expand_env(app.args.trim());
    let args_h = HSTRING::from(args.as_str());
    let params = if args.is_empty() { PCWSTR::null() } else { PCWSTR(args_h.as_ptr()) };
    let code = unsafe {
        // ShellExecute necesita COM para shell:, .lnk y protocolos.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let _ = AllowSetForegroundWindow(ASFW_ANY);
        ShellExecuteW(None, w!("open"), &file, params, PCWSTR::null(), SW_SHOWNORMAL)
    };
    let n = code.0 as isize;
    if n <= 32 {
        Err(format!("No se pudo abrir «{}» ({ruta}), código {n}", app.nombre))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables() {
        std::env::set_var("ORBE_TEST_VAR", "C:\\x");
        assert_eq!(expand_env("%ORBE_TEST_VAR%\\a.exe"), "C:\\x\\a.exe");
        assert_eq!(expand_env("%NO_EXISTE_ORBE%\\a"), "%NO_EXISTE_ORBE%\\a");
        assert_eq!(expand_env("sin variables"), "sin variables");
        assert_eq!(expand_env("100% seguro"), "100% seguro");
        assert_eq!(expand_env("%%"), "%%");
    }
}
