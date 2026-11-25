//! Configuración de Orbe: `%APPDATA%\orbe\config.toml`.
//! Los nombres de campo son los mismos en TOML y en el JSON que ve el frontend.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

pub const MAX_ANILLOS: usize = 4;
pub const DEFAULT_TINTE: &str = "#5B7CFA1F";

// Orden de campos importante: las tablas simples antes que los arrays de tablas.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub zona: Zona,
    pub rueda: Rueda,
    pub aspecto: Aspecto,
    #[serde(rename = "anillo")]
    pub anillos: Vec<Anillo>,
    pub apps: BTreeMap<String, App>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Zona {
    pub monitor: String,
    pub x: f64,
    pub y: f64,
    pub tamano: u32,
    pub orientacion: String,
    pub retardo_ms: u64,
    pub gracia_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Rueda {
    pub atajo: String,
    pub scroll_paso: f64,
    pub invertir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Aspecto {
    /// Color que tiñe el cristal de la placa, `#RRGGBBAA`.
    pub tinte: String,
    pub tema: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Anillo {
    pub nombre: String,
    pub apps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct App {
    pub nombre: String,
    pub ruta: String,
    pub args: String,
    /// Glifo de ORBE_GLYPHS usado si no hay imagen ni icono real.
    pub icono: String,
    /// Imagen elegida: "" (icono del programa), "lib:<nombre>" (biblioteca incluida
    /// en src/img/apps) o la ruta de un archivo importado.
    pub imagen: String,
}

impl Default for Zona {
    fn default() -> Self {
        Self {
            monitor: String::new(),
            x: 0.0,
            y: 0.5,
            tamano: 24,
            orientacion: "auto".into(),
            retardo_ms: 120,
            gracia_ms: 250,
        }
    }
}

impl Default for Rueda {
    fn default() -> Self {
        Self { atajo: "Super+Space".into(), scroll_paso: 50.0, invertir: false }
    }
}

impl Default for Aspecto {
    fn default() -> Self {
        Self { tinte: DEFAULT_TINTE.into(), tema: "auto".into() }
    }
}

#[cfg(windows)]
fn default_groups() -> [(&'static str, &'static [(&'static str, &'static str, &'static str, &'static str)]); 3] {
    [
        (
            "Sistema",
            &[
                ("ajustes", "Ajustes", "ms-settings:", "gear"),
                ("archivos", "Archivos", "explorer.exe", "folder"),
                ("terminal", "Terminal", "wt.exe", "terminal"),
                ("monitor", "Monitor", "taskmgr.exe", "pulse"),
                ("papelera", "Papelera", "shell:RecycleBinFolder", "trash"),
                ("bloquear", "Bloquear", "orbe:bloquear", "lock"),
            ],
        ),
        (
            "Trabajo",
            &[
                ("notas", "Bloc de notas", "notepad.exe", "notes"),
                ("calculadora", "Calculadora", "calc.exe", "calc"),
                ("paint", "Paint", "mspaint.exe", "pen"),
                ("cmd", "Símbolo del sistema", "cmd.exe", "terminal"),
                ("powershell", "PowerShell", "powershell.exe", "code"),
                ("descargas", "Descargas", "shell:Downloads", "download"),
                ("documentos", "Documentos", "shell:Personal", "book"),
                ("red", "Red", "ms-settings:network", "wifi"),
                ("sonido", "Sonido", "ms-settings:sound", "music"),
            ],
        ),
        (
            "Favoritos",
            &[
                ("navegador", "Navegador", "https://duckduckgo.com", "globe"),
                ("correo", "Correo", "mailto:", "mail"),
                ("calendario", "Calendario", "outlookcal:", "calendar"),
                ("fotos", "Fotos", "ms-photos:", "photo"),
                ("musica", "Música", "mswindowsmusic:", "music"),
                ("mapas", "Mapas", "bingmaps:", "map"),
                ("tiempo", "Tiempo", "bingweather:", "sun"),
                ("recortes", "Recortes", "ms-screenclip:", "image"),
                ("tienda", "Tienda", "ms-windows-store:", "cart"),
                ("xbox", "Xbox", "xbox:", "game"),
                ("reloj", "Reloj", "ms-clock:", "bell"),
                ("camara", "Cámara", "microsoft.windows.camera:", "photo"),
                ("mensajes", "Mensajes", "ms-chat:", "chat"),
                ("notas_rapidas", "Notas rápidas", "ms-stickynotes:", "notes"),
            ],
        ),
    ]
}

#[cfg(not(windows))]
fn default_groups() -> [(&'static str, &'static [(&'static str, &'static str, &'static str, &'static str)]); 3] {
    [
        (
            "Sistema",
            &[
                ("ajustes", "Ajustes", "gnome-control-center", "gear"),
                ("archivos", "Archivos", "xdg-open ~", "folder"),
                ("terminal", "Terminal", "x-terminal-emulator", "terminal"),
                ("monitor", "Monitor", "htop", "pulse"),
                ("papelera", "Papelera", "trash:///", "trash"),
                ("bloquear", "Bloquear", "orbe:bloquear", "lock"),
            ],
        ),
        (
            "Trabajo",
            &[
                ("notas", "Editor", "xdg-open ~/Documentos", "notes"),
                ("calculadora", "Calculadora", "gnome-calculator", "calc"),
                ("paint", "Dibujo", "gimp", "pen"),
                ("cmd", "Consola", "kitty", "terminal"),
                ("powershell", "Código", "code", "code"),
                ("descargas", "Descargas", "xdg-open ~/Descargas", "download"),
                ("documentos", "Documentos", "xdg-open ~/Documentos", "book"),
                ("red", "Red", "nm-connection-editor", "wifi"),
                ("sonido", "Sonido", "pavucontrol", "music"),
            ],
        ),
        (
            "Favoritos",
            &[
                ("navegador", "Navegador", "https://duckduckgo.com", "globe"),
                ("correo", "Correo", "mailto:", "mail"),
                ("calendario", "Calendario", "https://calendar.google.com", "calendar"),
                ("fotos", "Imágenes", "xdg-open ~/Imágenes", "photo"),
                ("musica", "Música", "xdg-open ~/Música", "music"),
                ("mapas", "Mapas", "https://www.openstreetmap.org", "map"),
                ("tiempo", "Tiempo", "https://wttr.in", "sun"),
                ("recortes", "Capturas", "flameshot", "image"),
                ("tienda", "Software", "gnome-software", "cart"),
                ("steam", "Juegos", "steam", "game"),
                ("reloj", "Reloj", "gnome-clocks", "bell"),
                ("camara", "Vídeos", "xdg-open ~/Vídeos", "photo"),
                ("mensajes", "Mensajes", "https://web.whatsapp.com", "chat"),
                ("notas_rapidas", "Notas", "obsidian", "notes"),
            ],
        ),
    ]
}

impl Default for Config {
    fn default() -> Self {
        let mut apps = BTreeMap::new();
        let mut anillos = Vec::new();
        for (nombre, lista) in default_groups() {
            let mut ids = Vec::new();
            for (id, n, ruta, icono) in lista {
                apps.insert(
                    id.to_string(),
                    App {
                        nombre: n.to_string(),
                        ruta: ruta.to_string(),
                        args: String::new(),
                        icono: icono.to_string(),
                        imagen: String::new(),
                    },
                );
                ids.push(id.to_string());
            }
            anillos.push(Anillo { nombre: nombre.into(), apps: ids });
        }
        Self { zona: Zona::default(), rueda: Rueda::default(), aspecto: Aspecto::default(), anillos, apps }
    }
}

pub fn config_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("orbe")
    }
    #[cfg(not(windows))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()) {
            return PathBuf::from(xdg).join("orbe");
        }
        std::env::var_os("HOME")
            .filter(|v| !v.is_empty())
            .map(|h| PathBuf::from(h).join(".config"))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("orbe")
    }
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

/// Carga la configuración. Nunca falla: si falta se crea, si está corrupta se aparta a `.bak`.
pub fn load() -> Config {
    let path = config_path();
    let mut cfg = match fs::read_to_string(&path) {
        Ok(text) => match parse(&text) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("config.toml no válido ({e}); se usan valores por defecto");
                let _ = fs::rename(&path, path.with_extension("toml.bak"));
                let c = Config::default();
                let _ = save(&c);
                c
            }
        },
        Err(_) => {
            let c = Config::default();
            let _ = save(&c);
            c
        }
    };
    let _ = validate(&mut cfg);
    cfg
}

pub fn parse(text: &str) -> Result<Config, String> {
    toml::from_str(text).map_err(|e| e.to_string())
}

pub fn save(cfg: &Config) -> Result<(), String> {
    let path = config_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("No se pudo crear {}: {e}", dir.display()))?;
    }
    let text = toml::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, text).map_err(|e| format!("No se pudo escribir la configuración: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("No se pudo guardar la configuración: {e}"))
}

/// Corrige valores fuera de rango. Solo falla si no queda ningún anillo utilizable.
pub fn validate(cfg: &mut Config) -> Result<(), String> {
    if cfg.anillos.is_empty() {
        cfg.anillos.push(Anillo { nombre: "Anillo 1".into(), apps: vec![] });
    }
    cfg.anillos.truncate(MAX_ANILLOS);
    let apps = &cfg.apps;
    for (i, a) in cfg.anillos.iter_mut().enumerate() {
        if a.nombre.trim().is_empty() {
            a.nombre = format!("Anillo {}", i + 1);
        }
        a.apps.retain(|id| apps.contains_key(id));
    }
    let z = &mut cfg.zona;
    z.x = if z.x.is_finite() { z.x.clamp(0.0, 1.0) } else { 0.0 };
    z.y = if z.y.is_finite() { z.y.clamp(0.0, 1.0) } else { 0.5 };
    z.tamano = z.tamano.clamp(8, 200);
    z.retardo_ms = z.retardo_ms.min(5000);
    z.gracia_ms = z.gracia_ms.min(5000);
    if !["auto", "derecha", "izquierda", "abajo", "arriba"].contains(&z.orientacion.as_str()) {
        z.orientacion = "auto".into();
    }
    let r = &mut cfg.rueda;
    r.scroll_paso = if r.scroll_paso.is_finite() { r.scroll_paso.clamp(5.0, 500.0) } else { 50.0 };
    if hex_rgba(&cfg.aspecto.tinte).is_none() {
        cfg.aspecto.tinte = DEFAULT_TINTE.into();
    }
    Ok(())
}

/// `#RRGGBB` o `#RRGGBBAA` -> (r, g, b, a).
pub fn hex_rgba(s: &str) -> Option<(u8, u8, u8, u8)> {
    let h = s.trim().strip_prefix('#')?;
    if !(h.len() == 6 || h.len() == 8) || !h.is_ascii() {
        return None;
    }
    let b = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some((b(0)?, b(2)?, b(4)?, if h.len() == 8 { b(6)? } else { 255 }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta_toml() {
        let c = Config::default();
        let text = toml::to_string_pretty(&c).unwrap();
        assert_eq!(parse(&text).unwrap(), c);
        assert_eq!(c.anillos.len(), 3);
        assert_eq!(c.anillos.iter().map(|a| a.apps.len()).collect::<Vec<_>>(), vec![6, 9, 14]);
        assert_eq!(c.apps.len(), 29);
    }

    #[test]
    fn toml_parcial() {
        let c = parse("[zona]\nx = 1.0\n").unwrap();
        assert_eq!(c.zona.x, 1.0);
        assert_eq!(c.zona.y, 0.5);
        assert_eq!(c.rueda, Rueda::default());
        // Configs antiguas con `blur` siguen cargando.
        assert!(parse("[aspecto]\nblur = 18\ntinte = \"#FFFFFF22\"\n").is_ok());
    }

    #[test]
    fn validar() {
        let mut c = Config::default();
        c.anillos.push(Anillo { nombre: "".into(), apps: vec!["no_existe".into(), "cmd".into()] });
        c.anillos.push(Anillo { nombre: "sobra".into(), apps: vec![] });
        c.anillos[1].nombre = " ".into();
        c.zona.x = 3.0;
        c.zona.y = f64::NAN;
        c.aspecto.tinte = "rojo; background:url(x)".into();
        validate(&mut c).unwrap();
        assert_eq!(c.aspecto.tinte, DEFAULT_TINTE);
        assert_eq!(c.anillos.len(), 4);
        assert_eq!(c.anillos[1].nombre, "Anillo 2");
        assert_eq!(c.anillos[3].apps, vec!["cmd".to_string()]);
        assert_eq!(c.zona.x, 1.0);
        assert_eq!(c.zona.y, 0.5);
    }

    #[test]
    fn colores() {
        assert_eq!(hex_rgba("#FFFFFF22"), Some((255, 255, 255, 0x22)));
        assert_eq!(hex_rgba("#102030"), Some((16, 32, 48, 255)));
        assert_eq!(hex_rgba("102030"), None);
        assert_eq!(hex_rgba("#12345"), None);
    }
}
