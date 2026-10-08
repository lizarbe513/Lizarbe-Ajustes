//! Capítulo final: el «certificado». Logros obtenidos, chispas que caen y la
//! llamada a empezar a usar el equipo.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;

use crate::anim;
use crate::app::App;
use crate::brand::*;
use crate::contenido::LOGROS;
use crate::i18n::{t, tf};
use crate::ui::{Boton, Marco, boton, limpiar, marco, texto};
use lizarbe_core::ui::{center, put};

pub fn dibujar(app: &mut App, f: &mut Frame, area: Rect) {
    let tt = app.cap_t();
    limpiar(f, area);
    chispas(f, area, app.t);

    let w = 72u16.min(area.width.saturating_sub(4));
    let h = 20u16.min(area.height.saturating_sub(2));
    let r = Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    );
    limpiar(f, r);
    marco(f, r, Marco::Doble, RED, "");
    texto(
        f,
        r.x + 2,
        r.y,
        &format!(" {SPARK} {} ", t("final.certificado")),
        bold(RED),
    );

    // Logotipo.
    let logo = logotipo(1);
    let lw = logo[0].chars().count() as u16;
    let glob = anim::smooth(tt / 1.4);
    for (fila, linea) in logo.iter().enumerate() {
        let mut spans = vec![];
        for (x, c) in linea.chars().enumerate() {
            let av = anim::avance_celda(glob, x as u32, fila as u32);
            spans.push(Span::styled(
                anim::trama(av, c).to_string(),
                fg(blend(STEEL, PHOSPHOR, av)),
            ));
        }
        put(
            f,
            r.x + (r.width.saturating_sub(lw)) / 2,
            r.y + 2 + fila as u16,
            spans,
        );
    }
    let mut y = r.y + 6;
    texto(
        f,
        r.x + 2,
        y,
        &center(&t("final.listo").to_uppercase(), (r.width - 4) as usize),
        bold(PHOSPHOR),
    );
    y += 1;
    texto(
        f,
        r.x + 2,
        y,
        &center(
            &tf("final.para", &[("nombre", &app.usuario)]),
            (r.width - 4) as usize,
        ),
        fg(SILVER),
    );
    y += 2;

    // Logros en dos columnas.
    let mitad = LOGROS.len().div_ceil(2);
    let col_w = (r.width - 6) / 2;
    for (i, (id, clave)) in LOGROS.iter().enumerate() {
        let (col, fila) = (i / mitad, i % mitad);
        let x = r.x + 3 + col as u16 * col_w;
        let tiene = app.tiene(id);
        put(
            f,
            x,
            y + fila as u16,
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
    y += mitad as u16 + 1;
    let total = LOGROS.len();
    let texto_total = tf(
        "final.total",
        &[
            ("n", &app.logros.len().to_string()),
            ("total", &total.to_string()),
        ],
    );
    texto(
        f,
        r.x + 2,
        y,
        &center(&texto_total, (r.width - 4) as usize),
        fg(CADMIUM),
    );
    y += 2;

    // Botones.
    if tt > 1.2 && y + 1 < r.bottom() {
        boton(
            app,
            f,
            r.x + 4,
            y,
            Boton::new("Enter", &t("final.empezar"), '\n').principal(),
        );
        if y + 1 < r.bottom() - 1 {
            boton(
                app,
                f,
                r.x + 4,
                y + 1,
                Boton::new("R", &t("final.repetir"), 'r'),
            );
        }
    }
    let firma = t("final.firma");
    texto(
        f,
        area.x + (area.width.saturating_sub(firma.chars().count() as u16)) / 2,
        area.bottom() - 1,
        &firma,
        fg(SLATE),
    );
}

/// Chispas ✦ que caen despacio por toda la pantalla.
fn chispas(f: &mut Frame, area: Rect, t: f32) {
    let colores = [RED, CADMIUM, TEAL, COBALT, PHOSPHOR];
    for k in 0..28u32 {
        let x = (anim::hash2(k, 1) * area.width as f32) as u16;
        let velocidad = 0.8 + anim::hash2(k, 2) * 1.6;
        let y = ((anim::hash2(k, 3) * area.height as f32 + t * velocidad) as u16) % area.height;
        let color = colores[(k as usize) % colores.len()];
        let brillo = anim::pulso(t + k as f32, 2.0);
        let ch = if k % 3 == 0 { SPARK } else { "·" };
        texto(
            f,
            area.x + x.min(area.width - 1),
            area.y + y,
            ch,
            fg(blend(STEEL, color, 0.35 + 0.65 * brillo)),
        );
    }
}
