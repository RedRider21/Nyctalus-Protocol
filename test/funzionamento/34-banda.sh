#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# SCELTA DEI NODI PESATA PER BANDA (velocità): il gestore indica la banda di ogni nodo
# (nyctalus-rete banda IP:PORTA MBIT), la banda entra nell'elenco FIRMATO e il motore sceglie più
# spesso i nodi più capaci. In simulazione (velocita.py, nodi da 5 a 50 Mbit) da 3,9 a 9,9 Mbit/s
# contro i 6,2 di Tor. Qui: gestore, firma, lettura del motore, campi sconosciuti ignorati.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · scelta dei nodi pesata per banda"
assicura_binario
G="$ROOT/pc/gestore-rete/nyctalus-rete"; H="$TMP/gestore"; mkdir -p "$H"
R() { HOME="$H" NYCTALUS_BIN="$BIN" python3 "$G" "$@" 2>&1; }
R crea --nome prova > /dev/null
riga() { python3 -c "import os;print('203.0.113.%s:4601,%s,%s' % ($1, os.urandom(32).hex(), os.urandom(32).hex()))"; }
R aggiungi "$(riga 1)" "$(riga 2)" "$(riga 3)" --famiglia mario > /dev/null
R banda 203.0.113.1:4601 100 > "$TMP/b.log"
R banda 203.0.113.2:4601 10 > /dev/null
R famiglia 203.0.113.1:4601 luca > /dev/null
grep -q "^203.0.113.1:4601,[0-9a-f]*,[0-9a-f]*,luca,100$" "$H/.nyctalus-rete/nodi.txt" \
  && pass "gestore: «banda IP:PORTA 100» salvata, e cambiare la famiglia non la perde" || fail "nodi.txt: $(cat "$H/.nyctalus-rete/nodi.txt"; cat "$TMP/b.log")"
R banda 203.0.113.9:4601 5 | grep -q "non trovato" && pass "banda di un nodo inesistente rifiutata" || fail "nodo inesistente accettato"
R banda 203.0.113.1:4601 tanta | grep -q "uso:" && pass "banda non numerica rifiutata" || fail "banda non numerica accettata"
R firma > /dev/null
D="$H/.nyctalus-rete/directory.txt"
grep -q "203.0.113.1:4601 .* famiglia=luca banda=100$" "$D" && grep -q "203.0.113.2:4601 .* famiglia=mario banda=10$" "$D" \
  && pass "la banda entra nell'elenco firmato (banda=100, banda=10)" || fail "elenco: $(cat "$D")"
PUB=$(python3 -c "import json;print(json.load(open('$H/.nyctalus-rete/autorita.json'))['pubblica'])")
"$BIN" status --directory "$D" --authority "$PUB" --config /dev/null 2>&1 | grep -q "directory valid: 3 nodes" \
  && pass "il motore legge l'elenco con le bande (3 nodi)" || fail "motore: $("$BIN" status --directory "$D" --authority "$PUB" --config /dev/null 2>&1 | tail -2)"
sed 's/ banda=100$/ banda=900/' "$D" > "$TMP/falso.txt"
"$BIN" status --directory "$TMP/falso.txt" --authority "$PUB" --config /dev/null 2>&1 | grep -q "INVALID" \
  && pass "la banda è coperta dalla firma: cambiarla rende l'elenco non valido" || fail "banda alterata accettata"
R banda 203.0.113.1:4601 - > /dev/null
grep -q "^203.0.113.1:4601,[0-9a-f]*,[0-9a-f]*,luca$" "$H/.nyctalus-rete/nodi.txt" && pass "«banda IP:PORTA -» la toglie (la famiglia resta)" || fail "togliere la banda"
[ "$KO" -eq 0 ]
