//! Apps instaladas: accesos directos del menú Inicio en Windows y archivos `.desktop` en Linux.

use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InstalledApp {
    pub nombre: String,
    pub ruta: String,
}

// Solo lo usa Linux, pero se compila (y se testea) en todas las plataformas.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DesktopEntry {
    pub nombre: String,
    pub exec: String,
    pub icon: String,
}

const SKIP: [&str; 9] = [
    "uninstall", "desinstalar", "readme", "léame", "leame", "help", "ayuda", "website", "sitio web",
];

#[cfg(windows)]
pub fn list_installed() -> Vec<InstalledApp> {
    let mut items = Vec::new();
    for var in ["APPDATA", "ProgramData"] {
        if let Some(base) = std::env::var_os(var) {
            let dir = PathBuf::from(base).join(r"Microsoft\Windows\Start Menu\Programs");
            walk_windows(&dir, 0, &mut items);
        }
    }
    filter_and_sort(items)
}

#[cfg(windows)]
fn walk_windows(dir: &Path, depth: usize, out: &mut Vec<InstalledApp>) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            walk_windows(&path, depth + 1, out);
            continue;
        }
        let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        if ext != "lnk" && ext != "url" {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            out.push(InstalledApp { nombre: stem.to_string(), ruta: path.to_string_lossy().into_owned() });
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn desktop_app_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(data_home) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        dirs.push(PathBuf::from(data_home).join("applications"));
    } else if let Some(home) = std::env::var_os("HOME").filter(|v| !v.is_empty()) {
        let h = PathBuf::from(home);
        dirs.push(h.join(".local/share/applications"));
        dirs.push(h.join(".local/share/flatpak/exports/share/applications"));
    }
    if let Some(data_dirs) = std::env::var_os("XDG_DATA_DIRS").filter(|v| !v.is_empty()) {
        for base in std::env::split_paths(&data_dirs) {
            dirs.push(base.join("applications"));
        }
    } else {
        dirs.push(PathBuf::from("/usr/local/share/applications"));
        dirs.push(PathBuf::from("/usr/share/applications"));
    }
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share/applications"));
    dirs.push(PathBuf::from("/var/lib/snapd/desktop/applications"));

    let mut seen = HashSet::new();
    dirs.into_iter().filter(|d| seen.insert(d.clone())).collect()
}

#[cfg(not(windows))]
pub fn list_installed() -> Vec<InstalledApp> {
    let lang = preferred_lang();
    let mut items = Vec::new();
    for dir in desktop_app_dirs() {
        walk_desktop(&dir, 0, &lang, &mut items);
    }
    filter_and_sort(items)
}

#[cfg(not(windows))]
fn preferred_lang() -> String {
    std::env::var("LC_MESSAGES")
        .or_else(|_| std::env::var("LANG"))
        .ok()
        .and_then(|v| v.split(['.', '_', '@']).next().map(|s| s.to_ascii_lowercase()))
        .unwrap_or_default()
}

#[cfg(not(windows))]
fn walk_desktop(dir: &Path, depth: usize, lang: &str, out: &mut Vec<InstalledApp>) {
    if depth > 3 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            walk_desktop(&path, depth + 1, lang, out);
            continue;
        }
        if path.extension().and_then(|x| x.to_str()) != Some("desktop") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else { continue };
        if let Some(entry) = parse_desktop_entry(&content, lang) {
            out.push(InstalledApp { nombre: entry.nombre, ruta: path.to_string_lossy().into_owned() });
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn find_desktop_file(id_or_path: &str) -> Option<PathBuf> {
    let trimmed = id_or_path.trim().trim_matches('"');
    if trimmed.is_empty() {
        return None;
    }
    let p = Path::new(trimmed);
    if p.is_absolute() {
        return p.is_file().then(|| p.to_path_buf());
    }
    let with_ext = if trimmed.ends_with(".desktop") {
        trimmed.to_string()
    } else {
        format!("{trimmed}.desktop")
    };
    for dir in desktop_app_dirs() {
        let cand = dir.join(&with_ext);
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

/// Parsea la sección `[Desktop Entry]` de un archivo `.desktop`.
/// Devuelve `None` si está oculto (`NoDisplay=true` / `Hidden=true`), no es `Application` o no tiene `Exec`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn parse_desktop_entry(content: &str, lang: &str) -> Option<DesktopEntry> {
    let mut in_section = false;
    let mut entry_type = String::new();
    let mut name = String::new();
    let mut name_loc = String::new();
    let mut exec = String::new();
    let mut icon = String::new();
    let mut no_display = false;
    let mut hidden = false;

    let loc_key = (!lang.is_empty()).then(|| format!("Name[{lang}]"));

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            if in_section {
                break;
            }
            in_section = line == "[Desktop Entry]";
            continue;
        }
        if !in_section {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else { continue };
        let k = k.trim();
        let v = v.trim();
        match k {
            "Type" => entry_type = v.to_string(),
            "Name" => name = v.to_string(),
            "Exec" => exec = v.to_string(),
            "Icon" => icon = v.to_string(),
            "NoDisplay" => no_display = v.eq_ignore_ascii_case("true"),
            "Hidden" => hidden = v.eq_ignore_ascii_case("true"),
            _ if loc_key.as_deref() == Some(k) => name_loc = v.to_string(),
            _ => {}
        }
    }

    if no_display || hidden || entry_type != "Application" || exec.trim().is_empty() {
        return None;
    }
    let nombre = if !name_loc.is_empty() { name_loc } else { name };
    if nombre.trim().is_empty() {
        return None;
    }
    Some(DesktopEntry { nombre, exec, icon })
}

fn filter_and_sort(items: Vec<InstalledApp>) -> Vec<InstalledApp> {
    let mut seen = HashSet::new();
    let mut out: Vec<InstalledApp> = items
        .into_iter()
        .filter(|a| {
            let low = a.nombre.to_lowercase();
            !SKIP.iter().any(|s| low.contains(s)) && seen.insert(low)
        })
        .collect();
    out.sort_by_key(|a| a.nombre.to_lowercase());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(n: &str) -> InstalledApp {
        InstalledApp { nombre: n.into(), ruta: format!("C:\\{n}.lnk") }
    }

    #[test]
    fn filtra_deduplica_ordena() {
        let out = filter_and_sort(vec![a("zeta"), a("Uninstall Foo"), a("Alfa"), a("alfa"), a("Foo Help"), a("beta")]);
        let names: Vec<_> = out.iter().map(|x| x.nombre.as_str()).collect();
        assert_eq!(names, vec!["Alfa", "beta", "zeta"]);
    }

    #[test]
    fn parsea_desktop_entry() {
        let src = "\
[Desktop Entry]
Type=Application
Name=Files
Name[es]=Archivos
Exec=nautilus --new-window %U
Icon=org.gnome.Nautilus
NoDisplay=false

[Desktop Action new-window]
Name=New Window
Exec=nautilus -w
";
        let e = parse_desktop_entry(src, "es").unwrap();
        assert_eq!(e.nombre, "Archivos");
        assert_eq!(e.exec, "nautilus --new-window %U");
        assert_eq!(e.icon, "org.gnome.Nautilus");

        let hidden = "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNoDisplay=true\n";
        assert!(parse_desktop_entry(hidden, "es").is_none());

        let link = "[Desktop Entry]\nType=Link\nName=Web\nURL=https://x.com\n";
        assert!(parse_desktop_entry(link, "es").is_none());
    }
}
