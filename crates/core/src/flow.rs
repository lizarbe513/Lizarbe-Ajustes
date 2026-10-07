//! Ventanas de confirmación comunes de los paneles con cambios pendientes:
//! aplicar, descartar, salir y restaurar. Cada aplicación decide qué hace al
//! confirmar (su `Confirm`).

use crate::i18n::{t, tf};
use crate::popup::{Popup, PopupTypes};

/// «¿Aplicar n cambios?» con la lista de cambios.
pub fn apply<T: PopupTypes>(lines: Vec<String>, action: T::Confirm) -> Popup<T> {
    Popup::Confirm {
        title: tf("confirm.apply", &[("n", &lines.len().to_string())]),
        lines,
        action,
    }
}

/// Descartar los cambios pendientes.
pub fn discard<T: PopupTypes>(action: T::Confirm) -> Popup<T> {
    Popup::Confirm {
        title: t("confirm.discard.title"),
        lines: vec![t("confirm.discard")],
        action,
    }
}

/// Salir con cambios sin aplicar.
pub fn quit<T: PopupTypes>(action: T::Confirm) -> Popup<T> {
    Popup::Confirm {
        title: t("confirm.quit.title"),
        lines: vec![t("confirm.quit")],
        action,
    }
}

/// Restaurar lo que dice la clave `scope` (título) y `scope.desc` (detalle).
pub fn restore<T: PopupTypes>(scope: &str, action: T::Confirm) -> Popup<T> {
    Popup::Confirm {
        title: t(scope),
        lines: vec![
            t(&format!("{scope}.desc")),
            String::new(),
            t("restore.note"),
        ],
        action,
    }
}
