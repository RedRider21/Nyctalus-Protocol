#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Impronta uniforme delle schede 🦇 del browser del PC: una pagina misura tutto quello che misura
# un sito che traccia (identità, lingua, ora, schermo, WebGL, audio, canvas, WebRTC, cornici). Due
# avvii con fuso orario e lingua del PC DIVERSI devono dare un'impronta IDENTICA.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · impronta uniforme del browser del PC (schede 🦇)"
[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] || { salta "nessun display grafico"; exit 0; }
python3 -c 'import gi; gi.require_version("WebKit2","4.0"); from gi.repository import WebKit2' 2>/dev/null || { salta "WebKit2GTK non installato"; exit 0; }
giro() { # n fuso lingua
  local H="$TMP/home$1"; mkdir -p "$H/.nyctalus-browser" "$H/.config"
  echo '{"stealth": true, "socks_port": '"$((22190 + $1))"', "homepage": "about:blank"}' > "$H/.nyctalus-browser/config.json"
  HOME="$H" XDG_CONFIG_HOME="$H/.config" TZ="$2" LANG="$3" LC_ALL="$3" PYTHONPATH="$ROOT/pc/browser" \
    timeout 90 python3 "$STRUMENTI/prova_impronta_pc.py" $((22190 + $1)) > "$TMP/r$1.txt" 2>"$TMP/e$1.txt" \
    || echo "KO prova interrotta: $(grep -v -i warning "$TMP/e$1.txt" | tail -2)" >> "$TMP/r$1.txt"
}
giro 1 Europe/Rome it_IT.UTF-8
giro 2 America/New_York en_GB.UTF-8
while read -r esito resto; do
  case "$esito" in OK) pass "$resto" ;; KO) fail "$resto" ;; esac
done < <(grep -E "^(OK|KO) " "$TMP/r1.txt")
I1=$(grep "^IMPRONTA " "$TMP/r1.txt"); I2=$(grep "^IMPRONTA " "$TMP/r2.txt")
[ -n "$I1" ] && [ "$I1" = "$I2" ] && pass "due PC con fuso (Roma / New York) e lingua (it / en-GB) diversi: impronta IDENTICA" \
  || fail "impronte diverse: $(diff <(echo "$I1" | tr ',' '\n') <(echo "$I2" | tr ',' '\n') | head -6 | tr '\n' ' ')"
[ "$KO" -eq 0 ]
