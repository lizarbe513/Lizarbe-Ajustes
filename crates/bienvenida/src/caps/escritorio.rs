//! Capítulo 5: «su escritorio, a su gusto». Cambiar el tema en vivo cambia
//! todo el escritorio real; aquí se ve una vista previa y un botón para volver.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use crate::app::{App, Hit};
use crate::brand::*;
use crate::i18n::t;
use crate::ui::{Boton, Marco, boton, marco, texto, titulo_capitulo};
use lizarbe_core::ui::{put, truncate};

pub fn dibujar(app: &mut App, f: &mut Frame, r: Rect) {
    let h0 = titulo_capitulo(app, f, r, "cap.escritorio");
    let tt = app.cap_t();
    let y0 = r.y + h0 + 1;
    let lista_w = 34u16.min(r.width / 2);
    let alto_lista = r.bottom().saturating_sub(y0 + 3) as usize;

    // Lista de temas con una muestra de su color.
    let n = app.temas.len();
    let ini = if n <= alto_lista {
        0
    } else {
        app.tema_sel
            .saturating_sub(alto_lista / 2)
            .min(n - alto_lista)
    };
    for (k, i) in (ini..n.min(ini + alto_lista)).enumerate() {
        let y = y0 + k as u16;
        let sel = i == app.tema_sel;
        let aplicado = app.tema_aplicado.as_deref() == Some(app.temas[i].as_str());
        let muestra = app.tema_colores.get(i).copied().flatten();
        let mut spans = vec![
            Span::styled(if sel { "▍" } else { " " }, bold(rojo())),
            Span::raw(" "),
        ];
        match muestra {
            Some(c) => {
                spans.push(Span::styled("██", Style::new().fg(c.acento)));
                spans.push(Span::styled("██", Style::new().fg(c.fondo)));
            }
            None => spans.push(Span::styled("░░░░", fg(acero()))),
        }
        let nombre = truncate(&app.temas[i], lista_w.saturating_sub(10) as usize);
        spans.push(Span::styled(
            format!(" {nombre}"),
            if sel { bold(fosforo()) } else { fg(plata()) },
        ));
        if aplicado {
            spans.push(Span::styled(format!(" {SPARK}"), bold(verde())));
        }
        put(f, r.x, y, spans);
        app.hits.push((Rect::new(r.x, y, lista_w, 1), Hit::Fila(i)));
    }
    if app.temas.is_empty() {
        texto(f, r.x, y0, &t("esc.sin_temas"), fg(pizarra()));
    }

    // Vista previa: una ventana en los colores del tema elegido.
    let px = r.x + lista_w + 3;
    let pw = r.width.saturating_sub(lista_w + 3).min(60);
    if pw >= 30 {
        let colores = app.tema_colores.get(app.tema_sel).copied().flatten();
        let alto = 12.min(r.bottom().saturating_sub(y0 + 4));
        let caja = Rect::new(px, y0, pw, alto);
        if let Some(c) = colores {
            for yy in 0..caja.height {
                put(
                    f,
                    caja.x,
                    caja.y + yy,
                    vec![Span::styled(
                        " ".repeat(caja.width as usize),
                        Style::new().bg(c.fondo),
                    )],
                );
            }
            let borde = Style::new().fg(c.acento).bg(c.fondo);
            let cuerpo = Style::new().fg(c.texto).bg(c.fondo);
            let negrita = cuerpo.add_modifier(Modifier::BOLD);
            marco_color(f, caja, borde);
            put(
                f,
                caja.x + 2,
                caja.y + 1,
                vec![
                    Span::styled("lizarbe@pc ~ ❯ ", borde.add_modifier(Modifier::BOLD)),
                    Span::styled("fastfetch", cuerpo),
                ],
            );
            put(
                f,
                caja.x + 2,
                caja.y + 3,
                vec![
                    Span::styled("OS      ", borde),
                    Span::styled("Omarchy", cuerpo),
                ],
            );
            put(
                f,
                caja.x + 2,
                caja.y + 4,
                vec![
                    Span::styled("WM      ", borde),
                    Span::styled("Hyprland", cuerpo),
                ],
            );
            put(
                f,
                caja.x + 2,
                caja.y + 5,
                vec![
                    Span::styled("Tema    ", borde),
                    Span::styled(truncate(&app.temas[app.tema_sel], 30), negrita),
                ],
            );
            put(
                f,
                caja.x + 2,
                caja.y + 7,
                vec![
                    Span::styled("████", Style::new().fg(c.acento).bg(c.fondo)),
                    Span::styled("████", Style::new().fg(c.texto).bg(c.fondo)),
                ],
            );
        } else {
            marco(f, caja, Marco::Fino, acero(), "");
            texto(
                f,
                caja.x + 2,
                caja.y + 2,
                &t("esc.sin_vista"),
                fg(pizarra()),
            );
        }
        let yd = caja.bottom() + 1;
        if app.tema_ocupado {
            let giro =
                ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][(tt * 12.0) as usize % 10];
            texto(
                f,
                px,
                yd,
                &format!("{giro} {}", t("esc.aplicando")),
                bold(cadmio()),
            );
        }
        let mut xb = px;
        for (tecla_, clave, accion) in [("B", "btn.otro_fondo", 'b'), ("Z", "btn.volver_tema", 'z')]
        {
            let etiqueta = t(clave);
            if xb + etiqueta.chars().count() as u16 + 6 > r.right() || yd + 1 >= r.bottom() {
                break;
            }
            xb += boton(app, f, xb, yd + 1, Boton::new(tecla_, &etiqueta, accion)) + 3;
        }
    }
}

/// Marco fino con un estilo propio (colores del tema de la vista previa).
fn marco_color(f: &mut Frame, r: Rect, st: Style) {
    let w = r.width as usize;
    put(
        f,
        r.x,
        r.y,
        vec![Span::styled(format!("┌{}┐", "─".repeat(w - 2)), st)],
    );
    for y in 1..r.height - 1 {
        put(f, r.x, r.y + y, vec![Span::styled("│", st)]);
        put(f, r.right() - 1, r.y + y, vec![Span::styled("│", st)]);
    }
    put(
        f,
        r.x,
        r.bottom() - 1,
        vec![Span::styled(format!("└{}┘", "─".repeat(w - 2)), st)],
    );
}
