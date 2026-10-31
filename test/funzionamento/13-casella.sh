#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# La casella cifrata: Anna scrive a Bruno che è SPENTO. Il messaggio finisce, sigillato, nella
# casella della rete di casa (che non lo può leggere e non sa chi l'ha scritto); appena Bruno
# si accende lo riceve, autenticato come scritto da Anna, e la casella lo cancella. Spunte
# (spedito, casella, consegnato, letto) e «modalità aereo» (sessione morta: il messaggio senza
# conferma va nella casella). Una busta falsa viene scartata; una troppo grande rifiutata.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · casella cifrata (messaggi a chi non è in linea)"
assicura_binario
H="$TMP/gestore"; mkdir -p "$H"
NYCTALUS_RUBRICA_PORTE=20630,20631,20632,20633,20634 HOME="$H" NYCTALUS_BIN="$BIN" \
  python3 "$ROOT/pc/gestore-rete/nyctalus-rete" casa --ip 127.0.0.1 --porta 20620 --porta-base 20621 --non-importare \
  < /dev/null > "$TMP/casa.log" 2>&1 & PIDS+=($!)
for _ in $(seq 1 240); do grep -q "Rete di casa attiva" "$TMP/casa.log" && break; sleep 0.5; done
grep -q "Rete di casa attiva" "$TMP/casa.log" || { fail "rete di casa: $(tail -3 "$TMP/casa.log")"; exit 1; }
COD=$(grep -oE 'NYCT1:[A-Za-z0-9_-]+' "$TMP/casa.log" | head -1)
leggi() { python3 -c "import base64,json,sys;c=sys.argv[1][6:];print(json.loads(base64.urlsafe_b64decode(c+'='*(-len(c)%4))).get(sys.argv[2],''))" "$COD" "$1"; }
RUB=$(leggi rubrica); CAS=$(leggi casella)
[[ "$CAS" =~ ^[a-z2-7]{56}\.nyct$ ]] && [ "$CAS" != "$RUB" ] && pass "il codice di rete contiene la casella (indirizzo .nyct a sé, diverso dalla rubrica)" || { fail "casella non nel codice: $COD"; exit 1; }
PUB=$(python3 -c "import json;print(json.load(open('$H/.nyctalus-rete/casa/autorita.json'))['pubblica'])")
for _ in $(seq 1 20); do curl -sf http://127.0.0.1:20620/directory.txt -o "$TMP/dir.txt" && [ -s "$TMP/dir.txt" ] && break; sleep 1; done
[ -s "$TMP/dir.txt" ] || { fail "elenco dei nodi non scaricato dalla rete di casa: $(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:20620/directory.txt) · $(tail -3 "$TMP/casa.log")"; exit 1; }
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
utente bruno 20650 "$SB"; BRUNO=$ADDR; kill $RETE_PID $CHAT_PID $SITE_PID 2>/dev/null; sleep 1
utente anna 20640 "$SA"; ANNA=$ADDR
sleep 6
TESTO="Ciao Bruno, te lo lascio nella casella"
R=$(NYCT_FINO="^(CAS|ERR) " controllo 20642 "$SA" 200 SUB "ADD $BRUNO $(printf Bruno | xxd -p)" \
    "SEND $BRUNO $(printf %s "$TESTO" | xxd -p | tr -d '\n')")
echo "$R" | grep -q "^CAS $BRUNO " && echo "$R" | grep -q "^OUT $BRUNO " \
  && pass "Bruno spento: il messaggio di Anna va nella casella (OUT + CAS)" || fail "deposito: $(echo "$R" | grep -E '^(ERR|CAS|OUT)' | head -3) — rete di Anna: $(tail -4 "$TMP/anna.rete.log" | tr '\n' ' ')"
N=$(ls "$D/$BRUNO"/*.busta 2>/dev/null | wc -l)
[ "$N" = 1 ] && pass "nella casella c'è una busta per Bruno" || fail "buste per Bruno: $N"
F=$(ls "$D/$BRUNO"/*.busta 2>/dev/null | head -1)
! grep -q "casella" "$F" && ! grep -q "Bruno" "$F" && ! grep -q "$ANNA" "$F" && ! grep -rq "$ANNA" "$D" \
  && pass "la busta non contiene né il testo né l'indirizzo di Anna (la casella non sa chi scrive)" || fail "busta leggibile"
[ "$(stat -c %a "$D/$BRUNO")" = 700 ] && [ "$(stat -c %a "$F")" = 600 ] && pass "casella su disco privata (700/600)" || fail "permessi della casella"
! grep -rqi "$ANNA\|deposita" "$H/.nyctalus-rete/casa/rubrica/rubrica.log" 2>/dev/null && pass "nessun registro di chi deposita" || fail "il deposito lascia tracce nel registro"

# Bruno si accende: deve ricevere il messaggio da Anna e la casella si svuota
utente bruno 20650 "$SB"
R=$(NYCT_FINO="^MSG $ANNA " controllo 20652 "$SB" 150 SUB)
HX=$(printf %s "$TESTO" | xxd -p | tr -d '\n')
echo "$R" | grep -q "^MSG $ANNA [0-9]* $HX$" && pass "Bruno acceso: riceve il messaggio, autenticato come scritto da Anna" || fail "ritiro: $(echo "$R" | tail -3)"
echo "$R" | grep -q "^NEW $ANNA" && pass "Anna non era fra i contatti di Bruno: gli viene proposta (NEW)" || fail "NEW mancante"
grep -q "^in [0-9]* $HX$" "$TMP/bruno/cronologia/$ANNA.log" 2>/dev/null && pass "il messaggio è nella cronologia di Bruno" || fail "cronologia di Bruno"
for _ in $(seq 1 20); do [ -z "$(ls "$D/$BRUNO"/*.busta 2>/dev/null)" ] && break; sleep 0.5; done
[ -z "$(ls "$D/$BRUNO"/*.busta 2>/dev/null)" ] && pass "consegnata: la casella l'ha cancellata" || fail "busta ancora nella casella"

# spunte: la conferma «ricevuto» di Bruno torna ad Anna (stato salvato, si rilegge con HIST)
storia_anna() { NYCT_FINO="^ENDH" controllo 20642 "$SA" 20 "HIST $BRUNO"; }
spunta() { local i; for i in $(seq 1 "$2"); do storia_anna | grep -qE "$1" && return 0; sleep 2; done; return 1; }
TS1=$(grep -oE "^out [0-9]+" "$TMP/anna/cronologia/$BRUNO.log" | head -1 | cut -d' ' -f2)
spunta "^CAS $BRUNO $TS1" 1 && spunta "^SP $BRUNO $TS1 2" 45 \
  && pass "spunte: 📮✓ nella casella, poi ✓✓ consegnato quando Bruno la ritira" || fail "spunte del messaggio in casella: $(storia_anna | grep -E '^(SP|CAS)')"
R=$(NYCT_FINO="^SP $BRUNO [0-9]+ 2" controllo 20642 "$SA" 90 SUB "SEND $BRUNO $(printf 'Secondo, sei in linea' | xxd -p | tr -d '\n')")
TS2=$(echo "$R" | grep -oE "^OUT $BRUNO [0-9]+" | head -1 | awk '{print $3}')
echo "$R" | grep -q "^SP $BRUNO $TS2 1" && echo "$R" | grep -q "^SP $BRUNO $TS2 2" \
  && pass "Bruno in linea: ✓ spedito, poi ✓✓ consegnato" || fail "spunte dirette: $(echo "$R" | grep -E '^(SP|CAS|ERR)')"
R=$(NYCT_FINO="^SP $BRUNO [0-9]+ 3" controllo 20642 "$SA" 60 SUB) & ATT=$!
sleep 1; controllo 20652 "$SB" 3 "LETTO $ANNA $TS2" > /dev/null; wait $ATT
spunta "^SP $BRUNO $TS2 3" 15 && pass "Bruno apre la conversazione (LETTO): ad Anna le spunte diventano blu" || fail "spunta di lettura"

# «modalità aereo»: la rete di Bruno si blocca di colpo, la sessione sembra ancora aperta
kill -STOP $RETE_PID $SITE_PID $CHAT_PID
R=$(NYCT_FINO="^CAS $BRUNO " controllo 20642 "$SA" 150 SUB "SEND $BRUNO $(printf 'Terzo, in modalita aereo' | xxd -p | tr -d '\n')")
echo "$R" | grep -q "^CAS $BRUNO " && pass "modalità aereo: il messaggio senza «ricevuto» finisce comunque nella casella" \
  || fail "messaggio perso nella sessione morta: $(echo "$R" | grep -E '^(OUT|SP|CAS|ERR)')"
# battito: il contatto sparito di colpo non resta «in linea» (Anna non vede più la sessione)
for _ in $(seq 1 30); do
  controllo 20642 "$SA" 3 SUB 2>/dev/null | grep -q "^ST $BRUNO 1" || break; sleep 5
done
controllo 20642 "$SA" 3 SUB 2>/dev/null | grep -q "^ST $BRUNO 1" \
  && fail "Bruno sparito di colpo risulta ancora in linea (manca il battito)" || pass "battito: chi sparisce di colpo non resta «in linea»"
kill -CONT $RETE_PID $SITE_PID $CHAT_PID
H3=$(printf 'Terzo, in modalita aereo' | xxd -p | tr -d '\n')
for _ in $(seq 1 90); do grep -q " $H3$" "$TMP/bruno/cronologia/$ANNA.log" 2>/dev/null && break; sleep 2; done
[ "$(grep -c " $H3$" "$TMP/bruno/cronologia/$ANNA.log" 2>/dev/null)" = 1 ] \
  && pass "tolta la modalità aereo, Bruno lo riceve (una volta sola)" || fail "terzo messaggio: $(grep -c " $H3$" "$TMP/bruno/cronologia/$ANNA.log" 2>/dev/null) copie"

# una busta falsa (non sigillata da nessuno) e una troppo grande, depositate dai circuiti
for _ in $(seq 1 30); do [ -z "$(ls "$D/$BRUNO"/*.busta 2>/dev/null)" ] && break; sleep 1; done
PRIMA=$(wc -l < "$TMP/bruno/cronologia/$ANNA.log")
head -c 300 /dev/urandom > "$TMP/falsa.bin"
C=$(curl -s -o /dev/null -w '%{http_code}' --max-time 90 --socks5-hostname 127.0.0.1:20640 --data-binary @"$TMP/falsa.bin" "http://$CAS/deposita?a=$BRUNO")
[ "$C" = 200 ] && pass "deposito anonimo attraverso la rete (curl via SOCKS, senza identità)" || fail "deposito via SOCKS: $C"
head -c 70000 /dev/urandom > "$TMP/grande.bin"
C=$(curl -s -o /dev/null -w '%{http_code}' --max-time 90 --socks5-hostname 127.0.0.1:20640 --data-binary @"$TMP/grande.bin" "http://$CAS/deposita?a=$BRUNO")
[ "$C" = 413 ] && pass "busta troppo grande rifiutata" || fail "busta grande: $C"
for _ in $(seq 1 150); do [ -z "$(ls "$D/$BRUNO"/*.busta 2>/dev/null)" ] && break; sleep 1; done
[ -z "$(ls "$D/$BRUNO"/*.busta 2>/dev/null)" ] && [ "$(wc -l < "$TMP/bruno/cronologia/$ANNA.log")" = "$PRIMA" ] \
  && ! ls "$TMP/bruno/cronologia/" | grep -qv "$ANNA" \
  && pass "busta falsa: scartata da Bruno (nessun messaggio inventato) e tolta dalla casella" || fail "busta falsa: $(ls "$D/$BRUNO" "$TMP/bruno/cronologia/" 2>&1 | tr '\n' ' ') $(for f in "$TMP"/bruno/cronologia/*; do echo "== $f"; cut -c1-200 "$f"; done)"
[ "$KO" -eq 0 ]
