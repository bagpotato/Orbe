# Orbe

Media rueda de apps con aspecto de cristal, escondida en un punto de la pantalla. Pasas el ratón por la zona y se abre mirando hacia dentro; cada anillo gira por separado con la rueda del ratón. Versión 1: **Windows 10/11** (Tauri 2 + Rust + HTML/CSS/JS sin frameworks).

El planning original está en [docs/PLAN.html](docs/PLAN.html). Este README describe lo que está construido (fases 1 a 5).

## Uso

```powershell
npm install
npx tauri dev          # desarrollo
npx tauri build        # instalador .msi en src-tauri/target/release/bundle/msi
```

O directamente: `cargo build` en `src-tauri` y ejecutar `src-tauri/target/debug/orbe.exe`.

- **Zona por defecto**: centro del borde izquierdo del monitor principal.
- **Bandeja**: clic izquierdo abre los ajustes; menú con *Ajustes…*, *Colocar zona*, *Pausar/Reanudar* y *Salir*.
- **Línea de comandos**: `orbe.exe --ajustes`, `orbe.exe --colocar`. Si Orbe ya está en marcha, la segunda ejecución solo reenvía la orden (instancia única); ejecutarlo sin argumentos abre los ajustes.
- **Rueda**: el anillo bajo el cursor es el que gira; clic en una app la abre y cierra la rueda. Con el foco en una app, las flechas giran el anillo.
- **Pantalla completa**: si una app de otro proceso ocupa el monitor sin barra de título (juego, vídeo), la zona se desactiva sola. Las ventanas maximizadas no cuentan.

Diagnóstico: con la variable de entorno `ORBE_DEBUG=1`, el detector escribe en stderr cada cambio de estado (posición del cursor, zona, pantalla completa).

Probar la interfaz en un navegador normal: servir `src/` (por ejemplo `npx http-server src`) y abrir `wheel.html`, `settings.html` o `placer.html`. Sin Tauri, cada página entra en modo demo con datos de ejemplo.

## Configuración

`%APPDATA%\orbe\config.toml`. Se crea con 3 anillos de ejemplo (Sistema, Trabajo, Favoritos) la primera vez. Si está corrupto se aparta a `config.toml.bak`. Todo se puede editar desde los ajustes; el archivo admite:

```toml
[zona]
monitor = "\\\\.\\DISPLAY1"   # "" = principal
x = 0.0                        # 0 a 1, relativo al ancho del monitor
y = 0.5
tamano = 24                    # px lógicos
orientacion = "auto"           # auto, derecha, izquierda, abajo, arriba
retardo_ms = 120               # tiempo dentro de la zona antes de abrir
gracia_ms = 250                # margen al salir antes de cerrar

[rueda]
scroll_paso = 50.0             # recorrido de scroll por hueco
invertir = false

[aspecto]
tinte = "#5B7CFA1F"            # color que tiñe el cristal, #RRGGBBAA

[[anillo]]                     # de dentro hacia fuera, de 1 a 4
nombre = "Sistema"
apps = ["ajustes", "archivos"]

[apps.ajustes]
nombre = "Ajustes"
ruta = "ms-settings:"          # exe, .lnk, URI (ms-settings:, shell:…, https:), %VARIABLES%
args = ""
icono = "gear"                 # glifo si no hay ninguna imagen
imagen = ""                    # "" = icono del programa, "lib:discord" = biblioteca, o ruta a una imagen
```

`ruta = "orbe:bloquear"` bloquea el equipo.

**Imagen de cada app**, por orden de prioridad:
1. `imagen`. Puede ser:
   - una de la biblioteca incluida, `lib:<id>` (`discord`, `spotify`, `steam`, `whatsapp`, `github`, `claude`, `ai`);
   - una imagen propia importada desde los ajustes (PNG, JPG, GIF, WebP, ICO o SVG, hasta 2 MB), que se copia a `%APPDATA%\orbe\imagenes`.
2. El icono real del programa (exe o lnk), con caché en `%APPDATA%\orbe\icons`.
3. El glifo `icono`.

Al añadir una app cuyo nombre coincide con la biblioteca (por ejemplo Discord), se le asigna su imagen automáticamente.

## Créditos

Los iconos de `src/img/apps` son de la colección *Liquid Glass* de [Icons8](https://icons8.com). Su licencia gratuita exige enlazar a Icons8. Si distribuyes Orbe, mantén esta atribución (también aparece en los ajustes) o usa una licencia de pago.

## Arquitectura

| Archivo | Qué hace |
|---|---|
| `src-tauri/src/lib.rs` | Estado (`Runtime`), comandos Tauri, bandeja, ventanas, instancia única |
| `src-tauri/src/cursor.rs` | Máquina de estados *escondida → armada → abierta → cerrando* (pura, con tests) y el hilo que lee el cursor a ~60 Hz |
| `src-tauri/src/geometry.rs` | Zona, orientación, centro, escala y pruebas de "cursor dentro" en píxeles físicos (multi-monitor y HiDPI) |
| `src-tauri/src/win.rs` | Win32: ventana que no se activa, región en media rueda, pantalla completa, bloquear |
| `src-tauri/src/launcher.rs` | `ShellExecuteW` con permiso de primer plano |
| `src-tauri/src/icons.rs` | Imagen de cada app: biblioteca, imagen importada o icono del sistema (48 px, extracción serializada) con caché |
| `src/library.js`, `src/img/apps/` | Biblioteca de iconos incluidos y emparejado automático por nombre |
| `src-tauri/src/apps.rs` | Accesos del menú Inicio para el editor |
| `src-tauri/src/config.rs` | Config TOML, valores por defecto y validación |
| `src/geometry.js` | Las mismas fórmulas que `geometry.rs`, para dibujar |
| `src/wheel.*` | La media rueda (animación con `requestAnimationFrame` solo mientras algo se mueve) |
| `src/settings.*` | Editor de anillos (arrastrar entre anillos y desde las apps instaladas) y ajustes |
| `src/placer.*` | Overlay a pantalla completa para arrastrar la zona, con vista previa de la orientación |

Decisiones que no son obvias:

- **La ventana de la rueda nunca se oculta.** Se queda visible con una región vacía (`SetWindowRgn`), que no pinta ni recibe clics. Al abrir se recorta a la media rueda, así los clics fuera del semicírculo pasan a la ventana de debajo y WebView2 no se congela (abre al instante).
- **Sin Acrylic.** El desenfoque nativo de Windows no respeta la región y, en una ventana que nunca se activa, pinta su color de reserva gris. El cristal es CSS (degradados + `tinte`).
- **No roba el foco**: `WS_EX_NOACTIVATE` + `SWP_NOACTIVATE`. La rueda del ratón llega igual porque Windows manda el scroll a la ventana bajo el cursor.
- Los comandos que crean ventanas son `async` (en Windows, crearlas desde un comando síncrono bloquea la app). La ventana de ajustes desactiva el manejador de arrastre de Tauri para que funcione el drag & drop HTML5.

Tests: `cargo test --lib` en `src-tauri` (26 tests: máquina de estados, geometría, config, iconos e imágenes, launcher, apps).

## Pendiente (fases 6 y 7 del planning)

- Linux (X11 / Wayland layer-shell).
- Atajo global para abrir sin ratón, autoarranque con el sistema, comprobar el instalador `.msi` en un equipo limpio.
- El campo `rueda.atajo` y `aspecto.tema` se guardan pero todavía no se usan.
