//! Paso 1: la tecla Super. Un teclado dibujado con Super encendida y tres
//! ejemplos de «Super + algo».

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::anim;
use crate::app::App;
use crate::brand::*;
use crate::i18n::t;
use crate::ui::{Marco, ancho_spans, combo, marco, parrafo, texto, titulo_capitulo};
use lizarbe_core::ui::{center, put};

pub fn dibujar(app: &mut App, f: &mut Frame, r: Rect) {
    let h0 = titulo_capitulo(app, f, r, "cap.super");
    let tt = app.cap_t();
    let mut y = r.y + h0 + 2;

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
            blend(rojo(), fosforo(), pulso * 0.55)
        } else {
            acero()
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
        let st = if es_super { bold(color) } else { fg(pizarra()) };
        texto(f, x + 1, y + 1, &center(&etiqueta, (w - 2) as usize), st);
        if es_super {
            x_super = x;
        }
        x += w + 1;
    }
    y += 3;
    texto(
        f,
        x_super + 1,
        y,
        &format!("▲ {}", t("super.esta")),
        bold(rojo()),
    );
    y += 2;

    // Explicación breve.
    let ancho = r.width.min(76);
    let xp = r.x + r.width.saturating_sub(ancho) / 2;
    y += parrafo(f, xp, y, ancho, &t("super.texto"), fg(plata())) + 1;

    // «Super + algo»: tres ejemplos en líneas sueltas, sin cajas.
    let ejemplos: [(&[&str], &str); 3] = [
        (&["Super", "Enter"], "super.ej.terminal"),
        (&["Super", "Espacio"], "super.ej.menu"),
        (&["Super", "W"], "super.ej.cerrar"),
    ];
    let col = ejemplos
        .iter()
        .map(|(k, _)| ancho_spans(&combo(k, false, "+")))
        .max()
        .unwrap_or(0);
    for (i, (teclas, clave)) in ejemplos.iter().enumerate() {
        if y >= r.bottom() || tt < 0.4 + i as f32 * 0.3 {
            continue;
        }
        let mut spans = combo(teclas, false, "+");
        let relleno = col - ancho_spans(&spans);
        spans.push(Span::styled(
            format!("{}   {}", " ".repeat(relleno as usize), t(clave)),
            fg(plata()),
        ));
        put(f, xp, y, spans);
        y += 2;
    }
}
