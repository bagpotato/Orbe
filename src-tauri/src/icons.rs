//! Icono real de un archivo (exe, lnk…) como PNG en data URL, con caché en disco.

use base64::Engine;
use std::path::{Path, PathBuf};

use windows::core::HSTRING;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHGetImageList, SHFILEINFOW, SHGFI_SYSICONINDEX, SHIL_EXTRALARGE};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

pub fn icon_data_url(ruta: &str) -> Option<String> {
    let path = resolve(ruta)?;
    let png = cached_or_extract(&path)?;
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
}

const MAX_IMAGE: usize = 2 * 1024 * 1024;

/// Imagen de una app para el frontend: la elegida (`imagen`) o, si no hay, el icono del programa.
/// Devuelve una URL relativa (biblioteca incluida) o una data URL.
pub fn app_image(imagen: &str, ruta: &str) -> Option<String> {
    let imagen = imagen.trim();
    if let Some(name) = imagen.strip_prefix("lib:") {
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return Some(format!("img/apps/{name}.png"));
        }
    } else if !imagen.is_empty() {
        if let Some(url) = file_data_url(Path::new(&crate::launcher::expand_env(imagen))) {
            return Some(url);
        }
    }
    icon_data_url(ruta)
}

/// Tipo MIME por los primeros bytes; solo formatos de imagen que WebView2 sabe pintar.
fn sniff_mime(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.starts_with(b"GIF8") {
        Some("image/gif")
    } else if b.len() > 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else if b.starts_with(&[0, 0, 1, 0]) {
        Some("image/x-icon")
    } else {
        let head = String::from_utf8_lossy(&b[..b.len().min(512)]).to_lowercase();
        (head.contains("<svg")).then_some("image/svg+xml")
    }
}

fn file_data_url(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() > MAX_IMAGE {
        return None;
    }
    let mime = sniff_mime(&bytes)?;
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)))
}

/// Guarda una imagen importada desde los ajustes en `%APPDATA%\orbe\imagenes` y devuelve su ruta.
pub fn import_image(nombre: &str, data_url: &str) -> Result<String, String> {
    let b64 = data_url.split_once(";base64,").map(|(_, d)| d).ok_or("Formato de imagen no válido")?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|_| "Imagen dañada")?;
    if bytes.len() > MAX_IMAGE {
        return Err("La imagen pesa más de 2 MB".into());
    }
    let ext = match sniff_mime(&bytes).ok_or("Formato no admitido (usa PNG, JPG, GIF, WebP, ICO o SVG)")? {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/x-icon" => "ico",
        _ => "svg",
    };
    let base: String = nombre
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(40)
        .collect();
    let base = if base.is_empty() { "imagen".to_string() } else { base };
    let dir = crate::config::config_dir().join("imagenes");
    std::fs::create_dir_all(&dir).map_err(|e| format!("No se pudo crear {}: {e}", dir.display()))?;
    let digest = bytes.iter().fold(0xcbf29ce484222325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x100000001b3));
    let path = dir.join(format!("{base}-{:08x}.{ext}", digest as u32));
    std::fs::write(&path, &bytes).map_err(|e| format!("No se pudo guardar la imagen: {e}"))?;
    Ok(path.to_string_lossy().into_owned())
}

/// Convierte una ruta de la config en un archivo existente, o None si es una URI.
pub fn resolve(ruta: &str) -> Option<PathBuf> {
    let r = crate::launcher::expand_env(ruta.trim());
    let r = r.trim_matches('"');
    if r.is_empty() {
        return None;
    }
    let is_drive = r.len() >= 3 && r.as_bytes()[1] == b':' && matches!(r.as_bytes()[2], b'\\' | b'/');
    if r.contains(':') && !is_drive {
        return None; // ms-settings:, shell:, https://…
    }
    let p = PathBuf::from(r);
    if p.is_absolute() {
        return p.exists().then_some(p);
    }
    if r.contains(['\\', '/']) {
        return None;
    }
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|v| std::env::split_paths(&v).collect()).unwrap_or_default();
    if let Some(root) = std::env::var_os("SystemRoot") {
        dirs.push(PathBuf::from(&root).join("System32"));
        dirs.push(PathBuf::from(root));
    }
    // Los alias de ejecución de WindowsApps (wt.exe, mspaint.exe…) solo dan un icono genérico:
    // mejor usar el glifo.
    dirs.retain(|d| !d.to_string_lossy().to_lowercase().contains(r"microsoft\windowsapps"));
    dirs.into_iter().map(|d| d.join(r)).find(|c| c.is_file())
}

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn cached_or_extract(path: &Path) -> Option<Vec<u8>> {
    let key = path.to_string_lossy().to_lowercase();
    let cache = crate::config::config_dir().join("icons").join(format!("{:016x}.png", fnv1a(&key)));
    if let Ok(bytes) = std::fs::read(&cache) {
        return Some(bytes);
    }
    let (w, h, rgba) = extract(path)?;
    let png = encode_png(w, h, &rgba)?;
    if let Some(dir) = cache.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&cache, &png);
    Some(png)
}

/// La lista de imágenes del sistema es compartida por todo el proceso: extraer desde varios
/// hilos a la vez devuelve iconos cruzados entre apps (y se quedarían en la caché).
static EXTRACT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn extract(path: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let _guard = EXTRACT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let mut info = SHFILEINFOW::default();
        let ok = SHGetFileInfoW(
            &HSTRING::from(path.as_os_str()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_SYSICONINDEX,
        );
        if ok == 0 {
            return None;
        }
        let list: IImageList = SHGetImageList(SHIL_EXTRALARGE as i32).ok()?;
        let hicon = list.GetIcon(info.iIcon, ILD_TRANSPARENT.0).ok()?;
        let out = hicon_to_rgba(hicon);
        let _ = DestroyIcon(hicon);
        out
    }
}

unsafe fn bitmap_bits(hbm: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    let mut bm = BITMAP::default();
    if GetObjectW(HGDIOBJ(hbm.0), std::mem::size_of::<BITMAP>() as i32, Some(&mut bm as *mut _ as *mut _)) == 0 {
        return None;
    }
    let (w, h) = (bm.bmWidth, bm.bmHeight);
    if w <= 0 || h <= 0 || w > 512 || h > 512 {
        return None;
    }
    let mut bi = BITMAPINFO::default();
    bi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w,
        biHeight: -h, // de arriba abajo
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let dc = CreateCompatibleDC(None);
    let lines = GetDIBits(dc, hbm, 0, h as u32, Some(buf.as_mut_ptr() as *mut _), &mut bi, DIB_RGB_COLORS);
    let _ = DeleteDC(dc);
    (lines == h).then_some((w as u32, h as u32, buf))
}

unsafe fn hicon_to_rgba(hicon: HICON) -> Option<(u32, u32, Vec<u8>)> {
    let mut ii = ICONINFO::default();
    GetIconInfo(hicon, &mut ii).ok()?;
    let color = if ii.hbmColor.is_invalid() { None } else { bitmap_bits(ii.hbmColor) };
    let mask = if ii.hbmMask.is_invalid() { None } else { bitmap_bits(ii.hbmMask) };
    if !ii.hbmColor.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(ii.hbmColor.0));
    }
    if !ii.hbmMask.is_invalid() {
        let _ = DeleteObject(HGDIOBJ(ii.hbmMask.0));
    }
    let (w, h, mut px) = color?;
    // BGRA -> RGBA
    for p in px.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    // Iconos antiguos sin canal alfa: usar la máscara (blanco = transparente).
    if px.chunks_exact(4).all(|p| p[3] == 0) {
        match mask {
            Some((mw, mh, m)) if mw == w && mh == h => {
                for (p, mk) in px.chunks_exact_mut(4).zip(m.chunks_exact(4)) {
                    p[3] = if mk[0] == 0 { 255 } else { 0 };
                }
            }
            _ => px.chunks_exact_mut(4).for_each(|p| p[3] = 255),
        }
    }
    Some((w, h, px))
}

fn encode_png(w: u32, h: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_no_tienen_icono() {
        assert_eq!(resolve("https://x.com"), None);
        assert_eq!(resolve("shell:Downloads"), None);
        assert_eq!(resolve("ms-settings:"), None);
        assert_eq!(resolve(""), None);
    }

    #[test]
    fn encuentra_en_path() {
        assert!(resolve("notepad.exe").is_some());
        assert!(resolve(r"C:\Windows\explorer.exe").is_some());
    }

    #[test]
    fn imagen_de_biblioteca() {
        assert_eq!(app_image("lib:discord", "x"), Some("img/apps/discord.png".into()));
        // Nombres raros no salen de la carpeta: se ignora la imagen y se usa el icono (aquí, ninguno).
        assert_eq!(app_image("lib:../../secreto", "https://x.com"), None);
        assert_eq!(app_image("C:\\no\\existe.png", "https://x.com"), None);
    }

    #[test]
    fn importar_y_leer() {
        let png = encode_png(1, 1, &[255, 0, 0, 255]).unwrap();
        let url = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&png));
        let path = import_image("Mi App/../x", &url).unwrap();
        assert!(path.contains("mi-app"));
        let back = app_image(&path, "").unwrap();
        assert!(back.starts_with("data:image/png;base64,"));
        let _ = std::fs::remove_file(&path);
        assert!(import_image("x", "data:text/plain;base64,aG9sYQ==").is_err());
    }

    #[test]
    fn hash() {
        assert_eq!(fnv1a(""), 0xcbf29ce484222325);
    }

    #[test]
    fn extrae_icono_real() {
        let p = resolve("notepad.exe").unwrap();
        let (w, h, px) = extract(&p).expect("icono de notepad");
        assert_eq!((w, h), (48, 48));
        assert!(px.chunks_exact(4).any(|p| p[3] > 0));
        assert!(encode_png(w, h, &px).is_some());
    }
}
