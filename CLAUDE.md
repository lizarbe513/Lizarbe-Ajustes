# Reglas generales del desarrollo del proyecto:
- El proyecto se desarrollara sobre una interfaz TUI.
- El proyecto es un workspace de Rust con las TUIs de configuracion Lizarbe integradas al sistema Omarchy:
  - `crates/core`: nucleo compartido (idioma, preferencias, tema, escritura segura, Hyprland, dibujo).
  - `crates/widgets`: panel de configuracion de la barra, widgets y plugins Quickshell (`lizarbe-widgets`).
  - `crates/escritorio`: configuracion de Hyprland, port de Meca (`lizarbe-escritorio`, en desarrollo).
- Lo que sirva a mas de una app va en `crates/core`; lo especifico de cada app, en su crate.

# Herramientas de desarrollo:
- Se utilizara Rust como lenguaje de programacion.

# Contexto del objetivo del proyecto:
- El proyecto busca ser la herramienta de configuracion de Omarchy por interfaz TUI, sin editar archivos de texto.
- El proyecto busca lograr hacer que la configuracion sea intuitiva y sencilla.
- La herramienta estara destinada a ser utilizada por usuarios comunes sin necesidad de conocimientos tecnicos.
- Nunca se modifican archivos de los paquetes de Omarchy (`/usr/share/omarchy`); solo la configuracion del usuario.
