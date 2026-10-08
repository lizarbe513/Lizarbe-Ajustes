//! Capítulo 2: práctica en vivo. Cada reto se cumple cuando Hyprland cuenta
//! que el usuario lo hizo de verdad; a la derecha, un «radar» con lo que se ve.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::anim;
use crate::app::App;
use crate::brand::*;
use crate::i18n::{t, tf};
use crate::retos::{Estado, Reto};
use crate::ui::{Marco, combo, marco, parrafo, texto, titulo_capitulo};
use lizarbe_core::ui::{put, truncate};

pub fn dibujar(app: &mut App, f: &mut Frame, r: Rect) {
    let h0 = titulo_capitulo(app, f, r, "cap.practica");
    let tt = app.cap_t();
    let y0 = r.y + h0 + 1;
    let izq_w = (r.width * 58 / 100).clamp(40, 66);
    let der_x = r.x + izq_w + 2;
    let der_w = r.width.saturating_sub(izq_w + 2);

    // Lista de retos.
    let actual = app.reto_actual();
    let mut y = y0;
    for (i, reto) in Reto::TODOS.iter().enumerate() {
        let estado = app.retos[i];
        let es_actual = actual == Some(i);
        let (marca, color) = match estado {
            Estado::Hecho => (SPARK, GREEN),
            Estado::Saltado => ("·", SLATE),
            Estado::Pendiente if es_actual => ("▶", RED),
            Estado::Pendiente => ("○", SLATE),
        };
        let pulso = if es_actual { anim::pulso(tt, 1.2) } else { 0.0 };
        let color = if es_actual {
            blend(RED, PHOSPHOR, pulso * 0.5)
        } else {
            color
        };
        let mut spans = vec![Span::styled(format!("{marca} "), bold(color))];
        spans.extend(combo(reto.teclas(), es_actual, "+"));
        let titulo = format!("  {}", t(&format!("reto.{}", reto.id())));
        spans.push(Span::styled(
            titulo,
            if estado == Estado::Hecho {
                fg(SLATE)
            } else if es_actual {
                bold(PHOSPHOR)
            } else {
                fg(SILVER)
            },
        ));
        put(f, r.x, y, spans);
        y += 1;
        if es_actual {
            let texto_d = t(&format!("reto.{}.d", reto.id()));
            let filas = parrafo(f, r.x + 2, y, izq_w.saturating_sub(2), &texto_d, fg(SILVER));
            y += filas;
            let pista = tf(
                "practica.pista",
                &[("pista", &t(&format!("reto.{}.pista", reto.id())))],
            );
            y += parrafo(f, r.x + 2, y, izq_w.saturating_sub(2), &pista, fg(SLATE));
        }
        y += 1;
    }

    // Avance y avisos.
    let fila_pie = r.bottom().saturating_sub(2);
    let total = Reto::TODOS.len();
    let hechos = app.retos_cumplidos();
    put(
        f,
        r.x,
        fila_pie,
        vec![
            Span::styled(anim::barra(hechos as f32 / total as f32, 20), fg(RED)),
            Span::styled(format!("  {hechos}/{total}"), fg(SILVER)),
        ],
    );
    if actual.is_none() {
        texto(
            f,
            r.x,
            fila_pie + 1,
            &format!("{SPARK} {}", t("practica.listo")),
            bold(GREEN),
        );
    } else if !app.en_vivo() {
        texto(
            f,
            r.x,
            fila_pie + 1,
            &truncate(&t("practica.sin_hyprland"), r.width as usize),
            bold(CADMIUM),
        );
    } else {
        texto(
            f,
            r.x,
            fila_pie + 1,
            &truncate(&t("practica.saltar"), izq_w as usize),
            fg(SLATE),
        );
    }

    // Radar: lo que Hyprland cuenta.
    if der_w < 24 {
        return;
    }
    let alto = (r.bottom() - y0).min(12);
    let caja = Rect::new(der_x, y0, der_w, alto);
    marco(f, caja, Marco::Fino, STEEL, &t("practica.radar"));
    let vivo = if app.en_vivo() {
        Span::styled(
            format!("● {}", t("practica.en_vivo")),
            bold(blend(RED, PHOSPHOR, anim::pulso(tt, 1.0) * 0.6)),
        )
    } else {
        Span::styled(format!("○ {}", t("practica.demo")), fg(SLATE))
    };
    put(f, caja.x + 2, caja.y + 1, vec![vivo]);
    if app.ultimos.is_empty() {
        let puntos = ".".repeat((tt * 2.0) as usize % 4);
        texto(
            f,
            caja.x + 2,
            caja.y + 3,
            &format!("{}{puntos}", t("practica.esperando")),
            fg(SLATE),
        );
    }
    for (i, (cuando, texto_ev)) in app
        .ultimos
        .iter()
        .rev()
        .take(alto.saturating_sub(4) as usize)
        .enumerate()
    {
        let hace = (app.t - cuando).max(0.0) as u32;
        let linea = format!("{:>3} s  {}", hace, texto_ev);
        let color = if i == 0 { PHOSPHOR } else { SILVER };
        texto(
            f,
            caja.x + 2,
            caja.y + 3 + i as u16,
            &truncate(&linea, der_w.saturating_sub(4) as usize),
            fg(color),
        );
    }
}
