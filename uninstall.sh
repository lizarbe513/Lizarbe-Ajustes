#!/usr/bin/env bash
# Desinstala meca-qs. No toca tu configuración de Omarchy (shell.json/shell.toml)
# ni las copias de seguridad en ~/.local/state/meca-qs.
set -euo pipefail

MENU_EXT="$HOME/.config/omarchy/extensions/omarchy-menu.jsonc"
HYPR_MAIN="$HOME/.config/hypr/hyprland.lua"
MENU_KEY="setup.config.quickshell"

rm -f "$HOME/.local/bin/meca-qs"
rm -f "${XDG_DATA_HOME:-$HOME/.local/share}/applications/meca-qs.desktop"
rm -f "$HOME/.config/hypr/meca-qs.lua"

if [[ -f $HYPR_MAIN ]]; then
  sed -i '/^-- meca-qs (Quickshell): ventana flotante$/d; /^require("hypr.meca-qs")$/d' "$HYPR_MAIN"
  # Quita las líneas en blanco que hayan quedado al final del archivo.
  sed -i -e :a -e '/^\n*$/{$d;N;ba' -e '}' "$HYPR_MAIN"
fi

if [[ -f $MENU_EXT ]] && grep -q "\"$MENU_KEY\"" "$MENU_EXT"; then
  cp "$MENU_EXT" "$MENU_EXT.bak.$(date +%s)"
  # Borra el bloque de la entrada (desde su clave hasta la llave que lo cierra)
  # y la coma que pudiera quedar colgando antes del cierre del objeto.
  awk -v key="\"$MENU_KEY\"" '
    { lines[NR] = $0 }
    END {
      skip = 0; n = 0
      for (i = 1; i <= NR; i++) {
        if (!skip && index(lines[i], key)) { skip = 1 }
        if (skip) { if (lines[i] ~ /^[[:space:]]*},?[[:space:]]*$/) skip = 0; continue }
        out[++n] = lines[i]
      }
      # Quita la coma final de la última entrada antes de "}".
      for (i = n; i > 0; i--) if (out[i] ~ /^[[:space:]]*}[[:space:]]*$/) { last = i; break }
      for (i = last - 1; i > 0; i--) {
        l = out[i]; gsub(/^[[:space:]]+|[[:space:]]+$/, "", l)
        if (l == "" || l ~ /^\/\//) continue
        sub(/,[[:space:]]*$/, "", out[i]); break
      }
      for (i = 1; i <= n; i++) print out[i]
    }' "$MENU_EXT" >"$MENU_EXT.tmp"
  mv "$MENU_EXT.tmp" "$MENU_EXT"
fi

command -v omarchy &>/dev/null && omarchy menu refresh &>/dev/null || true
if command -v hyprctl &>/dev/null && [[ -n ${HYPRLAND_INSTANCE_SIGNATURE:-} ]]; then
  hyprctl reload &>/dev/null || true
fi
echo "meca-qs desinstalado. Tu configuración de Omarchy no se ha modificado."
