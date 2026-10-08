//! La práctica en vivo: cada reto se cumple cuando Hyprland informa de lo que
//! el usuario acaba de hacer (abrir el menú, una terminal, cambiar de escritorio…).

use lizarbe_core::hypr_events::HyprEvent;

/// Clase de ventana de la propia Bienvenida (para no contarla como «terminal nueva»).
pub const CLASE_PROPIA: &str = "org.omarchy.lizarbe-bienvenida";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reto {
    Menu,
    Terminal,
    Cerrar,
    Escritorio2,
    Escritorio1,
    Pantalla,
}

impl Reto {
    pub const TODOS: [Reto; 6] = [
        Reto::Menu,
        Reto::Terminal,
        Reto::Cerrar,
        Reto::Escritorio2,
        Reto::Escritorio1,
        Reto::Pantalla,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Reto::Menu => "menu",
            Reto::Terminal => "terminal",
            Reto::Cerrar => "cerrar",
            Reto::Escritorio2 => "escritorio2",
            Reto::Escritorio1 => "escritorio1",
            Reto::Pantalla => "pantalla",
        }
    }

    /// Teclas del reto, como lista de teclas para dibujarlas.
    pub fn teclas(self) -> &'static [&'static str] {
        match self {
            Reto::Menu => &["Super", "Espacio"],
            Reto::Terminal => &["Super", "Enter"],
            Reto::Cerrar => &["Super", "W"],
            Reto::Escritorio2 => &["Super", "2"],
            Reto::Escritorio1 => &["Super", "1"],
            Reto::Pantalla => &["Super", "F"],
        }
    }

    /// Logro que se otorga al cumplir este reto.
    pub fn id_logro(self) -> &'static str {
        match self {
            Reto::Menu => "menu",
            Reto::Terminal => "terminal",
            Reto::Cerrar => "cerrar",
            Reto::Pantalla => "pantalla",
            Reto::Escritorio2 | Reto::Escritorio1 => "escritorios",
        }
    }

    /// ¿Este evento de Hyprland cumple el reto?
    pub fn cumple(self, ev: &HyprEvent) -> bool {
        match self {
            Reto::Menu => ev.name == "openlayer" && ev.data == "omarchy-menu",
            Reto::Terminal => {
                // openwindow>>dirección,escritorio,clase,título
                ev.name == "openwindow"
                    && ev.data.split(',').nth(2).is_some_and(|c| c != CLASE_PROPIA)
            }
            Reto::Cerrar => ev.name == "closewindow",
            Reto::Escritorio2 => ev.name == "workspace" && ev.data == "2",
            Reto::Escritorio1 => ev.name == "workspace" && ev.data == "1",
            Reto::Pantalla => ev.name == "fullscreen",
        }
    }
}

/// Frase corta para el «radar» de la práctica; `None` si el evento no interesa.
pub fn describir(ev: &HyprEvent) -> Option<String> {
    use crate::i18n::tf;
    Some(match ev.name.as_str() {
        "workspace" => tf("radar.escritorio", &[("n", &ev.data)]),
        "openwindow" => {
            let clase = ev.data.split(',').nth(2).unwrap_or("?");
            if clase == CLASE_PROPIA {
                return None;
            }
            tf("radar.abre", &[("clase", clase)])
        }
        "closewindow" => tf("radar.cierra", &[]),
        "openlayer" if ev.data == "omarchy-menu" => tf("radar.menu_abre", &[]),
        "closelayer" if ev.data == "omarchy-menu" => tf("radar.menu_cierra", &[]),
        "fullscreen" => {
            if ev.data == "1" {
                tf("radar.pantalla_si", &[])
            } else {
                tf("radar.pantalla_no", &[])
            }
        }
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Estado {
    Pendiente,
    Hecho,
    Saltado,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(name: &str, data: &str) -> HyprEvent {
        HyprEvent {
            name: name.into(),
            data: data.into(),
        }
    }

    #[test]
    fn each_challenge_matches_its_own_event_only() {
        assert!(Reto::Menu.cumple(&ev("openlayer", "omarchy-menu")));
        assert!(!Reto::Menu.cumple(&ev("openlayer", "omarchy-bar")));
        assert!(Reto::Terminal.cumple(&ev("openwindow", "80a,1,kitty,~")));
        assert!(!Reto::Terminal.cumple(&ev(
            "openwindow",
            "80a,1,org.omarchy.lizarbe-bienvenida,Bienvenida"
        )));
        assert!(Reto::Cerrar.cumple(&ev("closewindow", "80a")));
        assert!(Reto::Escritorio2.cumple(&ev("workspace", "2")));
        assert!(!Reto::Escritorio2.cumple(&ev("workspace", "3")));
        assert!(Reto::Escritorio1.cumple(&ev("workspace", "1")));
        assert!(Reto::Pantalla.cumple(&ev("fullscreen", "1")));
        // Un evento no cumple retos que no son suyos.
        for r in Reto::TODOS {
            assert!(!r.cumple(&ev("activewindow", "x,y")));
        }
    }

    #[test]
    fn ids_are_unique_and_keys_present() {
        let mut ids: Vec<_> = Reto::TODOS.iter().map(|r| r.id()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), Reto::TODOS.len());
        assert!(Reto::TODOS.iter().all(|r| r.teclas().len() == 2));
    }
}
