//! Núcleo compartido de las TUIs de configuración Lizarbe (Escritorio y
//! Widgets): idioma, preferencias, paleta del tema, escritura segura de
//! archivos, comunicación con Hyprland y primitivas de dibujo.

pub mod fsutil;
pub mod hypr;
pub mod i18n;
pub mod ipc;
pub mod prefs;
pub mod term;
pub mod theme;
pub mod ui;
