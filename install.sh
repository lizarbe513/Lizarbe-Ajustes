#!/usr/bin/env bash
# ==============================================================================
# Instalador de Widgets (lizarbe-widgets) para Omarchy
#
#  - Compila el binario (cargo build --release) y lo copia a ~/.local/bin
#  - Registra la aplicación "Widgets"
#  - Añade "Widgets y barra" al menú de Omarchy: Menu → Setup
#  - Abre la ventana flotante y centrada en Hyprland
#  - Retira la instalación anterior (meca-qs), si existe
#
# Es idempotente: puede ejecutarse de nuevo para actualizar.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib.sh
source "$SCRIPT_DIR/scripts/lib.sh"

BIN="lizarbe-widgets"
APP_ID="org.omarchy.lizarbe-widgets"
HYPR_RULES="$HYPR_DIR/lizarbe-widgets.lua"
MENU_KEY="setup.widgets"

# ------------------------------------------------------------------ compilar
step "[1/5] Compilando $BIN…"
if ! command -v cargo &>/dev/null; then
  echo "Necesitas Rust (cargo). Instálalo con: omarchy pkg add rust" >&2
  exit 1
fi
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml" -p "$BIN"
mkdir -p "$BIN_DIR"
install -m755 "$SCRIPT_DIR/target/release/$BIN" "$BIN_DIR/$BIN"
ok "Binario en $BIN_DIR/$BIN"

# ------------------------------------------------------------------ legado
step "[2/5] Retirando la instalación anterior (meca-qs)…"
remove_legacy_meca_qs
ok "Hecho"

# ------------------------------------------------------------------ .desktop
step "[3/5] Registrando la aplicación \"Widgets\"…"
mkdir -p "$APP_DIR"
install -m644 "$SCRIPT_DIR/crates/widgets/$BIN.desktop" "$APP_DIR/$BIN.desktop"
command -v update-desktop-database &>/dev/null && update-desktop-database "$APP_DIR" &>/dev/null || true
ok "$APP_DIR/$BIN.desktop"

# ------------------------------------------------------------------ menú
step "[4/5] Añadiendo \"Widgets y barra\" a Menu → Setup…"
menu_add "$MENU_KEY" "  \"$MENU_KEY\": {
    \"icon\": \"󰕮\",
    \"label\": \"Widgets y barra\",
    \"description\": \"Barra, widgets, plugins y apariencia del shell\",
    \"action\": \"omarchy-launch-tui --app-id=$APP_ID $BIN\"
  }"
menu_refresh
ok "$MENU_EXT"

# ------------------------------------------------------------------ Hyprland
step "[5/5] Ventana flotante en Hyprland…"
mkdir -p "$HYPR_DIR"
cat >"$HYPR_RULES" <<EOF
-- Gestionado por lizarbe-widgets (install.sh). Ventana flotante y centrada
-- para el panel "Widgets" (Menu → Setup → Widgets y barra).
if o and o.window then
  o.window("$APP_ID", { tag = "-floating-window", float = true, center = true, size = { 1120, 760 } })
end
EOF
if [[ -f $HYPR_MAIN ]]; then
  if ! grep -q 'require("hypr.lizarbe-widgets")' "$HYPR_MAIN"; then
    printf '\n-- lizarbe-widgets: ventana flotante\nrequire("hypr.lizarbe-widgets")\n' >>"$HYPR_MAIN"
  fi
  hypr_reload
  ok "$HYPR_RULES"
else
  warn "No se encontró $HYPR_MAIN; se omite la regla de ventana."
fi

echo
echo -e "\033[32m✔ Widgets instalado.\033[0m Ábrelo desde Menu → Setup → Widgets y barra,"
echo "  buscando \"Widgets\" en el lanzador (Super + Espacio) o ejecutando: $BIN"
