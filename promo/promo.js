// Vídeo promocional de Orbe. Todo se dibuja en función del tiempo: renderAt(t) es determinista,
// así cada fotograma se puede capturar por separado (promo/render.mjs).
(() => {
  const GEO = window.ORBE_GEO;
  const $ = id => document.getElementById(id);
  const W = 1920, H = 1080;
  const VS = 1.45;                 // escala de la rueda en el vídeo
  const DURATION = 36;

  // ---------- utilidades ----------
  const clamp = (v, a = 0, b = 1) => Math.min(Math.max(v, a), b);
  const lerp = (a, b, k) => a + (b - a) * k;
  const seg = (t, a, b) => clamp((t - a) / (b - a));
  const easeIO = k => (k < 0.5 ? 4 * k * k * k : 1 - Math.pow(-2 * k + 2, 3) / 2);
  const easeOut = k => 1 - Math.pow(1 - k, 3);
  const backOut = k => { const c = 1.6; return 1 + (c + 1) * Math.pow(k - 1, 3) + c * Math.pow(k - 1, 2); };
  /** 0→1 entre a y b, 1 hasta c, 1→0 hasta d. */
  const inOut = (t, a, b, c, d) => (t < c ? easeOut(seg(t, a, b)) : 1 - easeIO(seg(t, c, d)));

  // ---------- datos de la rueda ----------
  const RINGS = [
    ["Sistema", [["Ajustes", "gear"], ["Archivos", "folder"], ["Terminal", "terminal"], ["Monitor", "pulse"], ["Papelera", "trash"], ["Bloquear", "lock"]]],
    ["Trabajo", [["Código", "code"], ["GitHub", "", "github"], ["Claude", "", "claude"], ["Notas", "notes"], ["Calculadora", "calc"], ["Correo", "mail"], ["Calendario", "calendar"], ["Diseño", "pen"], ["Documentos", "book"]]],
    ["Favoritos", [["Spotify", "", "spotify"], ["Discord", "", "discord"], ["Steam", "", "steam"], ["WhatsApp", "", "whatsapp"], ["Navegador", "globe"], ["Fotos", "photo"], ["Música", "music"], ["Mapas", "map"], ["Juegos", "game"], ["Vídeo", "video"], ["IA", "", "ai"], ["Descargas", "download"], ["Tiempo", "sun"], ["Chat", "chat"]]]
  ];

  const wheel = $("wheel"), plate = $("plate"), hub = $("hub");
  const rings = RINGS.map(([nombre, apps], ri) => {
    const track = document.createElement("div");
    track.className = "track";
    plate.appendChild(track);
    const label = document.createElement("div");
    label.className = "ring-name";
    label.textContent = nombre;
    wheel.appendChild(label);
    const els = apps.map(([n, glyph, lib]) => {
      const b = document.createElement("div");
      b.className = "item";
      b.innerHTML = lib ? `<img src="../src/img/apps/${lib}.png" alt="">` : window.ORBE_GLYPH(glyph);
      const tip = document.createElement("span");
      tip.className = "tip";
      tip.textContent = n;
      b.appendChild(tip);
      b.dataset.name = n;
      wheel.appendChild(b);
      return b;
    });
    return { track, label, els, n: apps.length };
  });

  // Giros de cada anillo: [tiempo, dirección]. Posición base al empezar.
  const BASE = [0, 0, 3];
  const TICKS = [
    [],
    [[13.9, 1], [14.4, 1]],
    [[11.6, -1], [12.1, -1], [12.6, -1]]
  ];
  const ringOff = (ri, t) => TICKS[ri].reduce((o, [at, d]) => o + d * easeOut(seg(t, at, at + 0.38)), BASE[ri]);

  // ---------- cursor ----------
  // Fotogramas clave [t, x, y]; entre ellos, easeIO.
  const CURSOR = [
    [4.5, 1250, 640], [5.2, 1250, 640], [6.6, 4, 540], [7.7, 4, 540], [8.7, 200, 640], [10.2, 228, 606],
    [11.2, 292, 548], [12.9, 292, 548], [13.6, 214, 566], [15.6, 214, 566],
    [16.8, 296, 540], [17.6, 296, 540], [18.7, 1120, 880], [20.3, 1120, 880],
    [21.2, 8, 540], [21.5, 8, 540],
    // arrastre de la zona (21.5 → 23.3), definido aparte
    [23.3, 960, 1072], [23.6, 960, 1072], [24.3, 1232, 226], [24.7, 1232, 226], [25.3, 960, 1078], [26.8, 960, 1078]
  ];
  const DRAG = [21.5, 23.3];
  function dragPos(t) {
    const k = easeIO(seg(t, DRAG[0], DRAG[1]));
    // arco: primero baja por el borde izquierdo, luego cruza hacia el centro inferior
    const x = lerp(8, 960, Math.pow(k, 1.6));
    const y = lerp(540, 1072, Math.sin(k * Math.PI / 2));
    return [x, y];
  }
  function cursorAt(t) {
    if (t >= DRAG[0] && t <= DRAG[1]) return dragPos(t);
    for (let i = 0; i < CURSOR.length - 1; i++) {
      const [t0, x0, y0] = CURSOR[i], [t1, x1, y1] = CURSOR[i + 1];
      if (t >= t0 && t <= t1) {
        if (t0 === DRAG[0]) return dragPos(t);
        const k = easeIO(seg(t, t0, t1));
        return [lerp(x0, x1, k), lerp(y0, y1, k)];
      }
    }
    const last = CURSOR[CURSOR.length - 1];
    return t < CURSOR[0][0] ? [CURSOR[0][1], CURSOR[0][2]] : [last[1], last[2]];
  }

  // ---------- logo ----------
  const LOGO_SVG = `<svg viewBox="35 112 800 800">
    <defs><linearGradient id="lg-glass" x1="0" x2="1"><stop offset="0" stop-color="#fff" stop-opacity=".12"/><stop offset="1" stop-color="#fff" stop-opacity=".34"/></linearGradient></defs>
    <path class="lp" d="M250 142 A370 370 0 0 1 250 882 Z" fill="url(#lg-glass)" stroke="#fff" stroke-opacity=".8" stroke-width="14"/>
    <path class="la" d="M250 252 A260 260 0 0 1 250 772" fill="none" stroke="#fff" stroke-opacity=".4" stroke-width="8" pathLength="1" stroke-dasharray="1"/>
    <path class="la" d="M250 372 A140 140 0 0 1 250 652" fill="none" stroke="#fff" stroke-opacity=".4" stroke-width="8" pathLength="1" stroke-dasharray="1"/>
    <path class="lh" d="M250 432 A80 80 0 0 1 250 592 Z" fill="#fff" fill-opacity=".9"/>
    <circle class="ld" cx="555" cy="512" r="58" fill="#5B7CFA" stroke="#fff" stroke-width="10"/>
    <circle class="ld" cx="514" cy="340" r="48" fill="#9BE3D6" stroke="#fff" stroke-width="10"/>
    <circle class="ld" cx="514" cy="684" r="48" fill="#F2A7C3" stroke="#fff" stroke-width="10"/>
    <circle class="ld" cx="380" cy="512" r="42" fill="#fff"/>
    <circle class="ld" cx="355" cy="415" r="36" fill="#B9A7F2" stroke="#fff" stroke-width="8"/>
    <circle class="ld" cx="355" cy="609" r="36" fill="#9BE3D6" stroke="#fff" stroke-width="8"/>
  </svg>`;
  $("intro-logo").innerHTML = LOGO_SVG;
  $("outro-logo").innerHTML = LOGO_SVG;

  function drawLogo(el, t0, t) {
    const s = el.querySelector("svg");
    const k = seg(t, t0, t0 + 0.7);
    s.style.opacity = clamp(k * 2);
    s.style.transform = `scale(${lerp(0.55, 1, backOut(k))}) rotate(${lerp(-25, 0, easeOut(k))}deg)`;
    s.querySelectorAll(".la").forEach((p, i) => { p.style.strokeDashoffset = 1 - easeOut(seg(t, t0 + 0.35 + i * 0.12, t0 + 1.0 + i * 0.12)); });
    s.querySelectorAll(".ld").forEach((c, i) => {
      const kk = backOut(seg(t, t0 + 0.55 + i * 0.07, t0 + 0.95 + i * 0.07));
      c.style.transformOrigin = `${c.getAttribute("cx")}px ${c.getAttribute("cy")}px`;
      c.style.transform = `scale(${Math.max(0, kk)})`;
    });
  }

  // Letras del título de la intro
  $("intro-word").innerHTML = [..."Orbe"].map(ch => `<span>${ch}</span>`).join("");

  // ---------- funciones (escena 6) ----------
  const LIB = ["spotify", "discord", "steam", "whatsapp", "github", "claude", "ai"];
  const featIcons = LIB.map((id, i) => {
    const g = document.createElement("div");
    g.className = "glass";
    g.style.left = 315 + i * 190 + "px";
    g.innerHTML = `<img src="../src/img/apps/${id}.png" alt="">`;
    $("feat-icons").appendChild(g);
    return g;
  });
  const CHIPS = [
    ['<path d="M12 3a9 9 0 0 1 0 18M12 7a5 5 0 0 1 0 10"/><circle cx="12" cy="12" r="1.5"/>', "Hasta 4 anillos"],
    [window.ORBE_GLYPHS.app, "Iconos reales de Windows"],
    [window.ORBE_GLYPHS.image, "Tus propias imágenes"],
    [window.ORBE_GLYPHS.shield, "No roba el foco"],
    [window.ORBE_GLYPHS.game, "Se aparta en pantalla completa"],
    [window.ORBE_GLYPHS.vm, "Varios monitores y HiDPI"]
  ];
  const chips = CHIPS.map(([svg, text]) => {
    const c = document.createElement("div");
    c.className = "chip";
    c.innerHTML = `<svg viewBox="0 0 24 24">${svg}</svg>`;
    c.append(text);
    $("feat-chips").appendChild(c);
    return c;
  });

  // ---------- rótulos ----------
  const CAPTIONS = [
    [5.3, 10.1, "Escondida", "Pasa el ratón por el borde y aparece"],
    [10.8, 15.7, "Anillos", "Cada anillo gira con la rueda del ratón"],
    [16.2, 19.9, "Abrir", "Un clic y a lo tuyo"],
    [20.6, 22.7, "Tu sitio", "Colócala en cualquier borde y monitor"]
  ];

  // ---------- rueda ----------
  const WHEEL_STATES = [
    // [desde, cx, cy, facing]
    [0, 0, 540, 0],
    [24.8, 960, 1080, -90]
  ];
  function wheelPose(t) {
    let p = WHEEL_STATES[0];
    for (const s of WHEEL_STATES) if (t >= s[0]) p = s;
    return { cx: p[1], cy: p[2], facing: p[3] };
  }
  function wheelReveal(t) {
    if (t < 17.25) return easeOut(seg(t, 6.85, 7.6));
    if (t < 25.3) return 1 - easeIO(seg(t, 17.25, 17.85));
    return easeOut(seg(t, 25.35, 26.1));
  }

  function drawWheel(t, cur) {
    const { cx, cy, facing } = wheelPose(t);
    const reveal = wheelReveal(t);
    wheel.style.display = reveal > 0.001 ? "" : "none";
    if (reveal <= 0.001) return null;
    wheel.style.left = cx + "px";
    wheel.style.top = cy + "px";
    const ease = 1 - Math.pow(1 - reveal, 3);
    const Ps = GEO.plateR(3) * VS;
    const circle = (el, r) => { el.style.width = el.style.height = 2 * r + "px"; el.style.left = el.style.top = -r + "px"; };
    circle(plate, Ps);
    circle(hub, GEO.HUB * VS);
    plate.style.transform = hub.style.transform = `rotate(${facing}deg) scale(${0.6 + 0.4 * ease})`;
    plate.style.opacity = hub.style.opacity = reveal;

    // anillo activo y app bajo el cursor
    const loc = GEO.local(cur[0] - cx, cur[1] - cy, facing);
    const active = loc.dist > GEO.HUB * VS ? GEO.ringAt(loc.dist, 3, VS) : 2;
    let hovered = null;
    const f = facing * Math.PI / 180;

    rings.forEach((ring, ri) => {
      const r = GEO.ringR(ri) * VS;
      ring.track.style.width = ring.track.style.height = 2 * r + "px";
      ring.track.style.left = ring.track.style.top = Ps - r + "px";
      ring.track.classList.toggle("on", ri === active);
      ring.label.classList.toggle("on", ri === active);
      const size = GEO.ringSize(ri) * VS;
      const off = ringOff(ri, t);
      ring.els.forEach((b, j) => {
        const p = GEO.itemPos(ri, j, off, ring.n, facing, VS * (0.55 + 0.45 * ease));
        const vis = p.vis * reveal;
        const isHover = reveal > 0.9 && vis > 0.6 && Math.hypot(cx + p.x - cur[0], cy + p.y - cur[1]) < size / 2;
        if (isHover) hovered = b;
        b.classList.toggle("hover", isHover);
        let s = 0.5 + 0.5 * vis;
        if (isHover && b.dataset.name === "Spotify") s *= 1 - 0.12 * Math.sin(Math.PI * seg(t, 17.0, 17.18));
        b.style.width = b.style.height = size + "px";
        b.style.transform = `translate(${p.x - size / 2}px,${p.y - size / 2}px) scale(${s})`;
        b.style.opacity = vis;
      });
      const lb = ring.label;
      const la = f + (ri % 2 ? 1 : -1) * Math.PI / 2;
      const push = (facing === 0 || facing === 180 ? lb.offsetWidth : lb.offsetHeight) / 2 + 10;
      lb.style.transform = `translate(${Math.cos(la) * r + Math.cos(f) * push}px,${Math.sin(la) * r + Math.sin(f) * push}px) translate(-50%,-50%)`;
      lb.style.opacity = reveal;
    });
    return hovered;
  }

  // Ángulo del fantasma del modo colocar: suavizado simulando paso a paso desde el inicio.
  function ghostState(t) {
    const t0 = 20.2;
    let ang = 0;
    let L = null;
    for (let s = t0; s <= t + 1e-6; s += 1 / 30) {
      const c = cursorAt(Math.min(s, 23.3));
      const x = clamp(c[0] / W), y = clamp(c[1] / H);
      L = GEO.layout(W / VS, H / VS, x, y, 3, "auto");
      let d = L.facing - ang;
      while (d > 180) d -= 360;
      while (d < -180) d += 360;
      ang += d * 0.22;
    }
    return { L, ang };
  }

  // ---------- fotograma ----------
  function renderAt(t) {
    // fondo con manchas que derivan despacio
    const blobs = [[0.18, 0.3, 0.9], [0.82, 0.25, 1.1], [0.6, 0.9, 0.8], [0.35, 0.8, 1.3]];
    document.querySelectorAll(".blob").forEach((b, i) => {
      const [bx, by, sp] = blobs[i];
      const x = bx * W + Math.sin(t * 0.21 * sp + i) * 120 - b.offsetWidth / 2;
      const y = by * H + Math.cos(t * 0.17 * sp + i * 2) * 90 - b.offsetHeight / 2;
      b.style.transform = `translate(${x}px,${y}px)`;
    });

    // 1 · intro
    const introO = 1 - easeIO(seg(t, 3.9, 4.5));
    $("intro").style.opacity = introO;
    $("intro").style.display = introO > 0 ? "" : "none";
    if (introO > 0) {
      drawLogo($("intro-logo"), 0.2, t);
      $("intro-word").querySelectorAll("span").forEach((s, i) => {
        const k = seg(t, 0.75 + i * 0.09, 1.3 + i * 0.09);
        s.style.opacity = easeOut(k);
        s.style.transform = `translateY(${lerp(60, 0, easeOut(k))}px)`;
      });
      const kt = easeOut(seg(t, 1.6, 2.3));
      $("intro-tag").style.opacity = kt;
      $("intro-tag").style.transform = `translateY(${lerp(24, 0, kt)}px)`;
    }

    // 2–5 · escritorio
    const deskO = easeOut(seg(t, 4.4, 5.0)) * (1 - easeIO(seg(t, 26.8, 27.4)));
    const desk = $("desk");
    desk.style.display = deskO > 0 ? "" : "none";
    desk.style.opacity = deskO;

    const cur = cursorAt(t);
    let hovered = null;
    if (deskO > 0) {
      // ventana de Spotify
      const sp = easeOut(seg(t, 17.5, 18.2)) * (1 - easeIO(seg(t, 20.0, 20.4)));
      const sw = $("spotify");
      sw.style.display = sp > 0 ? "" : "none";
      sw.style.opacity = sp;
      sw.style.transform = `translateY(${lerp(30, 0, sp)}px) scale(${lerp(0.93, 1, sp)})`;
      $("progress").style.width = 100 * easeIO(seg(t, 18.2, 20.0)) * 0.42 + "%";

      // modo colocar
      const pl = easeOut(seg(t, 20.2, 20.7)) * (1 - easeIO(seg(t, 24.25, 24.65)));
      ["shade", "ghost", "zone", "card"].forEach(id => { $(id).style.display = pl > 0 ? "" : "none"; $(id).style.opacity = pl; });
      if (pl > 0) {
        const zc = t < DRAG[0] ? [0, 540] : t <= DRAG[1] ? dragPos(t) : [960, 1072];
        $("zone").style.left = zc[0] + "px";
        $("zone").style.top = zc[1] + "px";
        const pulse = (t * 0.7) % 1;
        $("zone").style.setProperty("--pulse", lerp(0.8, 1.6, pulse));
        $("zone").style.setProperty("--pulse-o", 1 - pulse);
        const { L, ang } = ghostState(t);
        const gx = L.cx * VS, gy = L.cy * VS, gr = L.Ps * VS;
        const g = $("ghost");
        g.style.width = g.style.height = 2 * gr + "px";
        g.style.transform = `translate(${gx - gr}px,${gy - gr}px) rotate(${ang}deg)`;
        const press = Math.sin(Math.PI * seg(t, 24.05, 24.25));
        $("save-btn").style.transform = `scale(${1 - 0.08 * press})`;
        $("card").style.transform = `translateY(${lerp(-30, 0, easeOut(seg(t, 20.2, 20.7)))}px)`;
      }

      hovered = drawWheel(t, cur);

      // onda del clic
      const rp = seg(t, 17.0, 17.6);
      const rip = $("ripple");
      rip.style.opacity = rp > 0 && rp < 1 ? 1 - rp : 0;
      rip.style.left = "296px";
      rip.style.top = "540px";
      rip.style.transform = `scale(${lerp(0.3, 1.6, easeOut(rp))})`;
    }

    // 6 · funciones
    const fo = easeOut(seg(t, 26.9, 27.5)) * (1 - easeIO(seg(t, 31.3, 31.9)));
    $("features").style.display = fo > 0 ? "" : "none";
    $("features").style.opacity = fo;
    if (fo > 0) {
      const kt = easeOut(seg(t, 27.0, 27.6));
      $("feat-title").style.transform = `translateY(${lerp(40, 0, kt)}px)`;
      $("feat-title").style.opacity = kt;
      featIcons.forEach((g, i) => {
        const k = seg(t, 27.3 + i * 0.09, 27.85 + i * 0.09);
        const bob = Math.sin(t * 1.6 + i * 0.9) * 8;
        g.style.opacity = clamp(k * 2);
        g.style.transform = `translateY(${bob + lerp(60, 0, easeOut(k))}px) scale(${Math.max(0, backOut(k))})`;
      });
      chips.forEach((c, i) => {
        const k = easeOut(seg(t, 28.3 + i * 0.14, 28.8 + i * 0.14));
        c.style.opacity = k;
        c.style.transform = `translateY(${lerp(30, 0, k)}px)`;
      });
    }

    // 7 · cierre
    const oo = easeOut(seg(t, 31.8, 32.3));
    $("outro").style.display = oo > 0 ? "" : "none";
    $("outro").style.opacity = oo;
    if (oo > 0) {
      drawLogo($("outro-logo"), 31.9, t);
      [["#outro h1", 32.3], ["#outro .url", 32.7], ["#outro .meta", 33.0]].forEach(([sel, at]) => {
        const k = easeOut(seg(t, at, at + 0.6));
        const el = document.querySelector(sel);
        el.style.opacity = k;
        el.style.transform = `translateY(${lerp(30, 0, k)}px)`;
      });
    }

    // rótulo
    let capO = 0;
    for (const [a, d, k, txt] of CAPTIONS) {
      if (t >= a && t <= d) {
        capO = inOut(t, a, a + 0.5, d - 0.4, d);
        $("cap-k").textContent = k;
        $("cap-t").textContent = txt;
      }
    }
    const cap = $("caption");
    cap.style.opacity = capO;
    cap.style.transform = `translateY(${lerp(24, 0, capO)}px)`;

    // cursor
    const cOn = deskO * (t < 26.6 ? 1 : 0);
    const cursor = $("cursor");
    cursor.style.opacity = cOn;
    cursor.style.transform = `translate(${cur[0] - 8}px,${cur[1] - 5}px)`; // punta de la flecha en cur

    // indicador de scroll
    const sh = Math.max(inOut(t, 11.4, 11.6, 12.8, 13.0), inOut(t, 13.7, 13.9, 14.7, 14.9));
    const hint = $("scroll-hint");
    hint.style.opacity = sh;
    hint.style.transform = `translate(${cur[0] + 34}px,${cur[1] + 18}px)`;
    hint.querySelectorAll("i").forEach((a, i) => { a.style.opacity = 0.35 + 0.65 * Math.abs(Math.sin(t * 6 - i)); });

    // fundido de entrada y salida
    $("fade").style.opacity = Math.max(1 - easeOut(seg(t, 0, 0.6)), easeIO(seg(t, 35.3, 36)));
    return hovered ? hovered.dataset.name : null;
  }

  async function ready() {
    await document.fonts.ready;
    await Promise.all([...document.images].map(img => img.complete ? 0 : new Promise(r => { img.onload = img.onerror = r; })));
    renderAt(0);
    return true;
  }

  window.PROMO = { renderAt, ready, DURATION };

  // Vista previa en el navegador: ?t=12.3 congela ese instante; sin parámetro, reproduce en bucle.
  const q = new URLSearchParams(location.search);
  if (!q.has("capture")) {
    ready().then(() => {
      if (q.has("t")) { renderAt(Number(q.get("t"))); return; }
      const start = performance.now();
      const loop = now => { renderAt(((now - start) / 1000) % DURATION); requestAnimationFrame(loop); };
      requestAnimationFrame(loop);
    });
  }
})();
