#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Chiamate vocali, motore: su una rete di prova (circuiti a 3 salti, punto d'incontro)
# Alice chiama Bob, Bob risponde, 2 secondi di audio a pacchetti da 20 ms nei due sensi
# arrivano identici con poco ritardo, poi fine; un secondo invito viene rifiutato.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · chiamate (motore)"
assicura_binario
rete_di_prova 21000
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:21080 --hops 3 --control 127.0.0.1:21090 > "$TMP/c.log" 2>&1 & PIDS+=($!)
sleep 2
SA=alicesegreto0123456789abcdef; SB=bobsegreto0123456789abcdef00
identita alice 21011 21021 21080 "$SA"; ALICE=$ADDR
identita bob 21012 21022 21080 "$SB"; BOB=$ADDR
[ -n "$ALICE" ] && [ -n "$BOB" ] || { fail "identità non partite"; exit 1; }
sleep 4
python3 "$STRUMENTI/prova_chiamata.py" 21021 "$SA" 21022 "$SB" "$ALICE" "$BOB" > "$TMP/r.txt" 2>&1 || echo "KO prova interrotta: $(tail -1 "$TMP/r.txt")" >> "$TMP/r.txt"
while read -r esito resto; do
  case "$esito" in OK) pass "$resto" ;; KO) fail "$resto" ;; esac
done < "$TMP/r.txt"
[ "$KO" -eq 0 ]
