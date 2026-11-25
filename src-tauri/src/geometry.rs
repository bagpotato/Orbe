//! Geometría pura de la zona y la media rueda. Mismas fórmulas que `src/geometry.js`.
//! Se calcula en píxeles lógicos relativos al monitor y se devuelve en físicos absolutos.

pub const MARGIN: f64 = 30.0;
pub const SNAP: f64 = 60.0;
pub const MAX_RINGS: usize = 4;
/// Tolerancia (lógicos) alrededor de la rueda antes de empezar a cerrarla.
pub const SLACK: f64 = 40.0;

pub fn ring_r(i: usize) -> f64 {
    86.0 + 59.0 * i as f64
}

pub fn plate_r(n: usize) -> f64 {
    ring_r(n.clamp(1, MAX_RINGS) - 1) + 32.0
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub sf: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneGeo {
    /// Rectángulo de la zona, físicos absolutos (inclusivo).
    pub zone_l: f64,
    pub zone_t: f64,
    pub zone_r: f64,
    pub zone_b: f64,
    pub facing: f64,
    pub scale: f64,
    /// Centro de la rueda, físicos absolutos.
    pub cx: f64,
    pub cy: f64,
    /// Radio de la placa ya escalado, físicos.
    pub ps: f64,
    /// Lado de la ventana en lógicos (S).
    pub size_logical: f64,
    pub size_phys: i32,
    pub win_x: i32,
    pub win_y: i32,
    pub mon_x: i32,
    pub mon_y: i32,
    pub sf: f64,
}

/// `zx`, `zy`: lógicos relativos al monitor.
pub fn facing_for(w: f64, h: f64, zx: f64, zy: f64, orientacion: &str) -> f64 {
    match orientacion {
        "derecha" => return 0.0,
        "izquierda" => return 180.0,
        "abajo" => return 90.0,
        "arriba" => return -90.0,
        _ => {}
    }
    let d = [zx, w - zx, zy, h - zy];
    let mut m = 0;
    for i in 1..4 {
        if d[i] < d[m] {
            m = i;
        }
    }
    [0.0, 180.0, 90.0, -90.0][m]
}

pub fn compute(mon: MonitorRect, x: f64, y: f64, tamano: u32, orientacion: &str, n_rings: usize) -> ZoneGeo {
    let sf = if mon.sf > 0.0 { mon.sf } else { 1.0 };
    let w = mon.w as f64 / sf;
    let h = mon.h as f64 / sf;
    let zx = x.clamp(0.0, 1.0) * w;
    let zy = y.clamp(0.0, 1.0) * h;

    let facing = facing_for(w, h, zx, zy, orientacion);
    let p = plate_r(n_rings);
    let scale = 1f64.min((h - 16.0) / (2.0 * p)).min((w - 16.0) / (2.0 * p)).max(0.1);
    let ps = p * scale;
    // clamp() entra en pánico si min > max; la escala garantiza ps <= h/2, pero por si acaso:
    let clamp = |v: f64, a: f64, b: f64| if a > b { (a + b) / 2.0 } else { v.clamp(a, b) };

    let (cx, cy) = match facing.round() as i32 {
        0 => (if zx < SNAP { 0.0 } else { zx.min(w - ps) }, clamp(zy, ps, h - ps)),
        180 => (if w - zx < SNAP { w } else { zx.max(ps) }, clamp(zy, ps, h - ps)),
        90 => (clamp(zx, ps, w - ps), if zy < SNAP { 0.0 } else { zy.min(h - ps) }),
        _ => (clamp(zx, ps, w - ps), if h - zy < SNAP { h } else { zy.max(ps) }),
    };

    // Zona: cuadrado de lado `tamano`; si toca un borde, se extiende hasta él.
    let t = tamano as f64;
    let half = t / 2.0;
    let l = if zx < t { 0.0 } else { zx - half };
    let r = if w - zx < t { w } else { zx + half };
    let top = if zy < t { 0.0 } else { zy - half };
    let b = if h - zy < t { h } else { zy + half };

    let ox = mon.x as f64;
    let oy = mon.y as f64;
    let max_x = ox + mon.w as f64 - 1.0;
    let max_y = oy + mon.h as f64 - 1.0;

    let s = 2.0 * (ps + MARGIN);
    let size_phys = (s * sf).round() as i32;
    let cx_abs = ox + cx * sf;
    let cy_abs = oy + cy * sf;

    ZoneGeo {
        zone_l: ox + l * sf,
        zone_t: oy + top * sf,
        zone_r: (ox + r * sf).min(max_x),
        zone_b: (oy + b * sf).min(max_y),
        facing,
        scale,
        cx: cx_abs,
        cy: cy_abs,
        ps: ps * sf,
        size_logical: s,
        size_phys,
        win_x: (cx_abs - size_phys as f64 / 2.0).round() as i32,
        win_y: (cy_abs - size_phys as f64 / 2.0).round() as i32,
        mon_x: mon.x,
        mon_y: mon.y,
        sf,
    }
}

/// Franjas horizontales `(x, y, width)` de la media rueda dentro de un cuadrado `size x size`.
/// La usa el recorte `cairo::Region` de Linux; Windows recorta con regiones GDI propias.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn half_disc_spans(size: i32, radius: i32, facing_deg: f64) -> Vec<(i32, i32, i32)> {
    if size <= 0 || radius <= 0 {
        return Vec::new();
    }
    let c = size / 2;
    let (l, t, r, b) = match facing_deg.round() as i32 {
        0 => (c - 2, 0, size, size),
        180 => (0, 0, c + 2, size),
        90 => (0, c - 2, size, size),
        _ => (0, 0, size, c + 2),
    };
    let y0 = (c - radius).max(t).max(0);
    let y1 = (c + radius + 1).min(b).min(size);
    let r2 = (radius as f64) * (radius as f64);
    let mut spans = Vec::with_capacity((y1 - y0).max(0) as usize);
    for y in y0..y1 {
        let dy = (y - c) as f64;
        let dx = (r2 - dy * dy).max(0.0).sqrt().round() as i32;
        let x0 = (c - dx).max(l).max(0);
        let x1 = (c + dx + 1).min(r).min(size);
        if x1 > x0 {
            spans.push((x0, y, x1 - x0));
        }
    }
    spans
}

pub fn in_zone(g: &ZoneGeo, px: f64, py: f64) -> bool {
    px >= g.zone_l && px <= g.zone_r && py >= g.zone_t && py <= g.zone_b
}

pub fn in_wheel(g: &ZoneGeo, px: f64, py: f64) -> bool {
    let dx = px - g.cx;
    let dy = py - g.cy;
    let f = g.facing.to_radians();
    let along = dx * f.cos() + dy * f.sin();
    dx.hypot(dy) <= g.ps + SLACK * g.sf && along >= -SLACK * g.sf
}

#[cfg(test)]
mod tests {
    use super::*;

    const FHD: MonitorRect = MonitorRect { x: 0, y: 0, w: 1920, h: 1080, sf: 1.0 };

    #[test]
    fn borde_izquierdo() {
        let g = compute(FHD, 0.0, 0.5, 24, "auto", 3);
        assert_eq!(g.facing, 0.0);
        assert_eq!((g.cx, g.cy), (0.0, 540.0));
        assert_eq!(g.scale, 1.0);
        assert_eq!(g.ps, 236.0);
        assert_eq!(g.size_logical, 532.0);
        assert_eq!((g.win_x, g.win_y), (-266, 274));
        assert!(in_zone(&g, 0.0, 540.0));
        assert!(!in_zone(&g, 100.0, 540.0));
        assert!(in_wheel(&g, 200.0, 540.0));
        assert!(!in_wheel(&g, -60.0, 540.0));
        assert!(!in_wheel(&g, 600.0, 540.0));
    }

    #[test]
    fn otros_bordes() {
        let g = compute(FHD, 1.0, 0.5, 24, "auto", 3);
        assert_eq!(g.facing, 180.0);
        assert_eq!(g.cx, 1920.0);
        assert!(in_zone(&g, 1919.0, 540.0));
        assert_eq!(compute(FHD, 0.5, 0.0, 24, "auto", 3).facing, 90.0);
        assert_eq!(compute(FHD, 0.5, 1.0, 24, "auto", 3).facing, -90.0);
        assert_eq!(compute(FHD, 1.0, 0.5, 24, "derecha", 3).facing, 0.0);
    }

    #[test]
    fn esquina_se_desplaza_para_caber() {
        let g = compute(FHD, 0.0, 0.0, 24, "auto", 3);
        assert_eq!(g.facing, 0.0);
        assert_eq!(g.cy, 236.0);
    }

    #[test]
    fn pantalla_pequena_escala() {
        let m = MonitorRect { x: 0, y: 0, w: 1366, h: 768, sf: 1.0 };
        let g = compute(m, 0.0, 0.5, 24, "auto", 3);
        assert!((g.scale - 1.0).abs() < 1e-9);
        let m = MonitorRect { x: 0, y: 0, w: 1024, h: 400, sf: 1.0 };
        let g = compute(m, 0.0, 0.5, 24, "auto", 3);
        assert!((g.scale - (400.0 - 16.0) / 472.0).abs() < 1e-9);
    }

    #[test]
    fn monitor_secundario_hidpi() {
        let m = MonitorRect { x: 1920, y: 0, w: 2560, h: 1440, sf: 1.5 };
        let g = compute(m, 0.0, 0.5, 24, "auto", 3);
        assert_eq!(g.cx, 1920.0);
        assert_eq!(g.cy, 720.0);
        assert_eq!(g.ps, 354.0);
        assert_eq!(g.size_phys, 798);
        assert!(in_zone(&g, 1920.0, 720.0));
        assert!(!in_zone(&g, 1919.0, 720.0));
    }

    #[test]
    fn cuatro_anillos() {
        assert_eq!(plate_r(4), 295.0);
        assert_eq!(plate_r(0), plate_r(1));
        assert_eq!(plate_r(9), plate_r(4));
    }

    #[test]
    fn franjas_media_rueda() {
        let right = half_disc_spans(532, 238, 0.0);
        assert!(!right.is_empty());
        assert!(right.iter().all(|&(x, y, w)| x >= 264 && x + w <= 532 && y >= 0 && y < 532));

        let left = half_disc_spans(532, 238, 180.0);
        assert!(left.iter().all(|&(x, y, w)| x >= 0 && x + w <= 268 && y >= 0 && y < 532));

        let down = half_disc_spans(532, 238, 90.0);
        assert!(down.iter().all(|&(_, y, _)| y >= 264 && y < 532));

        let up = half_disc_spans(532, 238, -90.0);
        assert!(up.iter().all(|&(_, y, _)| y >= 0 && y < 268));

        assert!(half_disc_spans(0, 100, 0.0).is_empty());
    }
}
