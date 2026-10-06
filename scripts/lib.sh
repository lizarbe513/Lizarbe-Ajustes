# Funciones compartidas por install.sh y uninstall.sh. Se cargan con `source`.

MENU_EXT="$HOME/.config/omarchy/extensions/omarchy-menu.jsonc"
HYPR_DIR="$HOME/.config/hypr"
HYPR_MAIN="$HYPR_DIR/hyprland.lua"
BIN_DIR="$HOME/.local/bin"
APP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"

step() { echo -e "\033[1;34m::\033[0m $*"; }
ok() { echo -e "   \033[32m✔\033[0m $*"; }
warn() { echo -e "   \033[33m!\033[0m $*"; }

# menu_add <clave> <entrada JSONC>: inserta la entrada antes de la última
# llave del objeto, con copia de seguridad. No hace nada si la clave ya existe.
menu_add() {
  local key=$1 entry=$2
  mkdir -p "$(dirname "$MENU_EXT")"
  if [[ ! -f $MENU_EXT ]]; then
    printf '{\n%s\n}\n' "$entry" >"$MENU_EXT"
    return
  fi
  grep -q "\"$key\"" "$MENU_EXT" && return
  cp "$MENU_EXT" "$MENU_EXT.bak.$(date +%s)"
  awk -v entry="$entry" '
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
}

# menu_remove <clave>: borra el bloque de la entrada (desde su clave hasta la
# llave que lo cierra) y la coma que pudiera quedar antes del cierre.
menu_remove() {
  local key=$1
  [[ -f $MENU_EXT ]] && grep -q "\"$key\"" "$MENU_EXT" || return 0
  cp "$MENU_EXT" "$MENU_EXT.bak.$(date +%s)"
  awk -v key="\"$key\"" '
    { lines[NR] = $0 }
    END {
      skip = 0; n = 0
      for (i = 1; i <= NR; i++) {
        if (!skip && index(lines[i], key)) { skip = 1 }
        if (skip) { if (lines[i] ~ /^[[:space:]]*},?[[:space:]]*$/) skip = 0; continue }
        out[++n] = lines[i]
      }
      for (i = n; i > 0; i--) if (out[i] ~ /^[[:space:]]*}[[:space:]]*$/) { last = i; break }
      for (i = last - 1; i > 0; i--) {
        l = out[i]; gsub(/^[[:space:]]+|[[:space:]]+$/, "", l)
        if (l == "" || l ~ /^\/\//) continue
        sub(/,[[:space:]]*$/, "", out[i]); break
      }
      for (i = 1; i <= n; i++) print out[i]
    }' "$MENU_EXT" >"$MENU_EXT.tmp"
  mv "$MENU_EXT.tmp" "$MENU_EXT"
}

# hypr_require_remove <módulo> <comentario>: quita `require("<módulo>")` y su
# comentario de hyprland.lua, y las líneas en blanco que queden al final.
hypr_require_remove() {
  local module=$1 comment=$2
  [[ -f $HYPR_MAIN ]] || return 0
  local esc_module=${module//./\\.}
  sed -i "/^$comment\$/d; /^require(\"$esc_module\")\$/d" "$HYPR_MAIN"
  sed -i -e :a -e '/^\n*$/{$d;N;ba' -e '}' "$HYPR_MAIN"
}

hypr_reload() {
  if command -v hyprctl &>/dev/null && [[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} ]]; then
    hyprctl reload &>/dev/null || true
    local errors
    errors="$(hyprctl configerrors 2>/dev/null || true)"
    if [[ -n ${errors//[[:space:]]/} ]]; then
      warn "Hyprland informa errores de configuración:"
      echo "$errors"
    fi
  fi
}

menu_refresh() {
  command -v omarchy &>/dev/null && omarchy menu refresh &>/dev/null || true
}

# Restos de cuando Widgets se llamaba meca-qs.
remove_legacy_meca_qs() {
  rm -f "$BIN_DIR/meca-qs" "$APP_DIR/meca-qs.desktop" "$HYPR_DIR/meca-qs.lua"
  hypr_require_remove "hypr.meca-qs" "-- meca-qs (Quickshell): ventana flotante"
  menu_remove "setup.config.quickshell"
}
