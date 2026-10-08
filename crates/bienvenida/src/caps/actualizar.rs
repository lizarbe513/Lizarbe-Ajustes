//! Paso previo: actualizar el sistema para que todo esté sincronizado. Se
//! puede omitir; si trae una Bienvenida nueva, esta se reabre sola.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::app::{Act, App};
use crate::brand::*;
use crate::i18n::{t, tf};
use crate::ui::{parrafo, texto, titulo_capitulo};
use lizarbe_core::ui::{put, truncate};

/// Líneas de paquetes que se enseñan antes de resumir el resto.
const MAX_LINEAS: usize = 8;

pub fn dibujar(app: &mut App, f: &mut Frame, r: Rect) {
    let h0 = titulo_capitulo(app, f, r, "cap.actualizar");
    let tt = app.cap_t();
    let mut y = r.y + h0 + 2;
    let ancho = r.width.min(76);

    y += parrafo(f, r.x, y, ancho, &t("act.motivo"), fg(pizarra())) + 1;

    match &app.act {
        Act::Buscando => {
            let giro =
                ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][(tt * 10.0) as usize % 10];
            texto(
                f,
                r.x,
                y,
                &format!("{giro} {}", t("act.buscando")),
                fg(pizarra()),
            );
        }
        Act::AlDia | Act::Actualizado => {
            let clave = if app.act == Act::AlDia {
                "act.aldia"
            } else {
                "act.hecho"
            };
            texto(f, r.x, y, &format!("{SPARK} {}", t(clave)), bold(verde()));
        }
        Act::SinRed => {
            parrafo(f, r.x, y, ancho, &t("act.sinred"), bold(cadmio()));
        }
        Act::Hay(lista) => {
            let titulo = if lista.len() == 1 {
                t("act.una")
            } else {
                tf("act.varias", &[("n", &lista.len().to_string())])
            };
            texto(f, r.x, y, &titulo, bold(fosforo()));
            y += 2;
            for linea in lista.iter().take(MAX_LINEAS) {
                if y >= r.bottom() {
                    break;
                }
                let mut partes = linea.split_whitespace();
                let (paquete, antes, despues) = (
                    partes.next().unwrap_or(""),
                    partes.next().unwrap_or(""),
                    partes.nth(1).unwrap_or(""),
                );
                // Las herramientas de Lizarbe resaltan: son las que traen esta guía.
                let color = if paquete.starts_with("lizarbe-") {
                    rojo()
                } else {
                    plata()
                };
                put(
                    f,
                    r.x + 2,
                    y,
                    vec![
                        Span::styled(format!("{:<26}", truncate(paquete, 25)), fg(color)),
                        Span::styled(format!("{antes} → {despues}"), fg(pizarra())),
                    ],
                );
                y += 1;
            }
            if lista.len() > MAX_LINEAS && y < r.bottom() {
                let resto = (lista.len() - MAX_LINEAS).to_string();
                texto(
                    f,
                    r.x + 2,
                    y,
                    &tf("act.mas", &[("n", &resto)]),
                    fg(pizarra()),
                );
            }
        }
    }
}
