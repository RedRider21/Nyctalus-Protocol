#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Programma File, motore: trasferimenti di file grandi fra Alice e Bob sulla rete di prova
# (circuiti a 3 salti, più canali in parallelo): sotto soglia parte da solo, sopra soglia si
# accetta, pausa e ripresa, motore di Bob spento a metà e riacceso (riprende), file sempre
# identico; chi non è fra i contatti viene rifiutato. Tutto dentro il PC (127.0.0.1).
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · trasferimenti di file grandi (motore)"
assicura_binario
rete_di_prova 21500
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:21580 --hops 3 --control 127.0.0.1:21590 > "$TMP/ca.log" 2>&1 & PIDS+=($!)
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:21581 --hops 3 --control 127.0.0.1:21591 > "$TMP/cb.log" 2>&1 & PIDS+=($!)
sleep 2
SA=alicesegreto0123456789abcdef; SB=bobsegreto0123456789abcdef00
identita alice 21511 21521 21580 "$SA"; ALICE=$ADDR
# il motore di Bob (chi scarica) rallentato SOLO in questa prova: pausa e spegnimento a metà
NYCTALUS_PROVA_RALLENTA_MS=120 identita bob 21512 21522 21581 "$SB"; BOB=$ADDR
[ -n "$ALICE" ] && [ -n "$BOB" ] || { fail "identità non partite"; exit 1; }
sleep 5
# per riaccendere il motore di Bob a metà trasferimento (stessa cartella dati)
RIAVVIA="NYCTALUS_PROVA_RALLENTA_MS=120 NYCTALUS_CHAT_TOKEN=$SB '$BIN' chat --data '$TMP/bob' --backend 127.0.0.1:21512 --control 127.0.0.1:21522 --socks 127.0.0.1:21581 >> '$TMP/bob.chat2.log' 2>&1 & echo \$! > '$TMP/bob2.pid'"
timeout 900 python3 "$STRUMENTI/prova_trasferimenti.py" 21521 "$SA" 21522 "$SB" "$ALICE" "$BOB" "$TMP" "$RIAVVIA" > "$TMP/r.txt" 2>&1 \
  || echo "KO prova interrotta: $(tail -1 "$TMP/r.txt")" >> "$TMP/r.txt"
[ -s "$TMP/bob2.pid" ] && kill "$(cat "$TMP/bob2.pid")" 2>/dev/null
while read -r esito resto; do
  case "$esito" in OK) pass "$resto" ;; KO) fail "$resto" ;; esac
done < "$TMP/r.txt"
[ "$KO" -eq 0 ]
