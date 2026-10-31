#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# La casella cifrata: Anna scrive a Bruno che è SPENTO. Il messaggio finisce, sigillato, nella
# casella della rete di casa (che non lo può leggere e non sa chi l'ha scritto); appena Bruno
# si accende lo riceve, autenticato come scritto da Anna, e la casella lo cancella. Una busta
# falsa viene scartata; una busta troppo grande rifiutata.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · casella cifrata (messaggi a chi non è in linea)"
assicura_binario
H="$TMP/gestore"; mkdir -p "$H"
NYCTALUS_RUBRICA_PORTE=19830,19831,19832,19833,19834 HOME="$H" NYCTALUS_BIN="$BIN" \
  python3 "$ROOT/pc/gestore-rete/nyctalus-rete" casa --ip 127.0.0.1 --porta 19820 --porta-base 19821 --non-importare \
  < /dev/null > "$TMP/casa.log" 2>&1 & PIDS+=($!)
for _ in $(seq 1 240); do grep -q "Rete di casa attiva" "$TMP/casa.log" && break; sleep 0.5; done
grep -q "Rete di casa attiva" "$TMP/casa.log" || { fail "rete di casa: $(tail -3 "$TMP/casa.log")"; exit 1; }
COD=$(grep -oE 'NYCT1:[A-Za-z0-9_-]+' "$TMP/casa.log" | head -1)
leggi() { python3 -c "import base64,json,sys;c=sys.argv[1][6:];print(json.loads(base64.urlsafe_b64decode(c+'='*(-len(c)%4))).get(sys.argv[2],''))" "$COD" "$1"; }
RUB=$(leggi rubrica); CAS=$(leggi casella)
[[ "$CAS" =~ ^[a-z2-7]{56}\.nyct$ ]] && [ "$CAS" != "$RUB" ] && pass "il codice di rete contiene la casella (indirizzo .nyct a sé, diverso dalla rubrica)" || { fail "casella non nel codice: $COD"; exit 1; }
PUB=$(python3 -c "import json;print(json.load(open('$H/.nyctalus-rete/casa/autorita.json'))['pubblica'])")
curl -s http://127.0.0.1:19820/directory.txt -o "$TMP/dir.txt"
D="$H/.nyctalus-rete/casa/rubrica/casella"

utente() { # nome base segreto → rete + chat (con rubrica e casella) + servizio; indirizzo in $ADDR
  local n="$1" b="$2" seg="$3"; mkdir -p "$TMP/$n"
  "$BIN" start --directory "$TMP/dir.txt" --authority "$PUB" --socks 127.0.0.1:$b --control 127.0.0.1:$((b+3)) > "$TMP/$n.rete.log" 2>&1 & PIDS+=($!); RETE_PID=$!
  NYCTALUS_CHAT_TOKEN="$seg" "$BIN" chat --data "$TMP/$n" --backend 127.0.0.1:$((b+1)) --control 127.0.0.1:$((b+2)) \
    --socks 127.0.0.1:$b --rubrica "$RUB" --casella "$CAS" > "$TMP/$n.chat.log" 2>&1 & PIDS+=($!); CHAT_PID=$!
  ADDR=$(attendi_regex "$TMP/$n.chat.log" '[a-z2-7]{56}\.nyct' 15)
  "$BIN" site --backend 127.0.0.1:$((b+1)) --identity "$TMP/$n/identita" --directory "$TMP/dir.txt" --authority "$PUB" \
    --hops 3 > "$TMP/$n.site.log" 2>&1 & PIDS+=($!); SITE_PID=$!
}
SA=annasegreto0123456789abcdef01; SB=brunosegreto0123456789abcdef0
# Bruno: un'identità che esiste ma è SPENTA (si accende, si legge l'indirizzo, si spegne)
utente bruno 19850 "$SB"; BRUNO=$ADDR; kill $RETE_PID $CHAT_PID $SITE_PID 2>/dev/null; sleep 1
utente anna 19840 "$SA"; ANNA=$ADDR
sleep 6
TESTO="Ciao Bruno, te lo lascio nella casella"
R=$(NYCT_FINO="^(CAS|ERR) " controllo 19842 "$SA" 200 SUB "ADD $BRUNO $(printf Bruno | xxd -p)" \
    "SEND $BRUNO $(printf %s "$TESTO" | xxd -p | tr -d '\n')")
echo "$R" | grep -q "^CAS $BRUNO " && echo "$R" | grep -q "^OUT $BRUNO " \
  && pass "Bruno spento: il messaggio di Anna va nella casella (OUT + CAS)" || fail "deposito: $(echo "$R" | grep -E '^(ERR|CAS|OUT)' | head -3)"
N=$(ls "$D/$BRUNO"/*.busta 2>/dev/null | wc -l)
[ "$N" = 1 ] && pass "nella casella c'è una busta per Bruno" || fail "buste per Bruno: $N"
F=$(ls "$D/$BRUNO"/*.busta 2>/dev/null | head -1)
! grep -q "casella" "$F" && ! grep -q "Bruno" "$F" && ! grep -q "$ANNA" "$F" && ! grep -rq "$ANNA" "$D" \
  && pass "la busta non contiene né il testo né l'indirizzo di Anna (la casella non sa chi scrive)" || fail "busta leggibile"
[ "$(stat -c %a "$D/$BRUNO")" = 700 ] && [ "$(stat -c %a "$F")" = 600 ] && pass "casella su disco privata (700/600)" || fail "permessi della casella"
! grep -rqi "$ANNA\|deposita" "$H/.nyctalus-rete/casa/rubrica/rubrica.log" 2>/dev/null && pass "nessun registro di chi deposita" || fail "il deposito lascia tracce nel registro"

# Bruno si accende: deve ricevere il messaggio da Anna e la casella si svuota
utente bruno 19850 "$SB"
R=$(NYCT_FINO="^MSG $ANNA " controllo 19852 "$SB" 150 SUB)
HX=$(printf %s "$TESTO" | xxd -p | tr -d '\n')
echo "$R" | grep -q "^MSG $ANNA [0-9]* $HX$" && pass "Bruno acceso: riceve il messaggio, autenticato come scritto da Anna" || fail "ritiro: $(echo "$R" | tail -3)"
echo "$R" | grep -q "^NEW $ANNA" && pass "Anna non era fra i contatti di Bruno: gli viene proposta (NEW)" || fail "NEW mancante"
grep -q "^in [0-9]* $HX$" "$TMP/bruno/cronologia/$ANNA.log" 2>/dev/null && pass "il messaggio è nella cronologia di Bruno" || fail "cronologia di Bruno"
for _ in $(seq 1 20); do [ -z "$(ls "$D/$BRUNO"/*.busta 2>/dev/null)" ] && break; sleep 0.5; done
[ -z "$(ls "$D/$BRUNO"/*.busta 2>/dev/null)" ] && pass "consegnata: la casella l'ha cancellata" || fail "busta ancora nella casella"

# una busta falsa (non sigillata da nessuno) e una troppo grande, depositate dai circuiti
head -c 300 /dev/urandom > "$TMP/falsa.bin"
C=$(curl -s -o /dev/null -w '%{http_code}' --max-time 90 --socks5-hostname 127.0.0.1:19840 --data-binary @"$TMP/falsa.bin" "http://$CAS/deposita?a=$BRUNO")
[ "$C" = 200 ] && pass "deposito anonimo attraverso la rete (curl via SOCKS, senza identità)" || fail "deposito via SOCKS: $C"
head -c 70000 /dev/urandom > "$TMP/grande.bin"
C=$(curl -s -o /dev/null -w '%{http_code}' --max-time 90 --socks5-hostname 127.0.0.1:19840 --data-binary @"$TMP/grande.bin" "http://$CAS/deposita?a=$BRUNO")
[ "$C" = 413 ] && pass "busta troppo grande rifiutata" || fail "busta grande: $C"
for _ in $(seq 1 60); do [ -z "$(ls "$D/$BRUNO"/*.busta 2>/dev/null)" ] && break; sleep 1; done
[ -z "$(ls "$D/$BRUNO"/*.busta 2>/dev/null)" ] && [ "$(wc -l < "$TMP/bruno/cronologia/$ANNA.log")" = 1 ] \
  && ! ls "$TMP/bruno/cronologia/" | grep -qv "$ANNA" \
  && pass "busta falsa: scartata da Bruno (nessun messaggio inventato) e tolta dalla casella" || fail "busta falsa: $(ls "$D/$BRUNO" "$TMP/bruno/cronologia/" 2>&1 | tr '\n' ' ')"
[ "$KO" -eq 0 ]
