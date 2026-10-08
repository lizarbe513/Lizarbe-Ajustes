//! Paso 2: práctica en vivo. Cada reto se cumple cuando Hyprland cuenta que el
//! usuario lo hizo de verdad; debajo, lo último que su Lizarbe ha visto.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::anim;
use crate::app::App;
use crate::brand::*;
use crate::i18n::{t, tf};
use crate::retos::{Estado, Reto};
use crate::ui::{ancho_spans, combo, parrafo, titulo_capitulo};
use lizarbe_core::ui::{put, truncate};

pub fn dibujar(app: &mut App, f: &mut Frame, r: Rect) {
    let h0 = titulo_capitulo(app, f, r, "cap.practica");
    let tt = app.cap_t();
    let ancho = r.width.min(76);

    // Lista de retos: el actual lleva su pista debajo.
    let actual = app.reto_actual();
    let columna = Reto::TODOS
        .iter()
        .map(|r| ancho_spans(&combo(r.teclas(), false, "+")))
        .max()
        .unwrap_or(0);
    let mut y = r.y + h0 + 2;
    for (i, reto) in Reto::TODOS.iter().enumerate() {
        let estado = app.retos[i];
        let es_actual = actual == Some(i);
        let (marca, color) = match estado {
            Estado::Hecho => (SPARK, verde()),
            Estado::Saltado => ("·", pizarra()),
            Estado::Pendiente if es_actual => ("▶", rojo()),
            Estado::Pendiente => ("○", acero()),
        };
        let color = if es_actual {
            blend(rojo(), fosforo(), anim::pulso(tt, 1.2) * 0.5)
        } else {
            color
        };
        let mut spans = vec![Span::styled(format!("{marca} "), bold(color))];
        let teclas = combo(reto.teclas(), es_actual, "+");
        let relleno = columna - ancho_spans(&teclas);
        spans.extend(teclas);
        spans.push(Span::styled(
            format!(
                "{}   {}",
                " ".repeat(relleno as usize),
                t(&format!("reto.{}", reto.id()))
            ),
            match estado {
                Estado::Hecho | Estado::Saltado => fg(pizarra()),
                Estado::Pendiente if es_actual => bold(fosforo()),
                Estado::Pendiente => fg(plata()),
            },
        ));
        put(f, r.x, y, spans);
        y += 1;
        if es_actual {
            let pista = tf(
                "practica.pista",
                &[("pista", &t(&format!("reto.{}.pista", reto.id())))],
            );
            y += parrafo(
                f,
                r.x + 2,
                y,
                ancho.saturating_sub(2),
                &pista,
                fg(pizarra()),
            );
        }
        y += 1;
    }

    // Lo último que Hyprland ha contado.
    let y = y.min(r.bottom().saturating_sub(1));
    let vivo = if app.en_vivo() {
        Span::styled(
            format!("● {}  ", t("practica.en_vivo")),
            bold(blend(rojo(), fosforo(), anim::pulso(tt, 1.0) * 0.6)),
        )
    } else {
        Span::styled(format!("○ {}  ", t("practica.demo")), fg(pizarra()))
    };
    let ultimo = match app.ultimos.last() {
        Some((_, texto_ev)) => truncate(texto_ev, ancho.saturating_sub(20) as usize),
        None => format!(
            "{}{}",
            t("practica.esperando"),
            ".".repeat((tt * 2.0) as usize % 4)
        ),
    };
    put(f, r.x, y, vec![vivo, Span::styled(ultimo, fg(plata()))]);
}
