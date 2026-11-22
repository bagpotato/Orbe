// Geometría de la media rueda. Mismas fórmulas que src-tauri/src/geometry.rs.
// Unidades: píxeles lógicos (CSS). Ángulos en grados.
window.ORBE_GEO = {
  HUB: 46,
  MARGIN: 30,
  VISIBLE_ARC: 78,
  SNAP: 60,
  MAX_RINGS: 4,

  ringR(i) { return 86 + 59 * i; },
  ringSize(i) { return [44, 50, 54, 54][i] || 54; },
  ringStep(i) { return [40, 28, 20, 16][i] || 16; },
  plateR(n) {
    const k = Math.min(Math.max(n, 1), this.MAX_RINGS);
    return this.ringR(k - 1) + 32;
  },

  // zx, zy en píxeles relativos al monitor.
  facingFor(W, H, zx, zy, orientacion) {
    const forced = { derecha: 0, izquierda: 180, abajo: 90, arriba: -90 }[orientacion];
    if (forced !== undefined) return forced;
    const d = [zx, W - zx, zy, H - zy];
    return [0, 180, 90, -90][d.indexOf(Math.min(...d))];
  },

  // x, y en 0..1. Devuelve el centro de la rueda (lógicos, relativo al monitor).
  layout(W, H, x, y, n, orientacion) {
    const zx = x * W, zy = y * H;
    const facing = this.facingFor(W, H, zx, zy, orientacion);
    const P = this.plateR(n);
    const scale = Math.min(1, (H - 16) / (2 * P), (W - 16) / (2 * P));
    const Ps = P * scale;
    const clamp = (v, a, b) => Math.min(Math.max(v, a), b);
    let cx, cy;
    if (facing === 0) { cx = zx < this.SNAP ? 0 : Math.min(zx, W - Ps); cy = clamp(zy, Ps, H - Ps); }
    else if (facing === 180) { cx = W - zx < this.SNAP ? W : Math.max(zx, Ps); cy = clamp(zy, Ps, H - Ps); }
    else if (facing === 90) { cy = zy < this.SNAP ? 0 : Math.min(zy, H - Ps); cx = clamp(zx, Ps, W - Ps); }
    else { cy = H - zy < this.SNAP ? H : Math.max(zy, Ps); cx = clamp(zx, Ps, W - Ps); }
    return { facing, scale, cx, cy, P, Ps, S: 2 * (Ps + this.MARGIN) };
  },

  // Posición de la app j del anillo i, relativa al centro.
  itemPos(i, j, off, L, facing, scale) {
    let d = ((j - off) % L + L) % L;
    if (d >= L / 2) d -= L;
    const a = d * this.ringStep(i);
    const vis = Math.max(0, Math.min(1, (this.VISIBLE_ARC - Math.abs(a)) / 12));
    const rad = (facing + a) * Math.PI / 180;
    const r = this.ringR(i) * scale;
    return { x: Math.cos(rad) * r, y: Math.sin(rad) * r, a, vis };
  },

  ringAt(dist, n, scale) {
    for (let i = 0; i < n - 1; i++) {
      if (dist < (this.ringR(i) + this.ringR(i + 1)) / 2 * scale) return i;
    }
    return Math.max(0, n - 1);
  },

  local(dx, dy, facing) {
    const f = facing * Math.PI / 180;
    return { dist: Math.hypot(dx, dy), along: dx * Math.cos(f) + dy * Math.sin(f) };
  }
};
