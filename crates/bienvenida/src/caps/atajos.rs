//! Capítulo 8: los atajos imprescindibles, agrupados, con la tecla dibujada.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::app::App;
use crate::brand::*;
use crate::contenido::{GRUPOS, total_atajos};
use crate::i18n::{t, tf};
use crate::ui::{ancho_spans, combo, texto, titulo_capitulo};
use lizarbe_core::ui::{put, truncate};

enum Fila {
    Titulo(&'static str),
    Atajo(usize, usize),
}

pub fn dibujar(app: &mut App, f: &mut Frame, r: Rect) {
    let h0 = titulo_capitulo(app, f, r, "cap.atajos");
    let y0 = r.y + h0 + 1;
    let alto = r.bottom().saturating_sub(y0 + 1) as usize;

    // Filas aplanadas: título de grupo y sus atajos.
    let mut filas = vec![];
    let mut indice_global = vec![];
    let mut n: usize = 0;
    for (g, (clave, atajos)) in GRUPOS.iter().enumerate() {
        filas.push(Fila::Titulo(clave));
        for k in 0..atajos.len() {
            filas.push(Fila::Atajo(g, k));
            indice_global.push(filas.len() - 1);
            n += 1;
        }
    }
    let sel = app.atajos_scroll.min(n.saturating_sub(1));
    let fila_sel = indice_global[sel];
    // Ventana que mantiene visible la fila elegida (y su título si cabe).
    let ini = (fila_sel + 1).saturating_sub(alto);
    let col_teclas = 38u16.min(r.width / 2);
    for (i, fila) in filas.iter().enumerate().skip(ini).take(alto) {
        let y = y0 + (i - ini) as u16;
        match fila {
            Fila::Titulo(clave) => {
                texto(
                    f,
                    r.x,
                    y,
                    &format!("{SPARK} {}", t(clave).to_uppercase()),
                    bold(rojo()),
                );
            }
            Fila::Atajo(g, k) => {
                let a = &GRUPOS[*g].1[*k];
                let es_sel = i == fila_sel;
                let c = combo(a.teclas, es_sel, "");
                let w = ancho_spans(&c);
                let x = r.x + 2 + col_teclas.saturating_sub(w);
                put(f, x, y, c);
                let desc = truncate(&t(a.clave), r.width.saturating_sub(col_teclas + 6) as usize);
                put(
                    f,
                    r.x + col_teclas + 4,
                    y,
                    vec![
                        Span::styled(if es_sel { "▍ " } else { "  " }, bold(rojo())),
                        Span::styled(desc, if es_sel { bold(fosforo()) } else { fg(plata()) }),
                    ],
                );
            }
        }
    }
    // Pie: contador y recordatorio.
    let yp = r.bottom() - 1;
    texto(
        f,
        r.x,
        yp,
        &tf(
            "atajos.contador",
            &[
                ("n", &(sel + 1).to_string()),
                ("total", &total_atajos().to_string()),
            ],
        ),
        fg(pizarra()),
    );
    let k = combo(&["Super", "K"], false, "+");
    let kw = ancho_spans(&k);
    let aviso = t("atajos.siempre");
    let ax = r
        .right()
        .saturating_sub(kw + 3 + aviso.chars().count() as u16);
    put(f, ax, yp, k);
    texto(f, ax + kw + 2, yp, &aviso, fg(cadmio()));
}
