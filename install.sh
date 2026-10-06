#!/usr/bin/env bash
# ==============================================================================
# Instalador de Lizarbe Ajustes para Omarchy: Escritorio y Widgets
#
#  - Compila los binarios (cargo build --release) y los copia a ~/.local/bin
#  - Registra las aplicaciones "Escritorio" y "Widgets" en el lanzador
#  - Abre sus ventanas flotantes y centradas en Hyprland
#  - Añade "Widgets y barra" al menú de Omarchy: Menu → Setup
#  - Retira la instalación anterior de Widgets (meca-qs), si existe
#
# Es idempotente: puede ejecutarse de nuevo para actualizar.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib.sh
source "$SCRIPT_DIR/scripts/lib.sh"

step "[1/4] Compilando…"
if ! command -v cargo &>/dev/null; then
  echo "Necesitas Rust (cargo). Instálalo con: omarchy pkg add rust" >&2
  exit 1
fi
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml" -p lizarbe-widgets -p lizarbe-escritorio
ok "Hecho"

step "[2/4] Instalando Escritorio y Widgets…"
remove_legacy_meca_qs
remove_legacy_meca
install_app lizarbe-escritorio escritorio
install_app lizarbe-widgets widgets
command -v update-desktop-database &>/dev/null && update-desktop-database "$APP_DIR" &>/dev/null || true
ok "Binarios en $BIN_DIR"

step "[3/4] Conectando Escritorio y Widgets al menú de Omarchy…"
menu_add "setup.widgets" "  \"setup.widgets\": {
    \"icon\": \"󰕮\",
    \"label\": \"Widgets y barra\",
    \"description\": \"Barra, widgets, plugins y apariencia del shell\",
    \"action\": \"omarchy-launch-tui --app-id=org.omarchy.lizarbe-widgets lizarbe-widgets\"
  }"
menu_install_escritorio
menu_refresh
ok "$MENU_EXT"

step "[4/4] Recargando Hyprland…"
hypr_reload
ok "Hecho"

echo
echo -e "\033[32m✔ Lizarbe Ajustes instalado.\033[0m"
echo "  Widgets:    Menu → Setup → Widgets y barra, o: lizarbe-widgets"
echo "  Escritorio: búscalo como \"Escritorio\" en el lanzador (Super + Espacio), o: lizarbe-escritorio"
