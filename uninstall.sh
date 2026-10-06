#!/usr/bin/env bash
# Desinstala Widgets (lizarbe-widgets) y los restos de meca-qs. No toca tu
# configuración de Omarchy (shell.json/shell.toml) ni las copias de seguridad
# en ~/.local/state/lizarbe.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib.sh
source "$SCRIPT_DIR/scripts/lib.sh"

rm -f "$BIN_DIR/lizarbe-widgets" "$APP_DIR/lizarbe-widgets.desktop" "$HYPR_DIR/lizarbe-widgets.lua"
hypr_require_remove "hypr.lizarbe-widgets" "-- lizarbe-widgets: ventana flotante"
menu_remove "setup.widgets"
remove_legacy_meca_qs

menu_refresh
hypr_reload
echo "Widgets desinstalado. Tu configuración de Omarchy no se ha modificado."
