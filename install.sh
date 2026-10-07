#!/usr/bin/env bash
# ==============================================================================
# Instalador de Lizarbe Ajustes para Omarchy: Escritorio, Widgets y Estudio de temas
#
#  - Compila los binarios (cargo build --release) y los copia a ~/.local/bin
#  - Registra "Escritorio", "Widgets", "Crear tema" y la Tienda en el lanzador
#  - Llama a lizarbe-doctor (paquete lizarbe-menu) para el menú y las ventanas
#
# Solo para desarrollo: los usuarios la reciben con el paquete lizarbe-ajustes.
# Es idempotente: puede ejecutarse de nuevo para actualizar.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib.sh
source "$SCRIPT_DIR/scripts/lib.sh"

step "[1/3] Compilando…"
if ! command -v cargo &>/dev/null; then
  echo "Necesitas Rust (cargo). Instálalo con: omarchy pkg add rust" >&2
  exit 1
fi
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml" -p lizarbe-widgets -p lizarbe-escritorio -p lizarbe-temas -p lizarbe-tienda
ok "Hecho"

step "[2/3] Instalando Escritorio, Widgets, Estudio de temas y Tienda…"
install_app lizarbe-escritorio escritorio
install_app lizarbe-widgets widgets
install_app lizarbe-temas temas
install_app lizarbe-tienda tienda
command -v update-desktop-database &>/dev/null && update-desktop-database "$APP_DIR" &>/dev/null || true
ok "Binarios en $BIN_DIR"

step "[3/3] Menú de Omarchy y reglas de ventana…"
sync_integration
ok "Hecho"

echo
echo -e "\033[32m✔ Lizarbe Ajustes instalado.\033[0m"
echo "  Menú → Configuración y Apariencia, o: lizarbe-escritorio / lizarbe-widgets / lizarbe-temas / lizarbe-tienda"
