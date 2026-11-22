//! Detector de cursor: máquina de estados pura + hilo que la alimenta a ~60 Hz.

use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

use crate::{geometry, win, AppState};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum State {
    Escondida,
    Armada { desde: Instant },
    Abierta,
    Cerrando { desde: Instant },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Input {
    pub in_zone: bool,
    pub in_wheel: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    Open,
    Close,
}

pub fn step(state: State, input: Input, now: Instant, retardo: Duration, gracia: Duration) -> (State, Option<Action>) {
    use State::*;
    if !input.enabled {
        return match state {
            Abierta | Cerrando { .. } => (Escondida, Some(Action::Close)),
            _ => (Escondida, None),
        };
    }
    match state {
        Escondida if input.in_zone => (Armada { desde: now }, None),
        Escondida => (Escondida, None),
        Armada { .. } if !input.in_zone => (Escondida, None),
        Armada { desde } if now.saturating_duration_since(desde) >= retardo => (Abierta, Some(Action::Open)),
        Armada { desde } => (Armada { desde }, None),
        Abierta if input.in_zone || input.in_wheel => (Abierta, None),
        Abierta => (Cerrando { desde: now }, None),
        Cerrando { .. } if input.in_zone || input.in_wheel => (Abierta, None),
        Cerrando { desde } if now.saturating_duration_since(desde) >= gracia => (Escondida, Some(Action::Close)),
        Cerrando { desde } => (Cerrando { desde }, None),
    }
}

const TICK: Duration = Duration::from_millis(16);
const FULLSCREEN_EVERY: Duration = Duration::from_millis(500);
const GEO_EVERY: Duration = Duration::from_secs(2);

pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("orbe-cursor".into())
        .spawn(move || run(app))
        .expect("no se pudo crear el hilo del cursor");
}

fn run(app: AppHandle) {
    crate::refresh_zone_geo(&app);
    let mut state = State::Escondida;
    let mut fullscreen = false;
    let mut last_fs = Instant::now() - FULLSCREEN_EVERY;
    let mut last_geo = Instant::now();
    // ORBE_DEBUG=1: registra cada cambio de estado en stderr.
    let debug = std::env::var_os("ORBE_DEBUG").is_some();
    let mut was_fullscreen = false;

    loop {
        std::thread::sleep(TICK);
        let now = Instant::now();

        if now.duration_since(last_geo) >= GEO_EVERY && state == State::Escondida {
            last_geo = now;
            crate::refresh_zone_geo(&app);
        }
        if now.duration_since(last_fs) >= FULLSCREEN_EVERY {
            last_fs = now;
            fullscreen = win::foreground_is_fullscreen();
            if debug && fullscreen != was_fullscreen {
                eprintln!("[orbe] pantalla completa: {fullscreen}");
            }
            was_fullscreen = fullscreen;
        }

        let (geo, enabled, retardo, gracia, force_close) = {
            let st = app.state::<AppState>();
            let Ok(mut rt) = st.0.lock() else { continue };
            let fc = std::mem::take(&mut rt.force_close);
            (
                rt.geo,
                rt.enabled && !rt.placing,
                Duration::from_millis(rt.config.zona.retardo_ms),
                Duration::from_millis(rt.config.zona.gracia_ms),
                fc,
            )
        };

        if force_close {
            if matches!(state, State::Abierta | State::Cerrando { .. }) {
                crate::hide_wheel(&app);
            }
            state = State::Escondida;
            continue;
        }

        let Some(g) = geo else { continue };
        let Some((px, py)) = win::cursor_pos() else { continue };
        let (px, py) = (px as f64, py as f64);
        let input = Input {
            in_zone: geometry::in_zone(&g, px, py),
            in_wheel: state != State::Escondida && geometry::in_wheel(&g, px, py),
            enabled: enabled && !fullscreen,
        };

        let (next, action) = step(state, input, now, retardo, gracia);
        if debug && std::mem::discriminant(&next) != std::mem::discriminant(&state) {
            eprintln!("[orbe] ({px},{py}) {input:?} {state:?} -> {next:?} {action:?} zona=({:.0},{:.0})-({:.0},{:.0})",
                g.zone_l, g.zone_t, g.zone_r, g.zone_b);
        }
        state = next;
        match action {
            Some(Action::Open) => crate::show_wheel(&app, g),
            Some(Action::Close) => crate::hide_wheel(&app),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use State::*;

    const R: Duration = Duration::from_millis(120);
    const G: Duration = Duration::from_millis(250);

    fn inp(in_zone: bool, in_wheel: bool) -> Input {
        Input { in_zone, in_wheel, enabled: true }
    }

    #[test]
    fn entrar_arma() {
        let t = Instant::now();
        assert_eq!(step(Escondida, inp(true, false), t, R, G), (Armada { desde: t }, None));
        assert_eq!(step(Escondida, inp(false, true), t, R, G), (Escondida, None));
    }

    #[test]
    fn salir_antes_del_retardo_desarma() {
        let t = Instant::now();
        let s = Armada { desde: t };
        assert_eq!(step(s, inp(false, false), t + Duration::from_millis(50), R, G), (Escondida, None));
        assert_eq!(step(s, inp(true, false), t + Duration::from_millis(50), R, G), (s, None));
    }

    #[test]
    fn retardo_abre() {
        let t = Instant::now();
        assert_eq!(step(Armada { desde: t }, inp(true, false), t + R, R, G), (Abierta, Some(Action::Open)));
    }

    #[test]
    fn abierta_se_mantiene_en_rueda() {
        let t = Instant::now();
        assert_eq!(step(Abierta, inp(false, true), t, R, G), (Abierta, None));
        assert_eq!(step(Abierta, inp(true, false), t, R, G), (Abierta, None));
    }

    #[test]
    fn salir_empieza_a_cerrar_sin_accion() {
        let t = Instant::now();
        assert_eq!(step(Abierta, inp(false, false), t, R, G), (Cerrando { desde: t }, None));
    }

    #[test]
    fn volver_cancela_cierre() {
        let t = Instant::now();
        let s = Cerrando { desde: t };
        assert_eq!(step(s, inp(false, true), t + Duration::from_millis(100), R, G), (Abierta, None));
    }

    #[test]
    fn gracia_cierra() {
        let t = Instant::now();
        let s = Cerrando { desde: t };
        assert_eq!(step(s, inp(false, false), t + Duration::from_millis(100), R, G), (s, None));
        assert_eq!(step(s, inp(false, false), t + G, R, G), (Escondida, Some(Action::Close)));
    }

    #[test]
    fn desactivada_cierra() {
        let t = Instant::now();
        let off = Input { in_zone: true, in_wheel: true, enabled: false };
        assert_eq!(step(Abierta, off, t, R, G), (Escondida, Some(Action::Close)));
        assert_eq!(step(Cerrando { desde: t }, off, t, R, G), (Escondida, Some(Action::Close)));
        assert_eq!(step(Armada { desde: t }, off, t, R, G), (Escondida, None));
        assert_eq!(step(Escondida, off, t, R, G), (Escondida, None));
    }
}
