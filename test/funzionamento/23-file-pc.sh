#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Il programma File del PC: la finestra sopra il motore di Bob, sulla rete di prova; Alice gli
# manda un file sotto soglia (arriva da solo) e uno sopra (si tocca «Accetta»). Tutto nel PC.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · programma File (finestra del PC)"
[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] || { salta "nessun display grafico"; exit 0; }
assicura_binario
rete_di_prova 21600
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:21680 --hops 3 --control 127.0.0.1:21690 > "$TMP/ca.log" 2>&1 & PIDS+=($!)
sleep 2
SA=alicesegreto0123456789abcdef; SB=bobsegreto0123456789abcdef00
identita alice 21611 21621 21680 "$SA"; ALICE=$ADDR
identita bob 21612 21622 21680 "$SB"; BOB=$ADDR
[ -n "$ALICE" ] && [ -n "$BOB" ] || { fail "identità non partite"; exit 1; }
sleep 5
mkdir -p "$TMP/home/.config"
HOME="$TMP/home" XDG_CONFIG_HOME="$TMP/home/.config" NYCTALUS_CHAT_PORT=21622 NYCTALUS_CHAT_TOKEN="$SB" \
  PYTHONPATH="$ROOT/pc/file:$ROOT/pc/browser" \
  timeout 400 python3 "$STRUMENTI/prova_file_pc.py" 21621 "$SA" "$BOB" "$ALICE" "$TMP" "${NYCT_FOTO:-$TMP}/file-pc.png" > "$TMP/r.txt" 2>"$TMP/e.txt" \
  || echo "KO prova interrotta (uscita $?): $(grep -v -i warning "$TMP/e.txt" | tail -2)" >> "$TMP/r.txt"
while read -r esito resto; do
  case "$esito" in OK) pass "$resto" ;; KO) fail "$resto" ;; esac
done < "$TMP/r.txt"
[ "$KO" -eq 0 ]
