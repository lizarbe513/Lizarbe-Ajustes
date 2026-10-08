//! Capítulo 0: el arranque. El isotipo y el logotipo emergen del «ruido» de
//! tramas ░▒▓█, aparecen los datos reales del equipo y se saluda al usuario.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::anim;
use crate::app::App;
use crate::brand::*;
use crate::i18n::{t, tf};
use crate::ui::{dato, limpiar, texto};
use lizarbe_core::ui::put;

pub fn dibujar(app: &mut App, f: &mut Frame, area: Rect) {
    let tt = app.cap_t();
    let logo = isotipo();
    let lw = logo.iter().map(|l| l.width()).max().unwrap_or(0) as u16;
    let lh = logo.len() as u16;
    let ancho_texto = 52u16.min(area.width.saturating_sub(lw + 8));
    let total_w = lw + 4 + ancho_texto;
    let x0 = area.x + area.width.saturating_sub(total_w) / 2;
    let y0 = area.y + area.height.saturating_sub(lh.max(16) + 2) / 2;
    limpiar(f, area);

    // Isotipo: cada celda sale de la trama en su propio momento.
    let g = anim::smooth(tt / 2.2);
    for (y, linea) in logo.iter().enumerate() {
        let mut x = 0u32;
        for span in &linea.spans {
            for c in span.content.chars() {
                let av = anim::avance_celda(g, x, y as u32);
                if c != ' ' || span.style.bg.is_some() {
                    pinta_celda(f, x0 + x as u16, y0 + y as u16, c, span.style, av);
                }
                x += 1;
            }
        }
    }
    // Destellos alrededor del isotipo.
    if tt > 2.2 {
        for (k, (dx, dy)) in [(lw as i32 - 3, -1i32), (lw as i32 + 2, 3), (-3, 7)]
            .iter()
            .enumerate()
        {
            let p = anim::pulso(tt + k as f32 * 0.7, 1.6);
            let color = blend(SLATE, if k == 0 { RED } else { PHOSPHOR }, p);
            let (x, y) = (x0 as i32 + dx, y0 as i32 + dy);
            if x >= area.x as i32 && y >= area.y as i32 {
                texto(f, x as u16, y as u16, SPARK, bold(color));
            }
        }
    }

    // Texto a la derecha.
    let xt = x0 + lw + 4;
    let mut y = y0 + 1;
    let ancho_punto = if area.width >= 120 { 2 } else { 1 };
    let logotipo = logotipo(1);
    let _ = ancho_punto;
    let gl = anim::smooth((tt - 1.0) / 1.6);
    for (fila, linea) in logotipo.iter().enumerate() {
        let mut spans = vec![];
        for (x, c) in linea.chars().enumerate() {
            let av = anim::avance_celda(gl, x as u32 + 40, fila as u32);
            let ch = anim::trama(av, c);
            let color = blend(STEEL, PHOSPHOR, av);
            spans.push(Span::styled(ch.to_string(), Style::new().fg(color)));
        }
        put(f, xt, y + fila as u16, spans);
    }
    y += 4;
    if tt > 2.6 {
        let lema = anim::maquina(&t("arranque.lema"), tt - 2.6, 28.0);
        texto(f, xt, y, &lema, fg(SILVER));
    }
    y += 2;

    // Datos reales del equipo.
    let memoria = app
        .info
        .memoria_mb
        .map(|m| tf("arranque.mem_val", &[("mb", &m.to_string())]))
        .unwrap_or_else(|| "—".into());
    let filas = [
        (
            t("arranque.nucleo"),
            if app.info.nucleo.is_empty() {
                "Linux".into()
            } else {
                format!("Linux {}", app.info.nucleo)
            },
        ),
        (
            t("arranque.escritorio"),
            if app.info.hyprland.is_empty() {
                "Hyprland".into()
            } else {
                format!("Hyprland {}", app.info.hyprland)
            },
        ),
        (t("arranque.memoria"), memoria),
        (t("arranque.telemetria"), t("arranque.ninguna")),
        (t("arranque.usuario"), app.usuario.clone()),
    ];
    for (i, (etiqueta, valor)) in filas.iter().enumerate() {
        let aparece = 3.0 + i as f32 * 0.4;
        if tt < aparece {
            break;
        }
        let mut linea = dato(etiqueta, valor, ancho_texto.saturating_sub(7) as usize);
        if tt > aparece + 0.3 {
            linea.spans.push(Span::styled("  OK", bold(GREEN)));
        }
        put(f, xt, y + i as u16, linea.spans);
    }
    y += filas.len() as u16 + 1;

    // Saludo y llamada a la acción.
    let saludo = tf("arranque.saludo", &[("nombre", &app.usuario)]);
    if tt > 5.2 {
        let s = anim::maquina(&saludo, tt - 5.2, 22.0);
        texto(f, xt, y, &s, bold(PHOSPHOR));
    }
    if tt > 6.6 {
        let parpadeo = anim::pulso(tt, 1.4);
        let color = blend(SLATE, PHOSPHOR, parpadeo);
        put(
            f,
            xt,
            y + 2,
            vec![
                crate::ui::tecla("Enter", true),
                Span::styled(format!("  {}", t("arranque.empezar")), fg(color)),
            ],
        );
    }

    // Firma.
    let firma = t("arranque.firma");
    let fx = area.x + area.width.saturating_sub(firma.chars().count() as u16) / 2;
    texto(f, fx, area.bottom() - 1, &firma, fg(SLATE));
    texto(
        f,
        area.x + 2,
        area.bottom() - 1,
        &t("arranque.salir"),
        fg(STEEL),
    );
}

/// Pinta una celda del isotipo con el avance de su «revelado» (0 = vacía, 1 = completa).
fn pinta_celda(f: &mut Frame, x: u16, y: u16, c: char, st: Style, av: f32) {
    if av <= 0.0 {
        return;
    }
    let origen_fg = st.fg.unwrap_or(PHOSPHOR);
    let origen_bg = st.bg;
    let ch = anim::trama(av, if c == ' ' { '█' } else { c });
    let fg_c = blend(BG, origen_fg, av);
    let mut estilo = Style::new().fg(fg_c);
    if av >= 0.75
        && let Some(bg) = origen_bg
    {
        estilo = estilo.bg(bg);
    }
    let ch = if c == ' ' && av < 0.75 { ' ' } else { ch };
    put(f, x, y, vec![Span::styled(ch.to_string(), estilo)]);
    let _: Option<Color> = None;
}
