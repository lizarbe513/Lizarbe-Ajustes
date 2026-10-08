//! Capítulo 1: la tecla Super. Un teclado dibujado con Super encendida y
//! tres ejemplos de «Super + algo».

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::anim;
use crate::app::App;
use crate::brand::*;
use crate::i18n::t;
use crate::ui::{Marco, ancho_spans, centrada, combo, marco, parrafo, texto, titulo_capitulo};
use lizarbe_core::ui::{center, put};

pub fn dibujar(app: &mut App, f: &mut Frame, r: Rect) {
    let h0 = titulo_capitulo(app, f, r, "cap.super");
    let tt = app.cap_t();
    let mut y = r.y + h0 + 1;

    // El teclado: fila inferior con Super encendida.
    let teclas: [(&str, u16); 6] = [
        ("Ctrl", 8),
        ("Fn", 6),
        ("Super", 11),
        ("Alt", 7),
        ("Espacio", 24),
        ("Alt", 7),
    ];
    let total: u16 = teclas.iter().map(|(_, w)| w).sum::<u16>() + (teclas.len() as u16 - 1);
    let mut x = r.x + r.width.saturating_sub(total) / 2;
    let pulso = anim::pulso(tt, 1.4);
    let mut x_super = x;
    for (nombre, w) in teclas {
        let es_super = nombre == "Super";
        let color = if es_super {
            blend(RED, PHOSPHOR, pulso * 0.55)
        } else {
            SLATE
        };
        let caja = Rect::new(x, y, w, 3);
        marco(
            f,
            caja,
            if es_super { Marco::Grueso } else { Marco::Fino },
            color,
            "",
        );
        let etiqueta = if es_super {
            format!("{SPARK} Super")
        } else {
            nombre.to_string()
        };
        let st = if es_super { bold(color) } else { fg(SILVER) };
        texto(f, x + 1, y + 1, &center(&etiqueta, (w - 2) as usize), st);
        if es_super {
            x_super = x;
        }
        x += w + 1;
    }
    y += 3;
    if tt > 0.8 {
        let flecha = format!("▲ {}", t("super.esta"));
        texto(f, x_super + 1, y, &flecha, bold(RED));
    }
    y += 2;

    // Explicación.
    let ancho = r.width.min(86);
    let xp = r.x + r.width.saturating_sub(ancho) / 2;
    y += parrafo(f, xp, y, ancho, &t("super.texto"), fg(PHOSPHOR)) + 1;
    y += parrafo(f, xp, y, ancho, &t("super.mac"), fg(SLATE)) + 1;

    // «Super + algo = una orden».
    centrada(
        f,
        Rect::new(r.x, y, r.width, 1),
        y,
        vec![Span::styled(t("super.formula"), bold(CADMIUM))],
    );
    y += 2;
    let ejemplos: [(&[&str], &str); 3] = [
        (&["Super", "Enter"], "super.ej.terminal"),
        (&["Super", "Espacio"], "super.ej.menu"),
        (&["Super", "W"], "super.ej.cerrar"),
    ];
    let hueco = 2;
    let w = (r.width.saturating_sub(hueco * 2) / 3).min(34);
    let total = w * 3 + hueco * 2;
    let x0 = r.x + r.width.saturating_sub(total) / 2;
    let alto = r.bottom().saturating_sub(y).min(5);
    if alto < 4 {
        return;
    }
    for (i, (teclas, clave)) in ejemplos.iter().enumerate() {
        if tt < 0.6 + i as f32 * 0.35 {
            continue;
        }
        let caja = Rect::new(x0 + i as u16 * (w + hueco), y, w, alto);
        marco(f, caja, Marco::Fino, STEEL, "");
        let c = combo(teclas, false, "+");
        let cw = ancho_spans(&c);
        put(f, caja.x + caja.width.saturating_sub(cw) / 2, caja.y + 1, c);
        texto(
            f,
            caja.x + 2,
            caja.y + 3,
            &center(&t(clave), (w - 4) as usize),
            fg(SILVER),
        );
    }
}
