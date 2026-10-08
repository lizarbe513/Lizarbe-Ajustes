//! Paso 0: el arranque. Solo el isotipo con efecto de monitor CRT (encendido,
//! rayas de barrido, rejilla de píxeles, parpadeo), el nombre de la marca y
//! una invitación a empezar.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::anim;
use crate::app::App;
use crate::brand::*;
use crate::i18n::{t, tf};
use crate::ui::{limpiar, parrafo, tecla};
use lizarbe_core::ui::put;

/// Segundos tras los que ya se puede continuar (un Enter antes lo salta todo).
pub const LISTO: f32 = 3.2;
/// Hueco entre el isotipo y el texto.
const HUECO: u16 = 6;

pub fn dibujar(app: &mut App, f: &mut Frame, area: Rect) {
    let tt = app.cap_t();
    limpiar(f, area);
    let logo = isotipo();
    let lw = logo.iter().map(|l| l.width()).max().unwrap_or(0) as u16;
    let lh = logo.len() as u16;
    let marca = logotipo(1);
    let mw = marca[0].chars().count() as u16;
    let total_w = (lw + HUECO + mw).min(area.width);
    let x0 = area.x + (area.width - total_w) / 2;
    let y0 = area.y + area.height.saturating_sub(lh) / 2;

    crt_isotipo(f, area, x0, y0, &logo, lw, tt);

    // A la derecha: el nombre, el saludo y la invitación, centrados con el isotipo.
    let xt = x0 + lw + HUECO;
    let ancho_texto = area.right().saturating_sub(xt + 1).max(1);
    let yt = y0 + lh.saturating_sub(7) / 2;
    let g = anim::smooth((tt - 0.9) / 1.4);
    for (fila, linea) in marca.iter().enumerate() {
        let mut spans = vec![];
        for (x, c) in linea.chars().enumerate() {
            if x as u16 >= ancho_texto {
                break;
            }
            let av = anim::avance_celda(g, x as u32 + 40, fila as u32);
            let color = crt_color(
                blend(STEEL, PHOSPHOR, av),
                x as u32 + xt as u32,
                (yt - y0) as u32 + fila as u32,
                tt,
            );
            spans.push(Span::styled(
                anim::trama(av, c).to_string(),
                Style::new().fg(color),
            ));
        }
        put(f, xt, yt + fila as u16, spans);
    }

    if tt > 1.8 {
        let saludo = tf("arranque.saludo", &[("nombre", &app.usuario)]);
        let visible = anim::maquina(&saludo, tt - 1.8, 24.0);
        parrafo(f, xt, yt + 4, ancho_texto, &visible, fg(SILVER));
    }
    if tt > LISTO - 0.4 {
        let color = blend(SLATE, PHOSPHOR, anim::pulso(tt, 1.4));
        put(
            f,
            xt,
            yt + 6,
            vec![
                tecla("Enter", true),
                Span::styled(format!("  {}", t("arranque.empezar")), fg(color)),
            ],
        );
    }
}

/// El isotipo como en un monitor de tubo: se enciende como una línea que se
/// abre, se aclara por tramas y queda con rayas de barrido, una rejilla de
/// píxeles, un haz que baja despacio, parpadeo y algún salto de línea.
fn crt_isotipo(
    f: &mut Frame,
    area: Rect,
    x0: u16,
    y0: u16,
    logo: &[ratatui::text::Line<'static>],
    lw: u16,
    tt: f32,
) {
    if tt < 0.05 {
        return;
    }
    let lh = logo.len() as i32;
    let centro_x = lw as f32 / 2.0;
    let centro_y = lh as f32 / 2.0;
    // 1.º la línea se ensancha, 2.º se abre hacia arriba y abajo.
    let abre_ancho = anim::smooth(tt / 0.35);
    let abre_alto = anim::smooth((tt - 0.35) / 0.8);
    let g = anim::smooth((tt - 0.35) / 1.9);
    // Salto de línea ocasional, ya con la imagen formada.
    let ciclo = tt % 5.0;
    let fotograma = (tt * 30.0) as u32;
    let con_salto = tt > 2.4 && ciclo < 0.16;

    for (y, linea) in logo.iter().enumerate() {
        let dy = (y as f32 + 0.5 - centro_y).abs();
        if dy > abre_alto * centro_y + 0.5 {
            continue;
        }
        // Cerca del encendido la imagen brilla casi blanca.
        let brillo_encendido = (1.0 - abre_alto) * 0.7;
        let salto: i32 = if con_salto && anim::hash2(fotograma, y as u32) > 0.82 {
            if anim::hash2(fotograma, y as u32 + 99) > 0.5 {
                1
            } else {
                -1
            }
        } else {
            0
        };
        let mut x = 0u32;
        for span in &linea.spans {
            for c in span.content.chars() {
                let cx = x;
                x += 1;
                if c == ' ' {
                    continue;
                }
                let dx = (cx as f32 + 0.5 - centro_x).abs();
                if dx > abre_ancho * centro_x + 0.5 {
                    continue;
                }
                let av = anim::avance_celda(g, cx, y as u32);
                if av <= 0.0 {
                    continue;
                }
                let px = x0 as i32 + cx as i32 + salto;
                if px < area.x as i32 || px >= area.right() as i32 {
                    continue;
                }
                let tono = |c: Color| {
                    let c = blend(c, PHOSPHOR, brillo_encendido);
                    crt_color(blend(BG, c, av), cx, y as u32, tt)
                };
                let mut estilo = Style::new().fg(tono(span.style.fg.unwrap_or(PHOSPHOR)));
                if av >= 0.75
                    && let Some(bg) = span.style.bg
                {
                    estilo = estilo.bg(tono(bg));
                }
                put(
                    f,
                    px as u16,
                    y0 + y as u16,
                    vec![Span::styled(anim::trama(av, c).to_string(), estilo)],
                );
            }
        }
    }
    // Mientras se abre, la línea de encendido: una raya blanca en el centro.
    if abre_alto < 0.2 {
        let ancho = (abre_ancho * lw as f32) as u16;
        let x = x0 + lw.saturating_sub(ancho) / 2;
        let y = y0 + (lh / 2) as u16;
        let linea = "━".repeat(ancho as usize);
        put(f, x, y, vec![Span::styled(linea, bold(PHOSPHOR))]);
    }
}

/// Aplica el aspecto de tubo a un color de la celda (x, y) del dibujo:
/// raya de barrido en las filas impares, rejilla en una de cada tres
/// columnas, un haz claro que baja y un parpadeo leve.
fn crt_color(c: Color, x: u32, y: u32, tt: f32) -> Color {
    let mut c = c;
    if y % 2 == 1 {
        c = blend(c, BG, 0.28);
    }
    if x % 3 == 2 {
        c = blend(c, BG, 0.10);
    }
    let haz = (tt * 4.5) % 26.0 - 5.0;
    let cerca = 1.0 - ((y as f32 - haz).abs() / 2.0).min(1.0);
    if cerca > 0.0 {
        c = blend(c, PHOSPHOR, cerca * 0.30);
    }
    let fotograma = (tt * 20.0) as u32;
    let mut apagado = 0.06 * anim::hash2(fotograma, 3);
    if anim::hash2(fotograma, 5) > 0.97 {
        apagado += 0.15;
    }
    blend(c, BG, apagado)
}
