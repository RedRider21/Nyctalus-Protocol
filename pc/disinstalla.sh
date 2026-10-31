#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Toglie SOLO Nyctalus (programma, comandi, voci del menu, avvio automatico).
# Non disinstalla pacchetti di sistema. I tuoi dati (identità, chat) si cancellano
# solo se rispondi di sì.   Uso:  ./disinstalla.sh   [--si per non chiedere]
set -u
DEST="${XDG_DATA_HOME:-$HOME/.local/share}/nyctalus"
APP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
AUTO_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"
SI=0; [ "${1:-}" = "--si" ] && SI=1

for c in launcher browser chat; do
  L="$HOME/.local/bin/nyctalus-$c"
  [ "$(readlink "$L" 2>/dev/null)" = "$DEST/avvia-$c.sh" ] && rm -f "$L"
done
for c in rete sito; do
  [ "$(readlink "$HOME/.local/bin/nyctalus-$c" 2>/dev/null)" = "$DEST/gestore-rete/nyctalus-$c" ] && rm -f "$HOME/.local/bin/nyctalus-$c"
done
for f in "$APP_DIR"/nyctalus-launcher.desktop "$APP_DIR"/nyctalus-browser.desktop "$APP_DIR"/nyctalus-chat.desktop \
         "$AUTO_DIR"/nyctalus-launcher.desktop; do
  grep -q '^X-Nyctalus=1' "$f" 2>/dev/null && rm -f "$f"
done
case "$DEST" in */nyctalus) rm -rf "$DEST" ;; esac
echo "Programma Nyctalus rimosso."

if [ "$SI" = 1 ]; then r=n; else
  [ -d "$HOME/.nyctalus-rete" ] && echo "NB: ~/.nyctalus-rete (chiavi della TUA RETE, se la gestisci) non viene mai cancellata da qui."
  [ -d "$HOME/.nyctalus-siti" ] && echo "NB: ~/.nyctalus-siti (i tuoi siti .nyct e le loro CHIAVI) non viene mai cancellata da qui."
  printf "Cancellare anche i tuoi DATI (identità/Contact ID, contatti, messaggi, allegati, impostazioni)? [s/N] "
  read -r r
fi
if [ "$r" = s ] || [ "$r" = S ]; then
  rm -rf "$HOME/.nyctalus-chat" "$HOME/.nyctalus-browser" "$HOME/.nyctalus-launcher" "${XDG_CACHE_HOME:-$HOME/.cache}/nyctalus-chat"
  echo "Dati cancellati."
else
  echo "Dati conservati (~/.nyctalus-chat, ~/.nyctalus-browser, ~/.nyctalus-launcher)."
fi
