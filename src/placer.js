// Ventana "placer": ocupa un monitor entero; se arrastra la zona y se guarda.
(() => {
  const GEO = window.ORBE_GEO;
  const TAURI = window.__TAURI__;
  const invoke = TAURI ? TAURI.core.invoke : demoInvoke;

  const zone = document.getElementById("zone");
  const ghost = document.getElementById("ghost");
  const msg = document.getElementById("msg");
  const next = document.getElementById("next");

  let info = null;
  let x = 0, y = 0.5;
  const SNAP_PX = 24;

  async function demoInvoke(cmd) {
    if (cmd === "get_placer_info" || cmd === "placer_next_monitor") {
      return { monitor: "DEMO", x: 0, y: 0.5, tamano: 24, index: 0, count: 1 };
    }
    msg.textContent = cmd === "save_zone"
      ? `Guardaría x = ${x.toFixed(3)}, y = ${y.toFixed(3)} (modo demo).`
      : "Cancelado (modo demo).";
  }

  function place() {
    const W = innerWidth, H = innerHeight;
    const size = Math.max(info ? info.tamano : 24, 28);
    zone.style.width = zone.style.height = size + "px";
    zone.style.left = x * W + "px";
    zone.style.top = y * H + "px";
    // Que la etiqueta no se salga de la pantalla en los bordes.
    zone.classList.toggle("edge-l", x < 0.06);
    zone.classList.toggle("edge-r", x > 0.94);
    zone.classList.toggle("edge-b", y > 0.9);
    const L = GEO.layout(W, H, x, y, 3, "auto");
    ghost.style.width = ghost.style.height = 2 * L.Ps + "px";
    ghost.style.transform = `translate(${L.cx - L.Ps}px, ${L.cy - L.Ps}px) rotate(${L.facing}deg)`;
  }

  function setFromPx(px, py) {
    const W = innerWidth, H = innerHeight;
    if (px < SNAP_PX) px = 0; else if (W - px < SNAP_PX) px = W;
    if (py < SNAP_PX) py = 0; else if (H - py < SNAP_PX) py = H;
    x = Math.min(Math.max(px / W, 0), 1);
    y = Math.min(Math.max(py / H, 0), 1);
    place();
  }

  function showInfo(i) {
    info = i;
    x = i.x;
    y = i.y;
    next.hidden = i.count < 2;
    next.textContent = `Siguiente monitor (${i.index + 1}/${i.count})`;
    place();
  }

  async function call(cmd, args) {
    try {
      return await invoke(cmd, args);
    } catch (e) {
      msg.textContent = "Error: " + e;
      return null;
    }
  }

  const save = () => call("save_zone", { monitor: info.monitor, x, y });
  const cancel = () => call("cancel_placing");

  // Arrastre
  let dragging = false;
  zone.addEventListener("pointerdown", e => {
    dragging = true;
    zone.classList.add("dragging");
    zone.setPointerCapture(e.pointerId);
    e.preventDefault();
  });
  zone.addEventListener("pointermove", e => { if (dragging) setFromPx(e.clientX, e.clientY); });
  const end = () => { dragging = false; zone.classList.remove("dragging"); };
  zone.addEventListener("pointerup", end);
  zone.addEventListener("pointercancel", end);

  // Teclado
  document.addEventListener("keydown", e => {
    if (e.key === "Escape") { e.preventDefault(); cancel(); return; }
    if (e.key === "Enter" && document.activeElement.tagName !== "BUTTON") { e.preventDefault(); save(); return; }
    if (document.activeElement !== zone) return;
    const d = e.shiftKey ? 0.05 : 0.01;
    const mv = { ArrowLeft: [-d, 0], ArrowRight: [d, 0], ArrowUp: [0, -d], ArrowDown: [0, d] }[e.key];
    if (!mv) return;
    e.preventDefault();
    x = Math.min(Math.max(x + mv[0], 0), 1);
    y = Math.min(Math.max(y + mv[1], 0), 1);
    place();
  });

  document.getElementById("save").addEventListener("click", save);
  document.getElementById("cancel").addEventListener("click", cancel);
  next.addEventListener("click", async () => {
    const i = await call("placer_next_monitor");
    if (i) showInfo(i);
  });
  addEventListener("resize", () => info && place());

  if (!TAURI) document.body.classList.add("demo");
  call("get_placer_info").then(i => {
    if (i) showInfo(i);
    zone.focus();
  });
})();
