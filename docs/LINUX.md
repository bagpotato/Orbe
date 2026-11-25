# Soporte de Linux en Orbe

Resumen de la implementación del soporte multiplataforma para Linux (**X11** y **Wayland con `layer-shell`**), empaquetado (`.deb`, `.rpm`, `.AppImage`) y flujo de CI añadidos a **Orbe**.

## Arquitectura en Linux

- **X11**:
  - Consulta global de cursor mediante `XQueryPointer` (`cursor_pos`).
  - Detección de ventanas a pantalla completa usando `_NET_ACTIVE_WINDOW`, `_NET_WM_PID` y `_NET_WM_STATE_FULLSCREEN` (`foreground_is_fullscreen`).
  - Ventana de rueda sin robo de foco (`set_accept_focus(false)`, `set_focus_on_map(false)`, `set_keep_above(true)`, `set_skip_taskbar_hint(true)`, `set_skip_pager_hint(true)`, `WindowTypeHint::Dock`) y recorte de clic pasante con `cairo::Region` (`input_shape_combine_region`) generado a partir de `geometry::half_disc_spans`.
  - **Con compositor**, la ventana queda mapeada siempre (transparente y sin entrada), como en Windows.
  - **Sin compositor** (i3/Openbox sin picom; se comprueba con `Screen::is_composited()` en cada uso), la rueda solo se mapea al abrirse y se oculta al cerrarse. Además se le aplica el recorte visual `shape_combine_region` con el mismo semicírculo, para que no aparezca un cuadrado negro de 532×532.
- **Wayland (Hyprland, Sway, Niri, KDE)**:
  - Carga dinámica en tiempo de ejecución de `libgtk-layer-shell.so.0` (o `libgtk-layer-shell.so`) mediante `libc::dlopen` / `libc::dlsym` para no exigir la librería al enlazar ni en sesiones X11. Dentro de un AppImage se prueba primero `$APPDIR/usr/lib/libgtk-layer-shell.so.0`.
  - Los símbolos de la API pública llevan el prefijo `gtk_layer_` (`gtk_layer_is_supported`, `gtk_layer_init_for_window`, `gtk_layer_set_*`), no `gtk_layer_shell_`.
  - Cuando `gtk_layer_is_supported()` devuelve verdadero, crea una ventana GTK transparente invisible como zona de activación en `LAYER_OVERLAY` (`sync_trigger_zone`) y posiciona la rueda también en `LAYER_OVERLAY` (`show_wheel` / `hide_wheel`), usando eventos `enter-notify-event` / `leave-notify-event` para el estado de hover (`native_hover_state`).
  - Si no hay layer-shell, `setup_wheel` avisa por stderr y con `notify-send`. El mensaje distingue entre librería ausente, compositor sin el protocolo (GNOME/Mutter, Weston) y Hyprland rechazando la superficie.
  - Incluye respaldo por socket IPC de Hyprland (`cursorpos` y `j/activewindow`) cuando `HYPRLAND_INSTANCE_SIGNATURE` está definido. `cursorpos` devuelve coordenadas lógicas, que se multiplican por la escala del monitor de la zona (`ZONE_SCALE`) para pasarlas al espacio físico de `ZoneGeo`. `fullscreen` se acepta como entero (Hyprland ≥ 0.42) o como booleano (versiones anteriores).
- **GNOME Wayland no está soportado**: Mutter no implementa `wlr-layer-shell` y no hay forma de leer el cursor global, así que la rueda no se abre. La alternativa es la sesión «GNOME sobre Xorg».
- **Bloqueo de sesión (`orbe:bloquear`)** (`lock_candidates`):
  - Escritorios completos (GNOME, KDE, Xfce, Cinnamon…): `loginctl lock-session`, `xdg-screensaver lock`, `gnome-screensaver-command -l`.
  - Hyprland: `hyprlock`, `swaylock -f`. Otros compositores Wayland: `swaylock -f`, `hyprlock`. X11 sin escritorio: `i3lock`, `xsecurelock`, `slock`, `xdg-screensaver lock`. En los tres casos, `loginctl lock-session` queda como último recurso.
  - `loginctl lock-session` no va primero fuera de los escritorios completos porque devuelve éxito aunque nadie atienda la señal (Hyprland/Sway sin hypridle/swayidle, i3).
  - Los bloqueadores que se quedan en primer plano (`hyprlock`, `slock`…) se lanzan con `spawn()`. Si siguen vivos a los 400 ms se da el bloqueo por hecho y un hilo recoge el proceso al desbloquear; así `launch_app` no se queda esperando.

## Archivos añadidos y modificados

| Archivo | Cambio realizado |
|---|---|
| `src-tauri/src/linux.rs` | Backend completo de Linux: X11, `gtk-layer-shell` dinámico, zona de activación, recorte `cairo::Region`, fullscreen, bloqueo e IPC opcional de Hyprland. |
| `src-tauri/src/win.rs` | Añadidos wrappers de plataforma (`setup_wheel`, `show_wheel`, `hide_wheel`, `uses_native_trigger`, `native_hover_state`, `sync_trigger_zone`) equivalentes a `linux.rs`. |
| `src-tauri/src/lib.rs` | Abstracción `#[cfg(windows)]` / `#[cfg(target_os = "linux")]` (`platform`) para inicializar, mostrar/ocultar la rueda y sincronizar la zona de activación. |
| `src-tauri/src/cursor.rs` | Usa `crate::platform` y soporta eventos nativos de entrada/salida (`uses_native_trigger` + `native_hover_state`) además del sondeo por cursor. |
| `src-tauri/src/geometry.rs` | Añadidos `mon_x`/`mon_y` en `ZoneGeo`, `half_disc_spans` para regiones de semicírculo y test `franjas_media_rueda`. |
| `src-tauri/src/config.rs` | Ruta XDG (`$XDG_CONFIG_HOME/orbe` o `~/.config/orbe`) y configuración por defecto para Linux (`default_groups`) manteniendo los 3 anillos / 29 apps. |
| `src-tauri/src/apps.rs` | Indexador de archivos `.desktop` en rutas XDG, Flatpak y Snap (`desktop_app_dirs`, `parse_desktop_entry`, `find_desktop_file`, `list_installed`) y test `parsea_desktop_entry`. |
| `src-tauri/src/launcher.rs` | Expansión de `$VAR`, `${VAR}` y `~/`, parser `parse_exec_command` para líneas `Exec=` (quitando `%u`, `%F`, etc.), lanzamiento de `.desktop` (con respaldo `gtk-launch`), URIs/directorios via `xdg-open`, alias `shell:` y procesos desacoplados. |
| `src-tauri/src/icons.rs` | Resolución de iconos Freedesktop (`linux_icon_name`, `gtk_icon_themes`, `find_icon_in_dirs`) leyendo temas GTK y `/usr/share/pixmaps`, más test `busca_icono_en_temas`. |
| `src-tauri/Cargo.toml` | Dependencias Linux (`gtk = "0.18"`, `x11-dl = "2.21"`, `libc = "0.2"`) y movimiento de `png = "0.17"` a `[dev-dependencies]` + `[target.'cfg(windows)'.dependencies]`. |
| `src-tauri/tauri.conf.json` | Añadidos targets `"deb"`, `"rpm"`, `"appimage"` junto a `"msi"`, e icono `icons/icon.png`. |
| `src-tauri/tauri.linux.conf.json` | Configuración que Tauri fusiona solo en Linux: dependencia `libgtk-layer-shell0` (deb) y `gtk-layer-shell` (rpm), y copia de `libgtk-layer-shell.so.0` dentro del AppImage. Como `dlopen` no deja rastro en el binario, sin esto los paquetes no la declararían. La ruta de origen es la de Ubuntu/Debian x86_64 (`/usr/lib/x86_64-linux-gnu/`), así que en otras distros o arquitecturas hay que ajustarla para generar el AppImage. |
| `src/settings.html` / `src/settings.js` | Placeholder actualizado con ejemplo `.desktop` y mensaje de error genérico para apps instaladas. |
| `.github/workflows/build.yml` | Workflow de GitHub Actions para compilar, pasar tests y generar instaladores en Windows (`windows-latest`) y Linux (`ubuntu-24.04`). |
| `README.md` | Documentación de dependencias en Linux, funcionamiento X11/Wayland, regla de blur en Hyprland y ruta de configuración. |

## Problemas detectados y corregidos en WSL (Ubuntu 24.04.5 LTS)

1. **Pánico de GDK fuera del hilo principal**: `cursor.rs` llamaba a `is_wayland()` / `layer_api()` desde el hilo `orbe-cursor`, provocando `GDK may only be used from the main thread`. Se corrigió cacheando `IS_WAYLAND` y `LAYER_SUPPORTED` en `OnceLock` durante `setup_wheel` en el hilo principal.
2. **Error fatal `BadWindow` en X11/XWayland**: bajo `GDK_BACKEND=wayland`, `with_x11` consultaba XWayland y Xlib abortaba con `X_GetProperty` (`BadWindow`). Se corrigió omitiendo `with_x11` cuando `is_wayland()` es `true` e instalando un `XSetErrorHandler` no fatal (`ignore_x11_error`).
3. **Warning de dependencia sin uso en Linux**: `png = "0.17"` solo se usa en Windows y en tests, por lo que se movió a `[dev-dependencies]` y `[target.'cfg(windows)'.dependencies]`.
4. **Nombres de símbolos de layer-shell incorrectos**: se buscaban `gtk_layer_shell_*`, pero la librería exporta `gtk_layer_*`. `dlsym` fallaba siempre, así que la ruta layer-shell no había llegado a activarse en ningún compositor. Se detectó al distinguir «librería ausente» de «no soportado» (WSL tiene `libgtk-layer-shell0` 0.8.2 instalada y aun así decía que faltaba) y se comprobó con `nm -D`.
5. **GNOME Wayland fallaba en silencio**: sin layer-shell ni Hyprland, `cursor_pos()` devuelve siempre `None` y la rueda no se abría, sin ningún aviso. Ahora avisa al arrancar.
6. **Paquetes sin `gtk-layer-shell`**: se resolvió con `tauri.linux.conf.json`.
7. **`orbe:bloquear`**: `loginctl` daba por bueno un bloqueo que no ocurría en compositores tiling, y `hyprlock`/`swaylock` con `.status()` dejaban el comando esperando hasta desbloquear. Se cambió por `lock_candidates` + `run_locker`.
8. **X11 sin compositor**: la rueda se veía como un cuadrado negro permanente. Ahora se oculta y se recorta visualmente.
9. **Hyprland**: se escala `cursorpos` (de lógico a físico) y `fullscreen` se acepta también como booleano.

## Verificación realizada

- **Linux (WSL2 Ubuntu 24.04.5 LTS, `CARGO_TARGET_DIR=/home/bag/orbe-target`)**:
  - `cargo test --lib`: **30 passed, 0 failed**, incluidos `orden_de_bloqueadores`, `cursor_hyprland_escalado` y `fullscreen_hyprland_int_y_bool`.
  - `cargo build`: **0 errores, 0 warnings**.
  - Smoke tests en WSLg (`/home/bag/orbe-target/debug/orbe` con `timeout`):
    - `GDK_BACKEND=x11`: arrancó sin panics (salida `124` por `timeout`).
    - `GDK_BACKEND=wayland`: arrancó sin panics (salida `124`). Con los símbolos corregidos, la librería carga y Weston responde «no soportado», lo que genera el aviso de compositor sin `wlr-layer-shell`.
    - `--ajustes` (`GDK_BACKEND=x11`): arrancó sin panics (salida `124`) y creó `~/.config/orbe/config.toml` con la configuración por defecto de Linux.
- **Windows**:
  - `cargo test --lib`: **30 passed, 0 failed**, con el código actual.

### Pendiente de validar fuera de WSLg
- Comportamiento real de `gtk-layer-shell` en un compositor Wayland con soporte de `wlr-layer-shell` (Hyprland, Sway, Niri o KDE), ya que WSLg usa Weston sin este protocolo. Es importante porque, hasta corregir los nombres de símbolos, esta ruta no se había ejecutado nunca.
- Generación completa de los paquetes `.deb`, `.rpm` y `.AppImage` (`npx tauri build` en Linux o mediante `.github/workflows/build.yml`), incluido que el AppImage lleve `usr/lib/libgtk-layer-shell.so.0` y que el `.deb` declare `Depends: libgtk-layer-shell0`.
- X11 sin compositor (por ejemplo i3 sin picom) y los bloqueadores de cada entorno.

## Comandos rápidos en Linux / WSL

```bash
# Dependencias en Ubuntu/Debian (24.04+)
sudo apt install build-essential pkg-config libssl-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf libx11-dev \
  libgtk-layer-shell0 libgtk-layer-shell-dev

# Tests y compilación desde WSL (usando target en ext4)
CARGO_TARGET_DIR=/home/bag/orbe-target cargo test --manifest-path /mnt/c/Users/bagpo/Desktop/todo/orbe/src-tauri/Cargo.toml --lib
CARGO_TARGET_DIR=/home/bag/orbe-target cargo build --manifest-path /mnt/c/Users/bagpo/Desktop/todo/orbe/src-tauri/Cargo.toml

# Ejecutar ventana de ajustes en WSLg (X11)
GDK_BACKEND=x11 /home/bag/orbe-target/debug/orbe --ajustes
```
