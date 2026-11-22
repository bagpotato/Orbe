// Ventana "wheel": dibuja la media rueda y gira los anillos.
// Abrir y cerrar lo decide Rust (eventos orbe:open / orbe:close). Sin Tauri, modo demo.
(() => {
  const GEO = window.ORBE_GEO;
  const TAURI = window.__TAURI__;
  const DEMO = !TAURI;
  const reduce = matchMedia("(prefers-reduced-motion: reduce)").matches;

  const stage = document.getElementById("stage");
  const wheel = document.getElementById("wheel");
  const plate = document.getElementById("plate");
  const hub = document.getElementById("hub");
  const toast = document.getElementById("toast");

  let data = null;
  let rings = [];
  let facing = 0, scale = 1, S = 0;
  let open = false, reveal = 0, revealTarget = 0;
  let active = 0, acc = 0, raf = null;

  const DEMO_DATA = [
    ["Sistema", [["Ajustes", "gear"], ["Archivos", "folder"], ["Terminal", "terminal"], ["Monitor", "pulse"], ["Papelera", "trash"], ["Bloquear", "lock"]]],
    ["Trabajo", [["Código", "code"], ["Git", "git"], ["SSH", "server"], ["Máquinas virtuales", "vm"], ["Documentos", "notes"], ["Hojas de cálculo", "sheet"], ["Diseño", "pen"], ["Correo", "mail"], ["Calendario", "calendar"]]],
    ["Favoritos", [["Navegador", "globe"], ["Spotify", "music", "spotify"], ["Discord", "chat", "discord"], ["Fotos", "photo"], ["WhatsApp", "phone", "whatsapp"], ["Steam", "game", "steam"], ["Mapas", "map"], ["Calculadora", "calc"], ["Descargas", "download"], ["Notas", "notes"], ["Llamadas", "phone"], ["Podcasts", "mic"], ["Lector", "book"], ["Tiempo", "sun"]]]
  ];

  async function loadData() {
    if (!DEMO) return TAURI.core.invoke("get_wheel_data");
    return {
      anillos: DEMO_DATA.map(([nombre, apps]) => ({
        nombre,
        apps: apps.map(([n, icono, lib]) => ({ id: n.toLowerCase(), nombre: n, icono, img: lib ? `img/apps/${lib}.png` : null }))
      })),
      scroll_paso: 50,
      invertir: false
    };
  }

  function build(d) {
    data = d;
    rings = [];
    if (/^#[0-9a-f]{6}([0-9a-f]{2})?$/i.test(d.tinte || "")) plate.style.setProperty("--tint", d.tinte);
    plate.replaceChildren();
    wheel.querySelectorAll(".ring-name, .item").forEach(el => el.remove());
    d.anillos.slice(0, GEO.MAX_RINGS).forEach((anillo, ri) => {
      const track = document.createElement("div");
      track.className = "track";
      plate.appendChild(track);

      const label = document.createElement("div");
      label.className = "ring-name";
      label.textContent = anillo.nombre;
      wheel.appendChild(label);

      const els = anillo.apps.map(app => {
        const b = document.createElement("button");
        b.className = "item";
        b.tabIndex = -1;
        b.setAttribute("aria-label", app.nombre);
        if (app.img) {
          const img = document.createElement("img");
          img.src = app.img;
          img.alt = "";
          img.draggable = false;
          b.appendChild(img);
        } else {
          b.innerHTML = window.ORBE_GLYPH(app.icono); // glifo de nuestro propio archivo
        }
        const tip = document.createElement("span");
        tip.className = "tip";
        tip.textContent = app.nombre;
        b.appendChild(tip);
        b.addEventListener("click", () => launch(app));
        b.addEventListener("focus", () => setActive(ri));
        wheel.appendChild(b);
        return b;
      });
      rings.push({ off: 0, target: 0, els, track, label, n: anillo.apps.length });
    });
    setActive(Math.min(active, rings.length - 1));
  }

  function applyLayout() {
    const n = rings.length;
    const Ps = GEO.plateR(n) * scale;
    S = 2 * (Ps + GEO.MARGIN);
    stage.style.width = stage.style.height = S + "px";
    const circle = (el, r) => {
      el.style.width = el.style.height = 2 * r + "px";
      el.style.left = el.style.top = -r + "px";
    };
    circle(plate, Ps);
    circle(hub, GEO.HUB * scale);
    rings.forEach((ring, i) => {
      const r = GEO.ringR(i) * scale;
      ring.track.style.width = ring.track.style.height = 2 * r + "px";
      ring.track.style.left = ring.track.style.top = Ps - r + "px";
      const size = GEO.ringSize(i) * scale + "px";
      ring.els.forEach(b => { b.style.width = b.style.height = size; });
    });
  }

  function frame() {
    const k = reduce ? 1 : 0.2;
    let moving = false;
    reveal += (revealTarget - reveal) * k;
    if (Math.abs(revealTarget - reveal) < 0.002) reveal = revealTarget; else moving = true;
    const ease = 1 - Math.pow(1 - reveal, 3);
    const f = facing * Math.PI / 180;

    plate.style.transform = hub.style.transform = `rotate(${facing}deg) scale(${0.6 + 0.4 * ease})`;
    plate.style.opacity = hub.style.opacity = reveal;

    rings.forEach((ring, ri) => {
      ring.off += (ring.target - ring.off) * k;
      if (Math.abs(ring.target - ring.off) < 0.002) ring.off = ring.target; else moving = true;
      const half = GEO.ringSize(ri) * scale / 2;
      const grow = scale * (0.55 + 0.45 * ease);
      ring.els.forEach((b, j) => {
        const p = GEO.itemPos(ri, j, ring.off, ring.n, facing, grow);
        const vis = p.vis * reveal;
        b.style.transform = `translate(${p.x - half}px,${p.y - half}px) scale(${0.5 + 0.5 * vis})`;
        b.style.opacity = vis;
        const usable = open && vis > 0.5;
        b.style.pointerEvents = usable ? "auto" : "none";
        b.tabIndex = usable ? 0 : -1;
        b.dataset.ang = p.a;
      });
      // Etiqueta del anillo junto al diámetro, alternando lados.
      const lb = ring.label;
      const la = f + (ri % 2 ? 1 : -1) * Math.PI / 2;
      const lr = GEO.ringR(ri) * scale;
      const push = (facing === 0 || facing === 180 ? lb.offsetWidth : lb.offsetHeight) / 2 + 8;
      const lx = Math.cos(la) * lr + Math.cos(f) * push;
      const ly = Math.sin(la) * lr + Math.sin(f) * push;
      lb.style.transform = `translate(${lx}px,${ly}px) translate(-50%,-50%)`;
      lb.style.opacity = reveal;
    });

    raf = moving ? requestAnimationFrame(frame) : null;
    if (!moving && !open) wheel.classList.remove("live");
  }
  const kick = () => { if (!raf) raf = requestAnimationFrame(frame); };

  function openWheel(p) {
    facing = p.facing;
    scale = p.scale;
    if (!DEMO) { stage.style.left = "0px"; stage.style.top = "0px"; }
    applyLayout();
    open = true;
    revealTarget = 1;
    wheel.classList.add("live");
    wheel.setAttribute("aria-hidden", "false");
    kick();
  }

  function closeWheel() {
    open = false;
    revealTarget = 0;
    acc = 0;
    wheel.setAttribute("aria-hidden", "true");
    kick();
  }

  function setActive(i) {
    if (i < 0 || i >= rings.length) return;
    active = i;
    rings.forEach((r, k) => {
      r.track.classList.toggle("on", k === i);
      r.label.classList.toggle("on", k === i);
    });
  }

  function rotate(dir) {
    const ring = rings[active];
    if (!ring) return;
    ring.target += dir;
    kick();
  }

  async function launch(app) {
    if (DEMO) {
      toast.textContent = `Abriendo ${app.nombre}`;
      toast.classList.add("show");
      setTimeout(() => toast.classList.remove("show"), 1400);
      closeWheel();
      return;
    }
    try {
      await TAURI.core.invoke("launch_app", { id: app.id });
    } catch (e) {
      console.error(e);
    }
  }

  function ringUnder(e) {
    const r = stage.getBoundingClientRect();
    const { dist } = GEO.local(e.clientX - r.left - S / 2, e.clientY - r.top - S / 2, facing);
    return dist > GEO.HUB * scale ? GEO.ringAt(dist, rings.length, scale) : -1;
  }

  document.addEventListener("pointermove", e => {
    if (!open) return;
    const i = ringUnder(e);
    if (i >= 0 && i !== active) setActive(i);
  });

  document.addEventListener("wheel", e => {
    e.preventDefault();
    if (!open || !data) return;
    const i = ringUnder(e);
    if (i >= 0 && i !== active) { setActive(i); acc = 0; }
    // deltaMode 1 = líneas (ratón clásico en algunos sistemas)
    acc += e.deltaMode === 1 ? e.deltaY * 33 : e.deltaY;
    const paso = data.scroll_paso || 50;
    if (Math.abs(acc) >= paso) {
      rotate(Math.sign(acc) * (data.invertir ? -1 : 1));
      acc = 0;
    }
  }, { passive: false });

  function focusCenter() {
    const ring = rings[active];
    if (!ring) return;
    let best = ring.els[0], m = Infinity;
    ring.els.forEach(b => { const a = Math.abs(+b.dataset.ang); if (a < m) { m = a; best = b; } });
    best && best.focus();
  }

  document.addEventListener("keydown", e => {
    if (!open) return;
    const dir = { ArrowDown: 1, ArrowRight: 1, ArrowUp: -1, ArrowLeft: -1 }[e.key];
    if (!dir) return;
    e.preventDefault();
    rotate(dir);
    setTimeout(focusCenter, reduce ? 0 : 160);
  });

  // ---------- Modo demo ----------
  const EDGE = { 0: [0, 0.5], 180: [1, 0.5], 90: [0.5, 0], [-90]: [0.5, 1] };

  function demoPlace(f) {
    const [x, y] = EDGE[f];
    const W = innerWidth, H = innerHeight;
    const L = GEO.layout(W, H, x, y, rings.length, "auto");
    facing = f;
    scale = L.scale;
    stage.style.left = L.cx - L.S / 2 + "px";
    stage.style.top = L.cy - L.S / 2 + "px";
    openWheel({ facing, scale, size: L.S });
  }

  if (DEMO) {
    document.body.classList.add("demo");
    const bar = document.getElementById("demo-bar");
    bar.hidden = false;
    bar.querySelectorAll("[data-facing]").forEach(b => b.addEventListener("click", () => demoPlace(+b.dataset.facing)));
    document.getElementById("demo-open").addEventListener("click", () => {
      if (open) closeWheel(); else openWheel({ facing, scale, size: S });
    });
    document.getElementById("demo-bg").addEventListener("click", () => document.body.classList.toggle("light"));
    addEventListener("resize", () => {
      if (!data) return;
      const wasOpen = open;
      demoPlace(facing);
      if (!wasOpen) closeWheel();
    });
  } else {
    TAURI.event.listen("orbe:open", e => openWheel(e.payload));
    TAURI.event.listen("orbe:close", () => closeWheel());
    TAURI.event.listen("orbe:config-changed", async () => {
      try {
        build(await loadData());
        applyLayout();
        kick();
      } catch (e) { console.error(e); }
    });
  }

  loadData().then(d => {
    build(d);
    setActive(rings.length - 1);
    if (DEMO) demoPlace(0);
  }).catch(e => console.error(e));
})();
