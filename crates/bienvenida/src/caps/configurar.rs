//! Capítulo 6: configurar sin editar archivos. Lista de herramientas con su
//! explicación y un botón para abrirlas ahora mismo.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::app::{App, Hit};
use crate::brand::*;
use crate::contenido::HERRAMIENTAS;
use crate::i18n::t;
use crate::ui::{Boton, Marco, boton, marco, parrafo, texto, titulo_capitulo};
use lizarbe_core::ui::{put, truncate};

pub fn dibujar(app: &mut App, f: &mut Frame, r: Rect) {
    let h0 = titulo_capitulo(app, f, r, "cap.configurar");
    let y0 = r.y + h0 + 1;
    let lista_w = 36u16.min(r.width / 2);

    for (i, h) in HERRAMIENTAS.iter().enumerate() {
        let y = y0 + i as u16 * 2;
        if y + 1 >= r.bottom() {
            break;
        }
        let sel = i == app.herramienta_sel;
        let abierta = app.hover == Some(Hit::Fila(i));
        put(
            f,
            r.x,
            y,
            vec![
                Span::styled(if sel { "▍ " } else { "  " }, bold(RED)),
                Span::styled(
                    format!("{}  ", h.icono),
                    if sel { bold(RED) } else { fg(SLATE) },
                ),
                Span::styled(
                    truncate(
                        &t(&format!("herr.{}", h.id)),
                        lista_w.saturating_sub(7) as usize,
                    ),
                    if sel {
                        bold(PHOSPHOR)
                    } else if abierta {
                        bold(CADMIUM)
                    } else {
                        fg(SILVER)
                    },
                ),
            ],
        );
        app.hits.push((Rect::new(r.x, y, lista_w, 2), Hit::Fila(i)));
    }

    // Ficha de la herramienta elegida.
    let h = &HERRAMIENTAS[app.herramienta_sel.min(HERRAMIENTAS.len() - 1)];
    let fx = r.x + lista_w + 3;
    let fw = r.width.saturating_sub(lista_w + 3);
    if fw < 30 {
        return;
    }
    let alto = r.bottom().saturating_sub(y0).min(14);
    let caja = Rect::new(fx, y0, fw, alto);
    marco(f, caja, Marco::Grueso, STEEL, &t(&format!("herr.{}", h.id)));
    texto(f, caja.x + 3, caja.y + 2, h.icono, bold(RED));
    let mut y = caja.y + 4;
    y += parrafo(
        f,
        caja.x + 3,
        y,
        caja.width - 6,
        &t(&format!("herr.{}.d", h.id)),
        fg(PHOSPHOR),
    ) + 1;
    put(
        f,
        caja.x + 3,
        y,
        vec![
            Span::styled(format!("{}  ", t("herr.donde")), fg(SLATE)),
            Span::styled(t(&format!("herr.{}.ruta", h.id)), fg(SILVER)),
        ],
    );
    // Botón «Abrir ahora».
    let yb = caja.bottom().saturating_sub(2);
    boton(
        app,
        f,
        caja.x + 3,
        yb,
        Boton::new("Enter", &t("herr.abrir"), '\n').principal(),
    );
    if app.tiene("ajustes") {
        texto(
            f,
            caja.x + 3,
            yb.saturating_sub(1),
            &format!("{SPARK} {}", t("herr.abierta")),
            fg(GREEN),
        );
    }
}
