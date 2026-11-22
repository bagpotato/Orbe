// Captura promo/index.html con Edge headless (protocolo DevTools) y codifica con ffmpeg.
//
//   node promo/render.mjs --url http://localhost:5181/promo/index.html --frames 7.6,12.4   -> JPEG sueltos para revisar
//   node promo/render.mjs --url http://localhost:5181/promo/index.html --video            -> promo/out/orbe-promo.mp4
//
// Cada escena se codifica en su propio .mp4 (mismos ajustes) y al final se unen con el
// demuxer concat sin recodificar.
import { spawn } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, renameSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const OUT = resolve(here, "out");
const FPS = 30;
const SCENES = [
  ["1-intro", 0, 4.5], ["2-aparece", 4.5, 10.5], ["3-gira", 10.5, 16], ["4-abre", 16, 20],
  ["5-colocar", 20, 26.8], ["6-funciones", 26.8, 31.8], ["7-cierre", 31.8, 36]
];

const args = process.argv.slice(2);
const opt = k => { const i = args.indexOf(k); return i >= 0 ? args[i + 1] : undefined; };
const url = (opt("--url") || "http://localhost:5181/promo/index.html") + "?capture";
const EDGE = process.env.EDGE || "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe";
const FFMPEG = process.env.FFMPEG || "ffmpeg";
mkdirSync(OUT, { recursive: true });

// ---------- navegador ----------
const port = 9300 + Math.floor(Math.random() * 500);
const profile = mkdtempSync(join(tmpdir(), "orbe-promo-"));
const browser = spawn(EDGE, [
  "--headless=new", `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`,
  "--window-size=1920,1080", "--hide-scrollbars", "--force-device-scale-factor=1",
  "--no-first-run", "--disable-extensions", "about:blank"
], { stdio: "ignore" });

const sleep = ms => new Promise(r => setTimeout(r, ms));
let targets = null;
for (let i = 0; i < 100 && !targets; i++) {
  try { targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json(); } catch { await sleep(100); }
}
const page = targets.find(t => t.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r, j) => { ws.onopen = r; ws.onerror = j; });
let nextId = 1;
const pending = new Map();
ws.onmessage = ev => {
  const m = JSON.parse(ev.data);
  if (m.id && pending.has(m.id)) {
    const { resolve: ok, reject } = pending.get(m.id);
    pending.delete(m.id);
    m.error ? reject(new Error(JSON.stringify(m.error))) : ok(m.result);
  }
};
const cdp = (method, params = {}) => new Promise((ok, reject) => {
  const id = nextId++;
  pending.set(id, { resolve: ok, reject });
  ws.send(JSON.stringify({ id, method, params }));
});
const evaluate = async expr => {
  const r = await cdp("Runtime.evaluate", { expression: expr, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
  return r.result.value;
};

await cdp("Emulation.setDeviceMetricsOverride", { width: 1920, height: 1080, deviceScaleFactor: 1, mobile: false });
await cdp("Page.enable");
await cdp("Page.navigate", { url });
for (let i = 0; i < 100; i++) {
  if (await evaluate("!!(window.PROMO)").catch(() => false)) break;
  await sleep(100);
}
await evaluate("PROMO.ready()");

async function shot(t) {
  await evaluate(`PROMO.renderAt(${t})`);
  const { data } = await cdp("Page.captureScreenshot", { format: "jpeg", quality: 92, optimizeForSpeed: true });
  return Buffer.from(data, "base64");
}

function encodeScene(name, t0, t1) {
  return new Promise(async (ok, fail) => {
    const tmp = join(OUT, `${name}.part.mp4`);
    const ff = spawn(FFMPEG, [
      "-y", "-loglevel", "error", "-f", "image2pipe", "-framerate", String(FPS), "-c:v", "mjpeg", "-i", "-",
      // Los JPEG son de rango completo: se pasa a rango TV estándar para que se vea igual en todas partes.
      "-vf", "scale=in_range=pc:out_range=tv,format=yuv420p", "-color_range", "tv",
      "-colorspace", "bt709", "-color_primaries", "bt709", "-color_trc", "bt709",
      "-c:v", "libx264", "-crf", "18", "-preset", "medium", "-pix_fmt", "yuv420p", "-r", String(FPS),
      "-movflags", "+faststart", tmp
    ], { stdio: ["pipe", "inherit", "inherit"] });
    ff.on("error", fail);
    const exit = new Promise(r => ff.on("close", r));
    const first = Math.round(t0 * FPS), last = Math.round(t1 * FPS);
    for (let f = first; f < last; f++) {
      const buf = await shot(f / FPS);
      if (!ff.stdin.write(buf)) await new Promise(r => ff.stdin.once("drain", r));
    }
    ff.stdin.end();
    const code = await exit;
    if (code !== 0) return fail(new Error(`ffmpeg salió con ${code} en ${name}`));
    renameSync(tmp, join(OUT, `${name}.mp4`));
    ok();
  });
}

try {
  if (opt("--frames")) {
    for (const t of opt("--frames").split(",").map(Number)) {
      writeFileSync(join(OUT, `frame-${t.toFixed(2)}.jpg`), await shot(t));
      console.log(`frame ${t}`);
    }
  } else if (args.includes("--video")) {
    const only = opt("--scene");
    const t0 = Date.now();
    for (const [name, a, b] of SCENES) {
      if (only && only !== name) continue;
      const s = Date.now();
      await encodeScene(name, a, b);
      console.log(`${name}: ${((Date.now() - s) / 1000).toFixed(0)} s`);
    }
    const list = SCENES.map(([n]) => `file '${join(OUT, n + ".mp4").replace(/\\/g, "/")}'`).join("\n");
    writeFileSync(join(OUT, "escenas.txt"), list + "\n");
    const final = join(OUT, "orbe-promo.mp4");
    const tmp = join(OUT, "orbe-promo.part.mp4");
    const code = await new Promise(r => spawn(FFMPEG, ["-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", join(OUT, "escenas.txt"), "-c", "copy", "-movflags", "+faststart", tmp], { stdio: "inherit" }).on("close", r));
    if (code !== 0) throw new Error("falló la unión de escenas");
    if (existsSync(final)) rmSync(final);
    renameSync(tmp, final);
    console.log(`listo: ${final} (${((Date.now() - t0) / 1000).toFixed(0)} s)`);
  }
} finally {
  ws.close();
  browser.kill();
  await sleep(300);
  try { rmSync(profile, { recursive: true, force: true }); } catch {}
}
