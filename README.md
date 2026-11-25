# Orbe

![Orbe Demo](https://github.com/bagpotato/orbe/blob/main/docs/Orbe.gif?raw=true)

## Usage

```powershell
npm install
npx tauri dev          # development
npx tauri build        # .msi (Windows) or .deb / .rpm / .AppImage (Linux)
```

Or directly: `cargo build` in `src-tauri` and run `src-tauri/target/debug/orbe.exe` (Windows) or `src-tauri/target/debug/orbe` (Linux).

### Linux (X11 & Wayland)

Build dependencies on Debian/Ubuntu (24.04+):

```bash
sudo apt install build-essential libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf libx11-dev libgtk-layer-shell0
```

- **X11**: uses `XQueryPointer`, `_NET_WM_STATE_FULLSCREEN` detection, and `input_shape_combine_region` pass-through (identical behavior to Windows).
  - A compositor (picom, KWin, Mutter, xfwm4…) is needed for real transparency. Without one the wheel is clipped to its half-disc shape and hidden while closed, but the area behind it is drawn black.
- **Wayland (Hyprland, Sway, Niri, KDE)**: loads `libgtk-layer-shell.so.0` dynamically at runtime (no link-time dependency) to create an invisible overlay trigger zone and position the half-wheel in `LAYER_OVERLAY`. The `.deb`/`.rpm` depend on it and the AppImage bundles it; when building from source install `gtk-layer-shell` on your distro.
  - On Hyprland, you can enable native blur behind the wheel with `layerrule = blur, orbe`.
  - **GNOME Wayland is not supported**: Mutter lacks `wlr-layer-shell`, so Orbe cannot see the cursor and the wheel never opens. Orbe shows a notification at startup when this happens. Use a GNOME on Xorg session instead; Settings/Placer still open via CLI (`orbe --ajustes`, `orbe --colocar`).
- **`orbe:bloquear`**: GNOME/KDE/Xfce… use `loginctl lock-session`; Hyprland uses `hyprlock`, other Wayland compositors `swaylock -f`, bare X11 window managers `i3lock`/`xsecurelock`/`slock`. Install one of them on tiling setups: `loginctl` alone silently does nothing without an idle daemon listening.

- **Default zone**: center of the primary monitor's left edge.
- **System tray**: left-click opens settings; menu with *Settings…*, *Place zone*, *Pause/Resume*, and *Exit*.
- **Command line**: `orbe.exe --ajustes`, `orbe.exe --colocar`. If Orbe is already running, a second execution forwards the command (single instance); running without arguments opens settings.
- **Wheel**: the ring under the cursor is the one that rotates; clicking an app launches it and closes the wheel. With focus on an app, arrow keys rotate the ring.
- **Full screen**: if an app from another process occupies the monitor without a title bar (game, video), the trigger zone automatically deactivates. Maximized windows do not count.

Diagnostics: with the environment variable `ORBE_DEBUG=1`, the detector logs every state change to stderr (cursor position, trigger zone, full screen).

Testing the interface in a standard browser: serve `src/` (for example `npx http-server src`) and open `wheel.html`, `settings.html`, or `placer.html`. Without Tauri, each page enters demo mode with sample data.

## Configuration

- **Windows**: `%APPDATA%\orbe\config.toml`
- **Linux**: `$XDG_CONFIG_HOME/orbe/config.toml` (defaults to `~/.config/orbe/config.toml`)

Created with 3 sample rings (System, Work, Favorites) on first launch. If corrupted, it is backed up to `config.toml.bak`. Everything can be edited from settings; the file supports:

```toml
[zona]
monitor = "\\\\.\\DISPLAY1"   # "" = primary
x = 0.0                        # 0 to 1, relative to monitor width
y = 0.5
tamano = 24                    # logical px
orientacion = "auto"           # auto, derecha, izquierda, abajo, arriba
retardo_ms = 120               # hover delay before opening (ms)
gracia_ms = 250                # grace period after exiting before closing (ms)

[rueda]
scroll_paso = 50.0             # scroll travel distance per slot
invertir = false

[aspecto]
tinte = "#5B7CFA1F"            # glass tint color, #RRGGBBAA

[[anillo]]                     # inside out, from 1 to 4
nombre = "Sistema"
apps = ["ajustes", "archivos"]

[apps.ajustes]
nombre = "Ajustes"
ruta = "ms-settings:"          # exe, .lnk, URI (ms-settings:, shell:…, https:), %VARIABLES%
args = ""
icono = "gear"                 # glyph if no image is available
imagen = ""                    # "" = program icon, "lib:discord" = bundled library, or path to an image
```

`ruta = "orbe:bloquear"` locks the workstation.

**App image**, in order of priority:
1. `imagen`. Can be:
   - one from the bundled library, `lib:<id>` (`discord`, `spotify`, `steam`, `whatsapp`, `github`, `claude`, `ai`);
   - a custom image imported from settings (PNG, JPG, GIF, WebP, ICO, or SVG, up to 2 MB), which is copied to `%APPDATA%\orbe\imagenes`.
2. The executable's actual icon (exe or lnk), cached in `%APPDATA%\orbe\icons`.
3. The fallback `icono` glyph.

When adding an app whose name matches an entry in the library (e.g. Discord), its image is assigned automatically.

## Promo Video

`promo/` contains a 36 s motion graphic (1080p, 30 fps) built with HTML. `promo.js` renders each frame as a function of time, using Orbe's actual geometry, glyphs, and icons. Rendering requires Edge and ffmpeg:

```powershell
npx http-server . -p 5181 -c-1
node promo/render.mjs --video                    # promo/out/orbe-promo.mp4
node promo/render.mjs --frames 8.4,17.1          # individual frames for review
```

Opening `http://localhost:5181/promo/` plays it in a loop in the browser, and `?t=12.5` freezes that exact instant.

## Credits

Icons in `src/img/apps` are from the *Liquid Glass* collection by [Icons8](https://icons8.com). Their free license requires attribution linking to Icons8. If distributing Orbe, maintain this attribution (also shown in settings) or use a paid license.

## Architecture

| File | What it does |
|---|---|
| `src-tauri/src/lib.rs` | State (`Runtime`), Tauri commands, system tray, windows, single instance |
| `src-tauri/src/cursor.rs` | State machine *hidden → armed → open → closing* (pure, with tests) and cursor polling thread at ~60 Hz |
| `src-tauri/src/geometry.rs` | Trigger zone, orientation, center, scale, and "cursor inside" hit-tests in physical pixels (multi-monitor & HiDPI) |
| `src-tauri/src/win.rs` | Win32: non-activating window, half-wheel region clipping, full-screen detection, lock screen |
| `src-tauri/src/linux.rs` | Linux (X11 & Wayland layer-shell): trigger zone, region clipping, full-screen detection, lock screen |
| `src-tauri/src/launcher.rs` | `ShellExecuteW` (Windows) and `.desktop` / `xdg-open` / detached spawn (Linux) |
| `src-tauri/src/icons.rs` | App images: bundled library, imported image, Windows executable icon or Freedesktop icon theme with cache |
| `src/library.js`, `src/img/apps/` | Bundled icon library and automatic name matching |
| `src-tauri/src/apps.rs` | Start Menu (`.lnk`/`.url`) on Windows and `.desktop` applications indexer on Linux |
| `src-tauri/src/config.rs` | TOML config, default values, and validation |
| `src/geometry.js` | Rendering geometry sharing the same mathematical formulas as `geometry.rs` |
| `src/wheel.*` | Half-wheel UI (animates with `requestAnimationFrame` only while in motion) |
| `src/settings.*` | Ring editor (drag & drop between rings and from installed apps) and settings UI |
| `src/placer.*` | Full-screen overlay to position the trigger zone with orientation preview |

Non-obvious design decisions:

- **The wheel window is never hidden.** It stays visible with an empty region (`SetWindowRgn`), which neither paints nor receives clicks. On open, it clips to the half-wheel region, allowing clicks outside the semicircle to pass through to underlying windows and preventing WebView2 freeze/lag (opens instantaneously).
- **No Acrylic.** Native Windows blur does not honor window regions properly and paints a grey fallback color on non-activating windows. The glass effect is implemented in CSS (gradients + `tinte`).
- **Does not steal focus**: `WS_EX_NOACTIVATE` + `SWP_NOACTIVATE`. Mouse wheel scrolling still works because Windows dispatches scroll events to whichever window is beneath the cursor.
- Window creation commands are `async` (on Windows, creating windows synchronously inside an IPC command deadlocks the app). The settings window disables Tauri's default drag handler so HTML5 drag & drop works properly.

Tests: `cargo test --lib` in `src-tauri` (26 tests: state machine, geometry, config, icons & images, launcher, apps).

## Roadmap / Pending (phase 7)

- Global keyboard shortcut to open without a mouse, run on system startup, verify `.msi` / `.deb` / `.AppImage` installers on clean machines.
- The `rueda.atajo` and `aspecto.tema` fields are saved in config but not yet hooked up.
