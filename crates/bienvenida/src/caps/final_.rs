//! Paso final: el nombre de la marca, los logros obtenidos y la llamada a
//! empezar a usar el equipo.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::anim;
use crate::app::App;
use crate::brand::*;
use crate::contenido::LOGROS;
use crate::i18n::{t, tf};
use crate::ui::{Boton, boton, centrada, limpiar};
use lizarbe_core::ui::put;

pub fn dibujar(app: &mut App, f: &mut Frame, area: Rect) {
    let tt = app.cap_t();
    limpiar(f, area);

    let marca = logotipo(1);
    let mw = marca[0].chars().count() as u16;
    let mitad = LOGROS.len().div_ceil(2) as u16;
    // Marca (3) + título (2) + logros + contador (2) + botones (3).
    let alto = 3 + 2 + 2 + mitad + 2 + 3;
    let mut y = area.y + area.height.saturating_sub(alto) / 2;

    // Nombre de la marca, que se aclara por tramas.
    let g = anim::smooth(tt / 1.4);
    for (fila, linea) in marca.iter().enumerate() {
        let mut spans = vec![];
        for (x, c) in linea.chars().enumerate() {
            let av = anim::avance_celda(g, x as u32, fila as u32);
            spans.push(Span::styled(
                anim::trama(av, c).to_string(),
                fg(blend(STEEL, PHOSPHOR, av)),
            ));
        }
        put(
            f,
            area.x + area.width.saturating_sub(mw) / 2,
            y + fila as u16,
            spans,
        );
    }
    y += 4;
    centrada(
        f,
        area,
        y,
        vec![Span::styled(
            format!("{SPARK} {}", t("final.listo")),
            bold(PHOSPHOR),
        )],
    );
    y += 2;

    // Logros en dos columnas.
    let col_w = 34u16.min(area.width / 2);
    let x_logros = area.x + area.width.saturating_sub(col_w * 2) / 2;
    for (i, (id, clave)) in LOGROS.iter().enumerate() {
        let (col, fila) = (i as u16 / mitad, i as u16 % mitad);
        let tiene = app.tiene(id);
        put(
            f,
            x_logros + col * col_w,
            y + fila,
            vec![
                Span::styled(
                    if tiene {
                        format!("{SPARK} ")
                    } else {
                        "○ ".to_string()
                    },
                    if tiene { bold(RED) } else { fg(STEEL) },
                ),
                Span::styled(t(clave), if tiene { fg(PHOSPHOR) } else { fg(SLATE) }),
            ],
        );
    }
    y += mitad + 1;
    let total = tf(
        "final.total",
        &[
            ("n", &app.logros.len().to_string()),
            ("total", &LOGROS.len().to_string()),
        ],
    );
    centrada(f, area, y, vec![Span::styled(total, fg(SLATE))]);
    y += 2;

    // Un solo botón grande; repetir queda discreto debajo.
    if tt > 0.8 {
        let etiqueta = t("final.empezar");
        let ancho = 8 + etiqueta.chars().count() as u16;
        let x = area.x + area.width.saturating_sub(ancho) / 2;
        boton(
            app,
            f,
            x,
            y,
            Boton::new("Enter", &etiqueta, '\n').principal(),
        );
        let repetir = t("final.repetir");
        let x = area.x
            + area
                .width
                .saturating_sub(repetir.chars().count() as u16 + 4)
                / 2;
        boton(app, f, x, y + 2, Boton::new("R", &repetir, 'r'));
    }
}
