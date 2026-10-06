#!/usr/bin/env bash
# ==============================================================================
# Instalador de meca-qs (Quickshell) para Omarchy
#
#  - Compila el binario (cargo build --release) y lo copia a ~/.local/bin
#  - Registra la entrada de escritorio "Quickshell"
#  - Añade "Quickshell" al menú de Omarchy: Menu → Setup → Config
#  - Abre la ventana flotante y centrada en Hyprland
#
# Es idempotente: puede ejecutarse de nuevo para actualizar.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN_DIR="$HOME/.local/bin"
APP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
MENU_EXT="$HOME/.config/omarchy/extensions/omarchy-menu.jsonc"
HYPR_DIR="$HOME/.config/hypr"
HYPR_RULES="$HYPR_DIR/meca-qs.lua"
HYPR_MAIN="$HYPR_DIR/hyprland.lua"
MENU_KEY="setup.config.quickshell"

step() { echo -e "\033[1;34m::\033[0m $*"; }
ok() { echo -e "   \033[32m✔\033[0m $*"; }
warn() { echo -e "   \033[33m!\033[0m $*"; }

# ------------------------------------------------------------------ compilar
step "[1/4] Compilando meca-qs…"
if ! command -v cargo &>/dev/null; then
  echo "Necesitas Rust (cargo). Instálalo con: omarchy pkg add rust" >&2
  exit 1
fi
cargo build --release --manifest-path "$SCRIPT_DIR/Cargo.toml"
mkdir -p "$BIN_DIR"
install -m755 "$SCRIPT_DIR/target/release/meca-qs" "$BIN_DIR/meca-qs"
ok "Binario en $BIN_DIR/meca-qs"

# ------------------------------------------------------------------ .desktop
step "[2/4] Registrando la aplicación \"Quickshell\"…"
mkdir -p "$APP_DIR"
install -m644 "$SCRIPT_DIR/meca-qs.desktop" "$APP_DIR/meca-qs.desktop"
command -v update-desktop-database &>/dev/null && update-desktop-database "$APP_DIR" &>/dev/null || true
ok "$APP_DIR/meca-qs.desktop"

# ------------------------------------------------------------------ menú
step "[3/4] Añadiendo Quickshell a Menu → Setup → Config…"
mkdir -p "$(dirname "$MENU_EXT")"
ENTRY="  \"$MENU_KEY\": {
    \"icon\": \"󰕮\",
    \"label\": \"Quickshell\",
    \"description\": \"Widgets, barra y apariencia del shell\",
    \"action\": \"omarchy-launch-tui --app-id=org.omarchy.meca-qs meca-qs\"
  }"
if [[ ! -f $MENU_EXT ]]; then
  printf '{\n%s\n}\n' "$ENTRY" >"$MENU_EXT"
  ok "Creado $MENU_EXT"
elif grep -q "\"$MENU_KEY\"" "$MENU_EXT"; then
  ok "La entrada ya existía en $MENU_EXT"
else
  cp "$MENU_EXT" "$MENU_EXT.bak.$(date +%s)"
  # Inserta la entrada antes de la última llave de cierre del objeto JSONC.
  awk -v entry="$ENTRY" '
    { lines[NR] = $0 }
    END {
      last = 0
      for (i = NR; i > 0; i--) if (lines[i] ~ /}[[:space:]]*$/) { last = i; break }
      # ¿Hay alguna entrada antes (algo distinto de comentarios y "{")?
      prev = 0
      for (i = last - 1; i > 0; i--) {
        l = lines[i]; gsub(/^[[:space:]]+|[[:space:]]+$/, "", l)
        if (l == "" || l ~ /^\/\//) continue
        prev = i; break
      }
      for (i = 1; i <= NR; i++) {
        if (i == prev) {
          l = lines[i]; sub(/[[:space:]]+$/, "", l)
          if (l !~ /[{,]$/) lines[i] = l ","
        }
        if (i == last) {
          sub(/}[[:space:]]*$/, "", lines[i])
          if (lines[i] !~ /^[[:space:]]*$/) print lines[i]
          print entry
          print "}"
        } else {
          print lines[i]
        }
      }
    }' "$MENU_EXT" >"$MENU_EXT.tmp"
  mv "$MENU_EXT.tmp" "$MENU_EXT"
  ok "Entrada añadida a $MENU_EXT (copia de seguridad: $MENU_EXT.bak.*)"
fi
command -v omarchy &>/dev/null && omarchy menu refresh &>/dev/null || true

# ------------------------------------------------------------------ Hyprland
step "[4/4] Ventana flotante en Hyprland…"
mkdir -p "$HYPR_DIR"
cat >"$HYPR_RULES" <<'EOF'
-- Gestionado por meca-qs (install.sh). Ventana flotante y centrada para el
-- panel "Quickshell" (Menu → Setup → Config → Quickshell).
if o and o.window then
  o.window("org.omarchy.meca-qs", { tag = "-floating-window", float = true, center = true, size = { 1120, 760 } })
end
EOF
if [[ -f $HYPR_MAIN ]]; then
  if ! grep -q 'require("hypr.meca-qs")' "$HYPR_MAIN"; then
    printf '\n-- meca-qs (Quickshell): ventana flotante\nrequire("hypr.meca-qs")\n' >>"$HYPR_MAIN"
  fi
  if command -v hyprctl &>/dev/null && [[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} ]]; then
    hyprctl reload &>/dev/null || true
    errors="$(hyprctl configerrors 2>/dev/null || true)"
    if [[ -n ${errors//[[:space:]]/} ]]; then
      warn "Hyprland informa errores de configuración:"
      echo "$errors"
    fi
  fi
  ok "$HYPR_RULES"
else
  warn "No se encontró $HYPR_MAIN; se omite la regla de ventana."
fi

echo
echo -e "\033[32m✔ meca-qs instalado.\033[0m Ábrelo desde Menu → Setup → Config → Quickshell,"
echo "  buscando \"Quickshell\" en el lanzador (Super + Espacio) o ejecutando: meca-qs"
