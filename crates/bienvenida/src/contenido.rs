//! Lo que la Bienvenida enseña: atajos, conceptos y herramientas de
//! configuración. Los textos están en `i18n/` (clave `atajo.*`, `con.*`, `herr.*`).

/// Un atajo: teclas a dibujar y clave del texto que lo explica.
pub struct Atajo {
    pub teclas: &'static [&'static str],
    pub clave: &'static str,
}

const fn a(teclas: &'static [&'static str], clave: &'static str) -> Atajo {
    Atajo { teclas, clave }
}

/// Grupos de atajos: (clave del título, atajos).
pub const GRUPOS: &[(&str, &[Atajo])] = &[
    (
        "grupo.basico",
        &[
            a(&["Super", "Espacio"], "atajo.menu"),
            a(&["Super", "Alt", "Espacio"], "atajo.apps"),
            a(&["Super", "Enter"], "atajo.terminal"),
            a(&["Super", "Shift", "B"], "atajo.navegador"),
            a(&["Super", "Shift", "F"], "atajo.archivos"),
            a(&["Super", "K"], "atajo.todos"),
        ],
    ),
    (
        "grupo.ventanas",
        &[
            a(&["Super", "W"], "atajo.cerrar"),
            a(&["Super", "F"], "atajo.pantalla"),
            a(&["Super", "T"], "atajo.flotante"),
            a(&["Super", "J"], "atajo.dividir"),
            a(&["Super", "← ↑ ↓ →"], "atajo.foco"),
            a(&["Super", "Shift", "← ↑ ↓ →"], "atajo.intercambiar"),
        ],
    ),
    (
        "grupo.escritorios",
        &[
            a(&["Super", "1 … 5"], "atajo.ir"),
            a(&["Super", "Shift", "1 … 5"], "atajo.mover"),
            a(&["Super", "Tab"], "atajo.siguiente"),
        ],
    ),
    (
        "grupo.sistema",
        &[
            a(&["Super", "Esc"], "atajo.sistema"),
            a(&["Impr Pant"], "atajo.captura"),
            a(&["Super", "Ctrl", "V"], "atajo.portapapeles"),
            a(&["Super", "Ctrl", "Espacio"], "atajo.fondo"),
            a(&["Super", "Shift", "Ctrl", "Espacio"], "atajo.tema"),
        ],
    ),
];

/// Cuántos atajos hay en total.
pub fn total_atajos() -> usize {
    GRUPOS.iter().map(|(_, g)| g.len()).sum()
}

/// Dibujo animado que acompaña a un concepto.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dibujo {
    Escritorios,
    Mosaico,
    Menu,
    Instalar,
    Actualizar,
    Privacidad,
    Modular,
}

pub struct Concepto {
    /// Clave base: `con.<id>` (título), `con.<id>.1` y `con.<id>.2` (párrafos).
    pub id: &'static str,
    pub dibujo: Dibujo,
}

pub const CONCEPTOS: &[Concepto] = &[
    Concepto {
        id: "escritorios",
        dibujo: Dibujo::Escritorios,
    },
    Concepto {
        id: "mosaico",
        dibujo: Dibujo::Mosaico,
    },
    Concepto {
        id: "menu",
        dibujo: Dibujo::Menu,
    },
    Concepto {
        id: "instalar",
        dibujo: Dibujo::Instalar,
    },
    Concepto {
        id: "actualizar",
        dibujo: Dibujo::Actualizar,
    },
    Concepto {
        id: "privacidad",
        dibujo: Dibujo::Privacidad,
    },
    Concepto {
        id: "modular",
        dibujo: Dibujo::Modular,
    },
];

/// Una herramienta de configuración que se puede abrir desde la Bienvenida.
pub struct Herramienta {
    /// Clave base: `herr.<id>` (nombre), `herr.<id>.d` (qué hace), `herr.<id>.ruta` (dónde está en el menú).
    pub id: &'static str,
    pub icono: &'static str,
    pub app_id: &'static str,
    pub programa: &'static str,
    pub args: &'static [&'static str],
}

pub const HERRAMIENTAS: &[Herramienta] = &[
    Herramienta {
        id: "escritorio",
        icono: "󰍹",
        app_id: "org.omarchy.lizarbe-escritorio",
        programa: "lizarbe-escritorio",
        args: &[],
    },
    Herramienta {
        id: "widgets",
        icono: "󰕮",
        app_id: "org.omarchy.lizarbe-widgets",
        programa: "lizarbe-widgets",
        args: &[],
    },
    Herramienta {
        id: "notificaciones",
        icono: "󰂚",
        app_id: "org.omarchy.lizarbe-widgets",
        programa: "lizarbe-widgets",
        args: &["--section", "notificaciones"],
    },
    Herramienta {
        id: "capturas",
        icono: "󰹑",
        app_id: "org.omarchy.lizarbe-escritorio",
        programa: "lizarbe-escritorio",
        args: &["--section", "capturas"],
    },
    Herramienta {
        id: "temas",
        icono: "󰸌",
        app_id: "org.omarchy.lizarbe-temas",
        programa: "lizarbe-temas",
        args: &[],
    },
    Herramienta {
        id: "tienda",
        icono: "󰏖",
        app_id: "org.omarchy.lizarbe-tienda",
        programa: "lizarbe-tienda",
        args: &[],
    },
    Herramienta {
        id: "centro",
        icono: "󰣆",
        app_id: "org.omarchy.lizarbe",
        programa: "lizarbe-tui",
        args: &[],
    },
];

impl Herramienta {
    /// Argumentos para `omarchy-launch-tui`.
    pub fn orden(&self) -> Vec<String> {
        let mut v = vec![
            format!("--app-id={}", self.app_id),
            self.programa.to_string(),
        ];
        v.extend(self.args.iter().map(|s| s.to_string()));
        v
    }
}

/// Enlaces de la app KDE Connect del teléfono: (clave del nombre, URL).
pub const TIENDAS: [(&str, &str); 3] = [
    ("tel.todas", "https://kdeconnect.kde.org/download.html"),
    (
        "tel.android",
        "https://play.google.com/store/apps/details?id=org.kde.kdeconnect_tp",
    ),
    (
        "tel.iphone",
        "https://apps.apple.com/app/kde-connect/id1580245991",
    ),
];

/// Logros: (id, clave del texto).
pub const LOGROS: [(&str, &str); 9] = [
    ("menu", "logro.menu"),
    ("terminal", "logro.terminal"),
    ("cerrar", "logro.cerrar"),
    ("escritorios", "logro.escritorios"),
    ("pantalla", "logro.pantalla"),
    ("fastfetch", "logro.fastfetch"),
    ("telefono", "logro.telefono"),
    ("tema", "logro.tema"),
    ("ajustes", "logro.ajustes"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_have_unique_keys_and_no_empty_combos() {
        let mut claves = std::collections::HashSet::new();
        for (_, g) in GRUPOS {
            for at in *g {
                assert!(claves.insert(at.clave), "{}", at.clave);
                assert!(!at.teclas.is_empty());
            }
        }
        assert_eq!(claves.len(), total_atajos());
    }

    #[test]
    fn tools_launch_through_omarchy_with_their_app_id() {
        let h = &HERRAMIENTAS[2];
        assert_eq!(
            h.orden(),
            vec![
                "--app-id=org.omarchy.lizarbe-widgets",
                "lizarbe-widgets",
                "--section",
                "notificaciones"
            ]
        );
        let ids: std::collections::HashSet<_> = HERRAMIENTAS.iter().map(|h| h.id).collect();
        assert_eq!(ids.len(), HERRAMIENTAS.len());
    }

    #[test]
    fn achievements_cover_every_challenge() {
        for r in crate::retos::Reto::TODOS {
            assert!(
                LOGROS.iter().any(|(id, _)| *id == r.id()) || r.id().starts_with("escritorio"),
                "{}",
                r.id()
            );
        }
    }
}
