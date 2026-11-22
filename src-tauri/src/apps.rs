//! Apps instaladas: accesos directos del menú Inicio (usuario y todos los usuarios).

use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InstalledApp {
    pub nombre: String,
    pub ruta: String,
}

const SKIP: [&str; 9] = [
    "uninstall", "desinstalar", "readme", "léame", "leame", "help", "ayuda", "website", "sitio web",
];

pub fn list_installed() -> Vec<InstalledApp> {
    let mut items = Vec::new();
    for var in ["APPDATA", "ProgramData"] {
        if let Some(base) = std::env::var_os(var) {
            let dir = PathBuf::from(base).join(r"Microsoft\Windows\Start Menu\Programs");
            walk(&dir, 0, &mut items);
        }
    }
    filter_and_sort(items)
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<InstalledApp>) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            walk(&path, depth + 1, out);
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
}
