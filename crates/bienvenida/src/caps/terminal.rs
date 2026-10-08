//! Capítulo 3: la terminal de juguete. El usuario escribe órdenes reales e
//! inofensivas y ve su resultado sin miedo a romper nada.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use crate::app::App;
use crate::brand::*;
use crate::i18n::t;
use crate::ui::{Marco, marco, recortar, titulo_capitulo};
use lizarbe_core::ui::put;

pub fn dibujar(app: &mut App, f: &mut Frame, r: Rect) {
    let h0 = titulo_capitulo(app, f, r, "cap.terminal");
    let tt = app.cap_t();
    let y0 = r.y + h0 + 1;
    let alto = r.bottom().saturating_sub(y0).clamp(6, 14);
    let ventana = Rect::new(r.x, y0, r.width.min(90), alto);
    marco(f, ventana, Marco::Fino, SLATE, "");
    // Barra de título de la «ventana».
    put(
        f,
        ventana.x + 2,
        ventana.y,
        vec![
            Span::styled(" ● ", fg(RED)),
            Span::styled("● ", fg(CADMIUM)),
            Span::styled("● ", fg(GREEN)),
            Span::styled(format!(" lizarbe — {} ", t("term.titulo")), bold(PHOSPHOR)),
        ],
    );
    let interior = Rect::new(
        ventana.x + 2,
        ventana.y + 1,
        ventana.width - 4,
        ventana.height - 2,
    );
    let aviso = |s: &str| Span::styled(s.to_string(), fg(SLATE));
    let prompt = || {
        vec![
            Span::styled(format!("{} ", app.usuario.to_lowercase()), bold(GREEN)),
            Span::styled("~ ", fg(COBALT)),
            Span::styled("❯ ", bold(RED)),
        ]
    };

    // Historial aplanado (orden + salida), mostrando lo último que cabe.
    let mut lineas: Vec<Vec<Span<'static>>> = vec![];
    if app.historial.is_empty() {
        lineas.push(vec![aviso(&t("term.bienvenida"))]);
        lineas.push(vec![]);
    }
    for (orden, salida) in &app.historial {
        let mut l = prompt();
        l.push(Span::styled(orden.clone(), fg(PHOSPHOR)));
        lineas.push(l);
        for s in salida {
            let spans: Vec<Span<'static>> = s.spans.clone();
            lineas.push(spans);
        }
        lineas.push(vec![]);
    }
    let filas_visibles = interior.height.saturating_sub(1) as usize;
    let fin = lineas
        .len()
        .saturating_sub(app.term_scroll.min(lineas.len().saturating_sub(1)));
    let ini = fin.saturating_sub(filas_visibles);
    for (i, l) in lineas[ini..fin].iter().enumerate() {
        put(
            f,
            interior.x,
            interior.y + i as u16,
            recortar(l, interior.width as usize),
        );
    }
    // Línea de entrada.
    let y_in = interior.bottom() - 1;
    let mut linea = prompt();
    linea.push(Span::styled(app.entrada.clone(), fg(PHOSPHOR)));
    let cursor_on = ((tt * 2.0) as u32).is_multiple_of(2);
    linea.push(Span::styled(if cursor_on { "█" } else { " " }, fg(RED)));
    if app.term_ocupado {
        let giro = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][(tt * 12.0) as usize % 10];
        linea.push(Span::styled(
            format!("  {giro} {}", t("term.ejecutando")),
            fg(CADMIUM),
        ));
    } else {
        let sug = app.sugerencia();
        let resto: String = sug.chars().skip(app.entrada.chars().count()).collect();
        if !resto.is_empty() {
            linea.push(Span::styled(resto, fg(SLATE)));
            linea.push(Span::styled("   [Tab]", fg(STEEL)));
        }
    }
    put(
        f,
        interior.x,
        y_in,
        recortar(&linea, interior.width as usize),
    );

    let _: Option<Line> = None;
}
