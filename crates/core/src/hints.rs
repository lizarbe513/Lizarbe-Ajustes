//! Textos de ayuda que se muestran en el pie al pasar el ratón por encima de
//! las piezas comunes (filas, controles y elementos de las ventanas).

use crate::form::Row;
use crate::i18n::t;
use crate::popup::{Popup, PopupTypes};
use crate::schema::Kind;
use crate::view::Sub;

/// «Nombre — qué hace un clic» para una fila de formulario.
pub fn row<B, A>(rows: &[Row<B, A>], i: usize) -> Option<String> {
    Some(match rows.get(i)? {
        Row::Field(f) => {
            let what = match f.def.kind {
                Kind::Bool => t("hint.click_toggle"),
                Kind::Enum(_) | Kind::Multi(_) => t("hint.click_choose"),
                _ if !f.def.presets.is_empty() => t("hint.click_choose"),
                Kind::Float {
                    min: Some(_),
                    max: Some(_),
                    ..
                } => t("hint.click_slide"),
                Kind::Int { .. } | Kind::Float { .. } => t("hint.click_step"),
                _ => t("hint.click_edit"),
            };
            format!("{} — {what}", f.def.label)
        }
        Row::Action(label, _) => format!("{label} — {}", t("hint.click_run")),
        _ => return None,
    })
}

/// Botones − / + y deslizador de una fila (`Main` es la propia fila).
pub fn ctrl(sub: Sub) -> Option<String> {
    Some(t(match sub {
        Sub::Minus => "hint.minus",
        Sub::Plus => "hint.plus",
        Sub::Slider => "hint.slider",
        Sub::Main => return None,
    }))
}

/// Descripción del elemento `i` de un selector o una lista de casillas.
pub fn popup_item<T: PopupTypes>(popup: Option<&Popup<T>>, i: usize) -> Option<String> {
    match popup? {
        Popup::Picker(p) => p.items.get(i).map(|it| it.detail.clone()),
        Popup::Checklist(c) => c.opts.get(i).map(|o| o.desc.clone()),
        _ => None,
    }
    .filter(|s| !s.is_empty())
}
