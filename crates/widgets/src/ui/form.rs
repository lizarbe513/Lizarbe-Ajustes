//! Formularios de Widgets sobre los formularios genéricos del núcleo.

pub use lizarbe_core::form::{FormState, NoteKind, nudge};

use crate::store::Bind;

/// Acciones disparadas desde filas tipo botón.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    AddRawKey(Bind),
    CreateQmlTemplate(String),
    OpenEditor(std::path::PathBuf),
    RestartShell,
}

pub type Row = lizarbe_core::form::Row<Bind, Act>;
pub type FieldRow = lizarbe_core::form::FieldRow<Bind>;
