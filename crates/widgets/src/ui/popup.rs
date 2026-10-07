//! Ventanas emergentes de Widgets: los tipos genéricos del núcleo con los
//! destinos y acciones propios de esta aplicación.

use lizarbe_core::popup as core;
pub use lizarbe_core::popup::{HelpContent, PickItem, PopupTypes, fold};

use crate::store::{Bind, Scope};

/// Qué hacer cuando el usuario confirma.
#[derive(Debug, Clone, PartialEq)]
pub enum Confirm {
    Apply,
    Discard,
    Quit,
    RestartShell,
    TogglePlugin(String),
    Restore(Scope),
}

/// Cuadros de texto propios de Widgets.
#[derive(Debug, Clone, PartialEq)]
pub enum WInput {
    NewRawKey { entry: Bind },
    RawValue { entry: Bind, key: String },
    GitUrl,
    NewModule { qml: bool, at: (usize, usize) },
}

/// Selectores propios de Widgets.
#[derive(Debug, Clone, PartialEq)]
pub enum WPick {
    AddWidget {
        at: (usize, usize),
    },
    /// Búsqueda de opciones: salta a la elegida.
    Search,
}

/// Acción de una entrada del menú contextual.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuAct {
    Activate(usize),
    ResetField(usize),
    WidgetSettings(usize, usize),
    WidgetMove(usize, usize, usize),
    WidgetAdd(usize),
    WidgetRemove(usize, usize),
    /// Tecla a reenviar a la lista de plugins (`E` = Enter).
    PluginKey(usize, char),
    Apply,
    Cancel,
    Restore,
    Help,
}

#[derive(Debug, Clone, PartialEq)]
pub struct W;

impl PopupTypes for W {
    type Bind = Bind;
    type Confirm = Confirm;
    type Input = WInput;
    type Pick = WPick;
    type Menu = MenuAct;
}

pub type Popup = core::Popup<W>;
pub type Input = core::Input<W>;
pub type InputTarget = core::InputTarget<W>;
pub type Picker = core::Picker<W>;
pub type PickTarget = core::PickTarget<W>;
pub type Menu = core::Menu<W>;
pub type MenuItem = core::MenuItem<W>;
pub type Outcome = core::Outcome<W>;
