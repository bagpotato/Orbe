//! Lanzar apps: `ShellExecuteW` en Windows y `.desktop` / `xdg-open` / comandos en Linux.

use crate::config::App;

#[cfg(windows)]
use windows::core::{w, HSTRING, PCWSTR};
#[cfg(windows)]
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE};
#[cfg(windows)]
use windows::Win32::UI::Shell::ShellExecuteW;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY, SW_SHOWNORMAL};

pub const LOCK: &str = "orbe:bloquear";

/// Sustituye `%VAR%`, `$VAR`, `${VAR}` y `~/` inicial por su valor; si no existen, los deja tal cual.
pub fn expand_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    if rest == "~" || rest.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) {
            out.push_str(&home.to_string_lossy());
            rest = &rest[1..];
        }
    }
    while !rest.is_empty() {
        let next_pct = rest.find('%');
        let next_dol = rest.find('$');
        let (idx, kind) = match (next_pct, next_dol) {
            (Some(a), Some(b)) => if a <= b { (a, '%') } else { (b, '$') },
            (Some(a), None) => (a, '%'),
            (None, Some(b)) => (b, '$'),
            (None, None) => {
                out.push_str(rest);
                break;
            }
        };
        out.push_str(&rest[..idx]);
        let after = &rest[idx + 1..];
        if kind == '%' {
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
        } else if let Some(braced) = after.strip_prefix('{') {
            match braced.find('}') {
                Some(end) if end > 0 => {
                    let name = &braced[..end];
                    match std::env::var(name) {
                        Ok(v) => out.push_str(&v),
                        Err(_) => {
                            out.push_str("${");
                            out.push_str(name);
                            out.push('}');
                        }
                    }
                    rest = &braced[end + 1..];
                }
                _ => {
                    out.push('$');
                    rest = after;
                }
            }
        } else {
            let len = after
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .count();
            if len > 0 {
                let name = &after[..len];
                match std::env::var(name) {
                    Ok(v) => out.push_str(&v),
                    Err(_) => {
                        out.push('$');
                        out.push_str(name);
                    }
                }
                rest = &after[len..];
            } else {
                out.push('$');
                rest = after;
            }
        }
    }
    out
}

/// Divide un campo `Exec=` de un `.desktop` en programa + argumentos, quitando códigos `%u`, `%F`, etc.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn parse_exec_command(exec: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_token = false;
    let mut quote: Option<char> = None;
    let mut chars = exec.chars().peekable();

    while let Some(c) = chars.next() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            } else if c == '\\' && q == '"' {
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            } else {
                cur.push(c);
            }
            in_token = true;
        } else if c == '"' || c == '\'' {
            quote = Some(c);
            in_token = true;
        } else if c == '\\' {
            if let Some(n) = chars.next() {
                cur.push(n);
                in_token = true;
            }
        } else if c.is_whitespace() {
            if in_token {
                tokens.push(std::mem::take(&mut cur));
                in_token = false;
            }
        } else {
            cur.push(c);
            in_token = true;
        }
    }
    if in_token {
        tokens.push(cur);
    }

    let mut out = Vec::with_capacity(tokens.len());
    for tok in tokens {
        if tok.starts_with('%') && tok.len() == 2 {
            let code = tok.as_bytes()[1] as char;
            if matches!(code, 'f' | 'F' | 'u' | 'U' | 'd' | 'D' | 'n' | 'N' | 'i' | 'c' | 'k' | 'v' | 'm') {
                continue;
            }
        }
        out.push(tok.replace("%%", "%"));
    }
    out
}

/// Bloqueante: llamar fuera del hilo principal.
#[cfg(windows)]
pub fn launch(app: &App) -> Result<(), String> {
    let ruta = expand_env(app.ruta.trim());
    if ruta.is_empty() {
        return Err(format!("«{}» no tiene ruta", app.nombre));
    }
    if ruta.eq_ignore_ascii_case(LOCK) {
        return crate::platform::lock_workstation();
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

#[cfg(not(windows))]
fn spawn_detached(bin: &str, args: &[String]) -> Result<(), String> {
    use std::process::{Command, Stdio};
    Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{bin}: {e}"))
}

#[cfg(not(windows))]
fn has_uri_scheme(s: &str) -> bool {
    let Some((scheme, _)) = s.split_once(':') else { return false };
    !scheme.is_empty() && scheme.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
}

#[cfg(not(windows))]
pub fn launch(app: &App) -> Result<(), String> {
    let ruta = expand_env(app.ruta.trim());
    if ruta.is_empty() {
        return Err(format!("«{}» no tiene ruta", app.nombre));
    }
    if ruta.eq_ignore_ascii_case(LOCK) {
        return crate::platform::lock_workstation();
    }

    // Alias heredados de configs de Windows.
    let ruta = match ruta.as_str() {
        "shell:RecycleBinFolder" => "trash:///".to_string(),
        "shell:Downloads" => expand_env("~/Descargas"),
        "shell:Personal" => expand_env("~/Documentos"),
        _ => ruta,
    };

    let extra_args = parse_exec_command(&expand_env(app.args.trim()));

    if ruta.ends_with(".desktop") {
        if let Some(path) = crate::apps::find_desktop_file(&ruta) {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Some(entry) = crate::apps::parse_desktop_entry(&content, "") {
                    let parts = parse_exec_command(&entry.exec);
                    if let Some((bin, base_args)) = parts.split_first() {
                        let mut all = base_args.to_vec();
                        all.extend(extra_args);
                        return spawn_detached(bin, &all)
                            .map_err(|e| format!("No se pudo abrir «{}»: {e}", app.nombre));
                    }
                }
            }
            if let Some(stem) = path.file_name().and_then(|s| s.to_str()) {
                return spawn_detached("gtk-launch", &[stem.to_string()])
                    .map_err(|e| format!("No se pudo abrir «{}»: {e}", app.nombre));
            }
        }
        return spawn_detached("gtk-launch", &[ruta])
            .map_err(|e| format!("No se pudo abrir «{}»: {e}", app.nombre));
    }

    if has_uri_scheme(&ruta) || std::path::Path::new(&ruta).is_dir() {
        return spawn_detached("xdg-open", &[ruta])
            .map_err(|e| format!("No se pudo abrir «{}»: {e}", app.nombre));
    }

    // Si `ruta` incluye argumentos en línea (p. ej. "xdg-open ~") y `args` está vacío, los dividimos.
    let parts = if extra_args.is_empty() && !std::path::Path::new(&ruta).exists() {
        parse_exec_command(&ruta)
    } else {
        let mut v = vec![ruta.clone()];
        v.extend(extra_args);
        v
    };
    let Some((bin, cmd_args)) = parts.split_first() else {
        return Err(format!("«{}» no tiene comando válido", app.nombre));
    };
    let expanded_args: Vec<String> = cmd_args.iter().map(|a| expand_env(a)).collect();
    spawn_detached(bin, &expanded_args).map_err(|e| format!("No se pudo abrir «{}»: {e}", app.nombre))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables() {
        std::env::set_var("ORBE_TEST_VAR", "C:\\x");
        assert_eq!(expand_env("%ORBE_TEST_VAR%\\a.exe"), "C:\\x\\a.exe");
        assert_eq!(expand_env("%NO_EXISTE_ORBE%\\a"), "%NO_EXISTE_ORBE%\\a");
        assert_eq!(expand_env("$ORBE_TEST_VAR/a"), "C:\\x/a");
        assert_eq!(expand_env("${ORBE_TEST_VAR}/b"), "C:\\x/b");
        assert_eq!(expand_env("$NO_EXISTE_ORBE/a"), "$NO_EXISTE_ORBE/a");
        assert_eq!(expand_env("sin variables"), "sin variables");
        assert_eq!(expand_env("100% seguro"), "100% seguro");
        assert_eq!(expand_env("%%"), "%%");

        std::env::set_var("HOME", "/home/test");
        assert_eq!(expand_env("~/Documentos"), "/home/test/Documentos");
    }

    #[test]
    fn exec_desktop() {
        assert_eq!(
            parse_exec_command("\"/usr/bin/my app\" --flag %U --name=\"a b\" %%"),
            vec!["/usr/bin/my app", "--flag", "--name=a b", "%"]
        );
        assert_eq!(parse_exec_command("firefox %u"), vec!["firefox"]);
    }
}
