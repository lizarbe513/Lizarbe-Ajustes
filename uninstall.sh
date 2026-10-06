#!/usr/bin/env bash
# Desinstala Escritorio y Widgets (y los restos de meca-qs). No toca tu
# configuración (shell.json, shell.toml, escritorio.lua) ni las copias de
# seguridad en ~/.local/state/lizarbe.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib.sh
source "$SCRIPT_DIR/scripts/lib.sh"

uninstall_app lizarbe-escritorio
uninstall_app lizarbe-widgets
menu_remove "setup.widgets"
remove_legacy_meca_qs

menu_refresh
hypr_reload
echo "Lizarbe Ajustes desinstalado. Tu configuración no se ha modificado."
