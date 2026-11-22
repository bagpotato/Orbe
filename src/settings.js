// Ventana "settings": editor de anillos y ajustes generales.
(() => {
  const TAURI = window.__TAURI__;
  const invoke = TAURI ? TAURI.core.invoke : demoInvoke;
  const MAX_RINGS = 4;
  const $ = id => document.getElementById(id);

  let cfg = null;        // copia de trabajo (mismo JSON que Rust)
  let saved = "";        // JSON de la última versión guardada
  let installed = [];
  let selected = null;   // id de la app seleccionada
  const iconCache = new Map();

  // ---------- modo demo (navegador sin Tauri) ----------
  function demoConfig() {
    const apps = {
      ajustes: { nombre: "Ajustes", ruta: "ms-settings:", args: "", icono: "gear" },
      archivos: { nombre: "Archivos", ruta: "explorer.exe", args: "", icono: "folder" },
      terminal: { nombre: "Terminal", ruta: "wt.exe", args: "", icono: "terminal" },
      notas: { nombre: "Bloc de notas", ruta: "notepad.exe", args: "", icono: "notes" },
      calculadora: { nombre: "Calculadora", ruta: "calc.exe", args: "", icono: "calc" },
      navegador: { nombre: "Navegador", ruta: "https://duckduckgo.com", args: "", icono: "globe" },
      musica: { nombre: "Música", ruta: "mswindowsmusic:", args: "", icono: "music" },
      discord: { nombre: "Discord", ruta: "discord.lnk", args: "", icono: "chat", imagen: "lib:discord" }
    };
    return {
      zona: { monitor: "", x: 0, y: 0.5, tamano: 24, orientacion: "auto", retardo_ms: 120, gracia_ms: 250 },
      rueda: { atajo: "Super+Space", scroll_paso: 50, invertir: false },
      aspecto: { tinte: "#5B7CFA1F", tema: "auto" },
      anillo: [
        { nombre: "Sistema", apps: ["ajustes", "archivos", "terminal"] },
        { nombre: "Trabajo", apps: ["notas", "calculadora"] },
        { nombre: "Favoritos", apps: ["navegador", "musica", "discord"] }
      ],
      apps
    };
  }
  let demoSaved = null;
  async function demoInvoke(cmd, args) {
    switch (cmd) {
      case "get_config": return JSON.parse(demoSaved || JSON.stringify(demoConfig()));
      case "save_config": demoSaved = JSON.stringify(args.config); return null;
      case "list_installed_apps":
        return ["Firefox", "Visual Studio Code", "Spotify", "Discord", "GIMP", "Steam", "OBS Studio", "VLC media player"]
          .map(n => ({ nombre: n, ruta: `C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\${n}.lnk` }));
      case "get_icon": return null;
      case "get_app_image": {
        const im = args.imagen || "";
        if (im.startsWith("lib:")) return `img/apps/${im.slice(4)}.png`;
        return im.startsWith("data:") ? im : null;
      }
      case "import_image": return args.data; // en demo, la propia data URL
      default: return null;
    }
  }

  // ---------- utilidades ----------
  let statusTimer = null;
  function status(text, isError = false) {
    const s = $("status");
    s.textContent = text;
    s.classList.toggle("error", isError);
    s.classList.add("show");
    clearTimeout(statusTimer);
    statusTimer = setTimeout(() => s.classList.remove("show"), isError ? 5000 : 2500);
  }

  const isDirty = () => cfg && JSON.stringify(cfg) !== saved;
  function markDirty() { $("save").classList.toggle("dirty", isDirty()); }

  function makeId(nombre) {
    const base = (nombre || "app").toLowerCase()
      .normalize("NFD").replace(/[\u0300-\u036f]/g, "")
      .replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "") || "app";
    let id = base, n = 2;
    while (cfg.apps[id]) id = `${base}_${n++}`;
    return id;
  }

  // Imagen elegida > icono del programa > glifo. Cacheado por (imagen, ruta).
  function fillIcon(el, app) {
    el.classList.remove("has-img");
    el.innerHTML = window.ORBE_GLYPH(app.icono); // glifo propio, no texto de usuario
    const ruta = (app.ruta || "").trim();
    const imagen = (app.imagen || "").trim();
    const key = imagen + "|" + ruta;
    el.dataset.key = key; // el mismo elemento (p. ej. la vista previa) se reutiliza para otras apps
    if (!ruta && !imagen) return;
    if (!iconCache.has(key)) iconCache.set(key, invoke("get_app_image", { imagen, ruta }).catch(() => null));
    iconCache.get(key).then(src => {
      if (!src || el.dataset.key !== key) return;
      const img = document.createElement("img");
      img.src = src;
      img.alt = "";
      el.replaceChildren(img);
      el.classList.add("has-img");
    });
  }

  function iconEl(app) {
    const span = document.createElement("span");
    span.className = "icon";
    fillIcon(span, app);
    return span;
  }

  function addApp(app, ringIndex) {
    const id = makeId(app.nombre);
    cfg.apps[id] = {
      nombre: app.nombre, ruta: app.ruta, args: app.args || "", icono: app.icono || "app",
      imagen: window.ORBE_LIBRARY_MATCH(app.nombre)
    };
    const ring = cfg.anillo[Math.min(ringIndex, cfg.anillo.length - 1)];
    ring.apps.push(id);
    selected = id;
    renderAll();
    status(`«${app.nombre}» añadida a ${ring.nombre}`);
  }

  function removeFromRings(id) {
    cfg.anillo.forEach(r => { r.apps = r.apps.filter(x => x !== id); });
  }

  // ---------- lista de apps instaladas ----------
  function renderInstalled() {
    const ul = $("installed");
    const q = $("search").value.trim().toLowerCase();
    const list = installed.filter(a => a.nombre.toLowerCase().includes(q));
    ul.replaceChildren();
    if (!list.length) {
      const li = document.createElement("li");
      li.className = "muted";
      li.textContent = installed.length ? "Sin resultados" : "No se encontraron apps";
      ul.appendChild(li);
      return;
    }
    list.slice(0, 300).forEach(a => {
      const li = document.createElement("li");
      li.draggable = true;
      li.title = a.ruta;
      const name = document.createElement("span");
      name.className = "name";
      name.textContent = a.nombre;
      const add = document.createElement("button");
      add.className = "icon-btn";
      add.textContent = "+";
      add.setAttribute("aria-label", `Añadir ${a.nombre} al último anillo`);
      add.addEventListener("click", () => addApp({ ...a, icono: "app" }, cfg.anillo.length - 1));
      li.append(iconEl({ ...a, icono: "app", imagen: window.ORBE_LIBRARY_MATCH(a.nombre) }), name, add);
      li.addEventListener("dragstart", e => {
        e.dataTransfer.setData("text/orbe-new", JSON.stringify(a));
        e.dataTransfer.effectAllowed = "copy";
      });
      ul.appendChild(li);
    });
  }

  // ---------- anillos ----------
  function dropTarget(tiles, ri) {
    tiles.addEventListener("dragover", e => {
      const types = e.dataTransfer.types;
      if (!types.includes("text/orbe-new") && !types.includes("text/orbe-move")) return;
      e.preventDefault();
      tiles.classList.add("drop-over");
      tiles.querySelectorAll(".insert-before").forEach(t => t.classList.remove("insert-before"));
      const tile = e.target.closest(".tile");
      if (tile && !tile.classList.contains("dragging")) tile.classList.add("insert-before");
    });
    tiles.addEventListener("dragleave", e => {
      if (!tiles.contains(e.relatedTarget)) {
        tiles.classList.remove("drop-over");
        tiles.querySelectorAll(".insert-before").forEach(t => t.classList.remove("insert-before"));
      }
    });
    tiles.addEventListener("drop", e => {
      e.preventDefault();
      tiles.classList.remove("drop-over");
      const before = e.target.closest(".tile")?.dataset.id;
      const ring = cfg.anillo[ri];
      const insert = id => {
        const at = before ? ring.apps.indexOf(before) : -1;
        if (at < 0) ring.apps.push(id); else ring.apps.splice(at, 0, id);
      };
      const nueva = e.dataTransfer.getData("text/orbe-new");
      const mover = e.dataTransfer.getData("text/orbe-move");
      if (nueva) {
        const a = JSON.parse(nueva);
        const id = makeId(a.nombre);
        cfg.apps[id] = { nombre: a.nombre, ruta: a.ruta, args: "", icono: "app", imagen: window.ORBE_LIBRARY_MATCH(a.nombre) };
        insert(id);
        selected = id;
      } else if (mover) {
        const { id } = JSON.parse(mover);
        if (id === before) return;
        removeFromRings(id);
        insert(id);
        selected = id;
      }
      renderAll();
    });
  }

  function renderRings() {
    const box = $("rings");
    box.replaceChildren();
    cfg.anillo.forEach((ring, ri) => {
      const card = document.createElement("div");
      card.className = "ring";

      const head = document.createElement("div");
      head.className = "ring-head";
      const name = document.createElement("input");
      name.value = ring.nombre;
      name.setAttribute("aria-label", `Nombre del anillo ${ri + 1}`);
      name.addEventListener("input", () => { ring.nombre = name.value; markDirty(); });
      const count = document.createElement("span");
      count.className = "ring-count";
      count.textContent = `${ring.apps.length} ${ring.apps.length === 1 ? "app" : "apps"} · ${["interior", "segundo", "tercero", "cuarto"][ri]}`;
      const btn = (label, aria, disabled, fn) => {
        const b = document.createElement("button");
        b.className = "icon-btn";
        b.textContent = label;
        b.setAttribute("aria-label", aria);
        b.title = aria;
        b.disabled = disabled;
        b.addEventListener("click", fn);
        return b;
      };
      const swap = (a, b) => { [cfg.anillo[a], cfg.anillo[b]] = [cfg.anillo[b], cfg.anillo[a]]; renderAll(); };
      head.append(
        name, count,
        btn("↑", "Mover hacia dentro", ri === 0, () => swap(ri, ri - 1)),
        btn("↓", "Mover hacia fuera", ri === cfg.anillo.length - 1, () => swap(ri, ri + 1)),
        btn("✕", "Eliminar anillo", cfg.anillo.length <= 1, () => {
          if (ring.apps.length && !confirm(`¿Eliminar el anillo «${ring.nombre}» y sus ${ring.apps.length} apps?`)) return;
          cfg.anillo.splice(ri, 1);
          renderAll();
        })
      );

      const tiles = document.createElement("div");
      tiles.className = "tiles";
      if (!ring.apps.length) {
        const empty = document.createElement("span");
        empty.className = "empty";
        empty.textContent = "Arrastra apps aquí";
        tiles.appendChild(empty);
      }
      ring.apps.forEach(id => {
        const app = cfg.apps[id];
        if (!app) return;
        const t = document.createElement("button");
        t.className = "tile" + (id === selected ? " selected" : "");
        t.dataset.id = id;
        t.draggable = true;
        t.title = app.ruta;
        const label = document.createElement("span");
        label.className = "name";
        label.textContent = app.nombre;
        t.append(iconEl(app), label);
        t.addEventListener("click", () => { selected = id; renderRings(); renderInspector(); });
        t.addEventListener("keydown", e => {
          if (e.key === "Delete") {
            removeFromRings(id);
            if (selected === id) selected = null;
            renderAll();
          }
        });
        t.addEventListener("dragstart", e => {
          e.dataTransfer.setData("text/orbe-move", JSON.stringify({ id, from: ri }));
          e.dataTransfer.effectAllowed = "move";
          t.classList.add("dragging");
        });
        t.addEventListener("dragend", () => t.classList.remove("dragging"));
        tiles.appendChild(t);
      });
      dropTarget(tiles, ri);

      card.append(head, tiles);
      box.appendChild(card);
    });
    $("add-ring").disabled = cfg.anillo.length >= MAX_RINGS;
  }

  // ---------- inspector ----------
  function renderInspector() {
    const app = selected && cfg.apps[selected];
    const inRing = app && cfg.anillo.some(r => r.apps.includes(selected));
    $("app-form").hidden = !inRing;
    $("no-app").hidden = !!inRing;
    if (!inRing) return;
    $("f-title").textContent = app.nombre;
    fillIcon($("f-icon"), app);
    $("f-nombre").value = app.nombre;
    $("f-ruta").value = app.ruta;
    $("f-args").value = app.args;
    $("f-icono").value = window.ORBE_GLYPHS[app.icono] ? app.icono : "globe";
    renderLibPicker(app);
  }

  // Opciones: "Auto" (icono del programa), cada imagen de la biblioteca y, si la hay, la importada.
  function renderLibPicker(app) {
    const grid = $("f-lib");
    grid.replaceChildren();
    const current = app.imagen || "";
    const opt = (value, label, fill) => {
      const b = document.createElement("button");
      b.type = "button";
      b.className = "lib-opt";
      b.setAttribute("role", "radio");
      b.setAttribute("aria-checked", String(value === current));
      b.setAttribute("aria-label", label);
      b.title = label;
      fill(b);
      b.addEventListener("click", () => setImage(value));
      grid.appendChild(b);
    };
    opt("", "Automática: icono del programa", b => { b.textContent = "Auto"; });
    window.ORBE_LIBRARY.forEach(l => opt("lib:" + l.id, l.nombre, b => {
      const img = document.createElement("img");
      img.src = `img/apps/${l.id}.png`;
      img.alt = "";
      b.appendChild(img);
    }));
    if (current && !current.startsWith("lib:")) {
      opt(current, "Imagen importada", b => {
        const span = document.createElement("span");
        span.className = "icon has-img";
        fillIcon(span, { ...app, ruta: "" });
        b.appendChild(span);
      });
    }
  }

  function refreshSelectedIcon() {
    const app = cfg.apps[selected];
    if (!app) return;
    fillIcon($("f-icon"), app);
    const tile = document.querySelector(`.tile[data-id="${CSS.escape(selected)}"] .icon`);
    if (tile) fillIcon(tile, app);
    renderLibPicker(app);
    markDirty();
  }

  function setImage(value) {
    const app = cfg.apps[selected];
    if (!app) return;
    app.imagen = value;
    refreshSelectedIcon();
  }

  function bindInspector() {
    const sel = $("f-icono");
    Object.keys(window.ORBE_GLYPHS).sort().forEach(k => {
      const o = document.createElement("option");
      o.value = o.textContent = k;
      sel.appendChild(o);
    });
    const field = (id, key) => $(id).addEventListener("input", () => {
      const app = cfg.apps[selected];
      if (!app) return;
      app[key] = $(id).value;
      $("f-title").textContent = app.nombre;
      if (key !== "args" && key !== "nombre") fillIcon($("f-icon"), app);
      const tile = document.querySelector(`.tile[data-id="${CSS.escape(selected)}"]`);
      if (tile) {
        tile.querySelector(".name").textContent = app.nombre;
        if (key !== "args" && key !== "nombre") fillIcon(tile.querySelector(".icon"), app);
      }
      markDirty();
    });
    field("f-nombre", "nombre");
    field("f-ruta", "ruta");
    field("f-args", "args");
    sel.addEventListener("input", () => {
      const app = cfg.apps[selected];
      if (!app) return;
      app.icono = sel.value;
      renderRings();
      fillIcon($("f-icon"), app);
      markDirty();
    });
    $("f-file").addEventListener("change", () => {
      const file = $("f-file").files[0];
      $("f-file").value = "";
      const app = cfg.apps[selected];
      if (!file || !app) return;
      if (file.size > 2 * 1024 * 1024) { status("La imagen pesa más de 2 MB", true); return; }
      const reader = new FileReader();
      reader.onload = async () => {
        try {
          app.imagen = await invoke("import_image", { nombre: app.nombre, data: reader.result });
          refreshSelectedIcon();
          status("Imagen importada");
        } catch (e) {
          status("No se pudo importar: " + e, true);
        }
      };
      reader.readAsDataURL(file);
    });
    $("f-remove").addEventListener("click", () => {
      removeFromRings(selected);
      selected = null;
      renderAll();
    });
  }

  // ---------- comportamiento ----------
  const BEHAVIOR = [
    ["c-retardo", () => cfg.zona, "retardo_ms", "number"],
    ["c-gracia", () => cfg.zona, "gracia_ms", "number"],
    ["c-tamano", () => cfg.zona, "tamano", "number"],
    ["c-paso", () => cfg.rueda, "scroll_paso", "number"],
    ["c-orientacion", () => cfg.zona, "orientacion", "text"],
    ["c-invertir", () => cfg.rueda, "invertir", "bool"]
  ];

  // Tinte: "#RRGGBBAA" <-> selector de color + intensidad (alfa en %).
  function splitTint(hex) {
    const m = /^#([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(hex || "");
    if (!m) return { color: "#5b7cfa", pct: 12 };
    return { color: "#" + m[1].toLowerCase(), pct: Math.round(parseInt(m[2] || "ff", 16) / 2.55) };
  }
  function joinTint() {
    const a = Math.round(Number($("c-tinte-a").value) * 2.55).toString(16).padStart(2, "0");
    return ($("c-tinte").value + a).toUpperCase();
  }

  function renderBehavior() {
    BEHAVIOR.forEach(([id, obj, key, kind]) => {
      const el = $(id);
      if (kind === "bool") el.checked = !!obj()[key]; else el.value = obj()[key];
    });
    const t = splitTint(cfg.aspecto.tinte);
    $("c-tinte").value = t.color;
    $("c-tinte-a").value = t.pct;
    $("c-tinte-out").textContent = t.pct + " %";
  }

  function bindBehavior() {
    BEHAVIOR.forEach(([id, obj, key, kind]) => {
      const el = $(id);
      el.addEventListener(kind === "bool" || el.tagName === "SELECT" ? "change" : "input", () => {
        if (kind === "bool") obj()[key] = el.checked;
        else if (kind === "number") {
          const v = Number(el.value);
          if (el.value === "" || !Number.isFinite(v)) return;
          obj()[key] = key === "scroll_paso" ? v : Math.round(v);
        } else obj()[key] = el.value;
        markDirty();
      });
    });
    ["c-tinte", "c-tinte-a"].forEach(id => $(id).addEventListener("input", () => {
      cfg.aspecto.tinte = joinTint();
      $("c-tinte-out").textContent = $("c-tinte-a").value + " %";
      markDirty();
    }));
  }

  function renderAll() {
    renderRings();
    renderInspector();
    renderBehavior();
    markDirty();
  }

  // ---------- carga y guardado ----------
  async function load() {
    try {
      cfg = await invoke("get_config");
      saved = JSON.stringify(cfg);
      if (selected && !cfg.apps[selected]) selected = null;
      renderAll();
    } catch (e) {
      status("No se pudo cargar la configuración: " + e, true);
    }
  }

  async function save() {
    // Quitar apps que ya no están en ningún anillo.
    const used = new Set(cfg.anillo.flatMap(r => r.apps));
    Object.keys(cfg.apps).forEach(id => { if (!used.has(id)) delete cfg.apps[id]; });
    cfg.anillo.forEach((r, i) => { if (!r.nombre.trim()) r.nombre = `Anillo ${i + 1}`; });
    try {
      await invoke("save_config", { config: cfg });
      cfg = await invoke("get_config"); // lo que Rust validó
      saved = JSON.stringify(cfg);
      renderAll();
      status("Guardado");
    } catch (e) {
      status("No se pudo guardar: " + e, true);
    }
  }

  $("save").addEventListener("click", save);
  $("reset").addEventListener("click", () => { selected = null; load(); status("Cambios descartados"); });
  $("place").addEventListener("click", () => invoke("start_placing").catch(e => status(String(e), true)));
  $("add-ring").addEventListener("click", () => {
    if (cfg.anillo.length >= MAX_RINGS) return;
    cfg.anillo.push({ nombre: `Anillo ${cfg.anillo.length + 1}`, apps: [] });
    renderAll();
  });
  $("search").addEventListener("input", renderInstalled);
  $("m-add").addEventListener("click", () => {
    const nombre = $("m-nombre").value.trim();
    const ruta = $("m-ruta").value.trim();
    if (!nombre || !ruta) { status("Pon nombre y ruta", true); return; }
    addApp({ nombre, ruta, args: $("m-args").value.trim(), icono: "app" }, cfg.anillo.length - 1);
    $("m-nombre").value = $("m-ruta").value = $("m-args").value = "";
  });
  document.addEventListener("keydown", e => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") { e.preventDefault(); save(); }
  });

  if (TAURI) {
    TAURI.event.listen("orbe:config-changed", () => { if (!isDirty()) load(); });
  }

  bindInspector();
  bindBehavior();
  load();
  invoke("list_installed_apps")
    .then(list => { installed = list || []; renderInstalled(); })
    .catch(e => status("No se pudo leer el menú Inicio: " + e, true));
})();
