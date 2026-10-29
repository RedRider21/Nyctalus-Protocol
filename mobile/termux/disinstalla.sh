#!/data/data/com.termux/files/usr/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Toglie SOLO le cose di Nyctalus. Non disinstalla pacchetti (python, rust, clang…):
# potrebbero servire ad altri progetti. I tuoi dati (identità, chat) si cancellano
# solo se rispondi di sì.
set -u
DEST="$HOME/nyctalus"
PREFIX="${PREFIX:-/data/data/com.termux/files/usr}"

[ -x "$DEST/nyctalus-mobile" ] && "$DEST/nyctalus-mobile" --ferma >/dev/null 2>&1
LINK="$PREFIX/bin/nyctalus-mobile"
[ "$(readlink "$LINK" 2>/dev/null)" = "$DEST/nyctalus-mobile" ] && rm -f "$LINK"
grep -q "Nyctalus" "$HOME/.termux/boot/nyctalus-mobile" 2>/dev/null && rm -f "$HOME/.termux/boot/nyctalus-mobile"
rm -rf "$DEST"
echo "Programma Nyctalus rimosso."

printf "Cancellare anche i tuoi DATI (identità/Contact ID, contatti, messaggi, allegati)? [s/N] "
read -r r
if [ "$r" = "s" ] || [ "$r" = "S" ]; then
  rm -rf "$HOME/.nyctalus-mobile" "$HOME/.nyctalus-chat"
  echo "Dati cancellati."
else
  echo "Dati conservati in ~/.nyctalus-chat e ~/.nyctalus-mobile."
fi
echo "Nota: la riga 'allow-external-apps = true' in ~/.termux/termux.properties è rimasta"
echo "      (può servire ad altre app); toglila a mano se non ti serve."
