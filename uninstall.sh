#!/usr/bin/env bash
# Desinstala las copias de desarrollo de Escritorio, Widgets y Estudio de temas. No toca tu
# configuración (shell.json, shell.toml, escritorio.lua) ni las copias de
# seguridad en ~/.local/state/lizarbe.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib.sh
source "$SCRIPT_DIR/scripts/lib.sh"

uninstall_app lizarbe-escritorio
uninstall_app lizarbe-widgets
uninstall_app lizarbe-temas
echo "Lizarbe Ajustes (copias de desarrollo) desinstalado. Tu configuración no se ha modificado."
echo "El menú y las reglas de ventana pertenecen al paquete lizarbe-menu."
