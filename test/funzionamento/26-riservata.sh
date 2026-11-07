#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Modalità riservata (PIANO-ANONIMATO, Tappa 2-bis): con la modalità accesa Anna non scrive mai
# in diretta. I suoi messaggi partono dalla casella a ritmo casuale, insieme a buste finte
# indirizzate a lei stessa, tutte della stessa misura; niente ricevute; la rubrica le consegna la
# posta solo quando lei la chiede. Ritmo e ritiro accorciati SOLO in prova. Tutto in 127.0.0.1.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · modalità riservata (casella, ritmo casuale, buste finte e tutte uguali)"
assicura_binario
H="$TMP/gestore"; mkdir -p "$H"
NYCTALUS_RUBRICA_PORTE=23630,23631,23632,23633,23634 HOME="$H" NYCTALUS_BIN="$BIN" \
  python3 "$ROOT/pc/gestore-rete/nyctalus-rete" casa --ip 127.0.0.1 --porta 23620 --porta-base 23621 --non-importare \
  < /dev/null > "$TMP/casa.log" 2>&1 & PIDS+=($!)
for _ in $(seq 1 240); do grep -q "Rete di casa attiva" "$TMP/casa.log" && break; sleep 0.5; done
grep -q "Rete di casa attiva" "$TMP/casa.log" || { fail "rete di casa: $(tail -3 "$TMP/casa.log")"; exit 1; }
COD=$(grep -oE 'NYCT1:[A-Za-z0-9_-]+' "$TMP/casa.log" | head -1)
leggi() { python3 -c "import base64,json,sys;c=sys.argv[1][6:];print(json.loads(base64.urlsafe_b64decode(c+'='*(-len(c)%4))).get(sys.argv[2],''))" "$COD" "$1"; }
RUB=$(leggi rubrica); CAS=$(leggi casella)
PUB=$(python3 -c "import json;print(json.load(open('$H/.nyctalus-rete/casa/autorita.json'))['pubblica'])")
for _ in $(seq 1 20); do curl -sf http://127.0.0.1:23620/directory.txt -o "$TMP/dir.txt" && [ -s "$TMP/dir.txt" ] && break; sleep 1; done
[ -s "$TMP/dir.txt" ] || { fail "elenco dei nodi non scaricato"; exit 1; }
D="$H/.nyctalus-rete/casa/rubrica/casella"

utente() { # nome base segreto → rete + chat (con rubrica e casella) + servizio; indirizzo in $ADDR
  local n="$1" b="$2" seg="$3"; mkdir -p "$TMP/$n"
  "$BIN" start --directory "$TMP/dir.txt" --authority "$PUB" --socks 127.0.0.1:$b --control 127.0.0.1:$((b+3)) > "$TMP/$n.rete.log" 2>&1 & PIDS+=($!)
  NYCTALUS_RITMO_RISERVATO=2 NYCTALUS_RITIRO_RISERVATO=${RITIRO:-3} NYCTALUS_CHAT_TOKEN="$seg" "$BIN" chat --data "$TMP/$n" \
    --backend 127.0.0.1:$((b+1)) --control 127.0.0.1:$((b+2)) --socks 127.0.0.1:$b --rubrica "$RUB" --casella "$CAS" > "$TMP/$n.chat.log" 2>&1 & PIDS+=($!)
  ADDR=$(attendi_regex "$TMP/$n.chat.log" '[a-z2-7]{56}\.nyct' 15)
  "$BIN" site --backend 127.0.0.1:$((b+1)) --identity "$TMP/$n/identita" --directory "$TMP/dir.txt" --authority "$PUB" \
    --hops 3 > "$TMP/$n.site.log" 2>&1 & PIDS+=($!)
}
SA=annasegreto0123456789abcdef01; SB=brunosegreto0123456789abcdef0
RITIRO=20 utente anna 23640 "$SA"; ANNA=$ADDR
utente bruno 23650 "$SB"; BRUNO=$ADDR
[ -n "$ANNA" ] && [ -n "$BRUNO" ] || { fail "identità non partite"; exit 1; }
sleep 6
for _ in $(seq 1 30); do curl -sf --max-time 20 --socks5-hostname 127.0.0.1:23640 "http://$CAS/" > /dev/null && break; sleep 3; done
controllo 23642 "$SA" 5 "ADD $BRUNO $(printf Bruno | xxd -p)" > /dev/null
controllo 23652 "$SB" 5 "ADD $ANNA $(printf Anna | xxd -p)" > /dev/null

R=$(NYCT_FINO="^RISERVATA" controllo 23642 "$SA" 10 "RISERVATA 1")
echo "$R" | grep -q "^RISERVATA 1 1 0 " && [ "$(cat "$TMP/anna/riservata")" = 1 ] \
  && pass "Anna accende la modalità riservata (RISERVATA 1), ricordata su disco" || fail "accensione: $R"

# misure delle buste nella casella, fotografate di continuo mentre si lavora
( for _ in $(seq 1 400); do find "$D" -name '*.busta' -printf '%s %h\n' 2>/dev/null; sleep 0.15; done ) > "$TMP/misure.txt" & FOTO=$!

TESTO="Riservato: ci vediamo alle otto, àèìòù 🦇"
HX=$(printf %s "$TESTO" | xxd -p | tr -d '\n')
R=$(NYCT_FINO="^OK" controllo 23642 "$SA" 20 SUB "SEND $BRUNO $HX")
echo "$R" | grep -q "^OUT $BRUNO " && echo "$R" | grep -q "^SP $BRUNO [0-9]* 1" \
  && pass "il messaggio entra in coda (✓ spedito), non parte in diretta" || fail "invio: $(echo "$R" | grep -E '^(OUT|SP|ERR|OK)')"
R=$(NYCT_FINO="^MSG $ANNA " controllo 23652 "$SB" 120 SUB)
echo "$R" | grep -q "^MSG $ANNA [0-9]* $HX$" \
  && pass "Bruno lo riceve dalla casella, identico (riempitivo tolto), autenticato come di Anna" || fail "ricezione: $(echo "$R" | tail -2)"
grep -q "^in [0-9]* $HX$" "$TMP/bruno/cronologia/$ANNA.log" && pass "nella cronologia di Bruno c'è il testo giusto" || fail "cronologia di Bruno"
TS=$(grep -oE "^out [0-9]+" "$TMP/anna/cronologia/$BRUNO.log" | head -1 | cut -d' ' -f2)
for _ in $(seq 1 20); do NYCT_FINO="^ENDH" controllo 23642 "$SA" 10 "HIST $BRUNO" | grep -q "^CAS $BRUNO $TS" && break; sleep 1; done
NYCT_FINO="^ENDH" controllo 23642 "$SA" 10 "HIST $BRUNO" | grep -q "^CAS $BRUNO $TS" \
  && pass "per Anna il messaggio è 📮 nella casella" || fail "spunta casella mancante"
[ ! -s "$TMP/anna/coda-riservata" ] && pass "la coda si svuota (salvata su disco finché serve)" || fail "coda: $(cat "$TMP/anna/coda-riservata")"

# Bruno (normale) scrive ad Anna: Anna, riservata, non manda ricevute
R=$(NYCT_FINO="^SP $ANNA [0-9]+ 1" controllo 23652 "$SB" 60 SUB "SEND $ANNA $(printf 'ciao Anna' | xxd -p)")
TSB=$(echo "$R" | grep -oE "^OUT $ANNA [0-9]+" | head -1 | awk '{print $3}')
sleep 15
! NYCT_FINO="^ENDH" controllo 23652 "$SB" 10 "HIST $ANNA" | grep -qE "^SP $ANNA $TSB (2|3)" \
  && pass "Anna riservata riceve ma NON manda ricevute (per Bruno niente ✓✓)" || fail "Anna ha mandato una ricevuta"

# giri a vuoto: buste finte verso Anna stessa, spedite e tornate
sleep 8
R=$(NYCT_FINO="^RISERVATA" controllo 23642 "$SA" 10 "RISERVATA")
SPED=$(echo "$R" | awk '/^RISERVATA/{print $5}'); TORN=$(echo "$R" | awk '/^RISERVATA/{print $6}')
[ "${SPED:-0}" -ge 3 ] && pass "senza messaggi da mandare, parte comunque una busta finta a ritmo casuale ($SPED spedite)" || fail "buste finte: $R"
[ "${TORN:-0}" -ge 1 ] && pass "le buste finte tornano ad Anna (controllo che la casella non perda posta: $TORN tornate)" || fail "nessuna busta finta tornata: $R"

# la rubrica non consegna subito ad Anna riservata: una busta resta finché non la chiede lei
python3 -c "import os,sys;sys.stdout.buffer.write(os.urandom(200))" > "$TMP/finta.bin"
curl -sf --max-time 30 --socks5-hostname 127.0.0.1:23650 -X POST --data-binary @"$TMP/finta.bin" "http://$CAS/deposita?a=$ANNA" > /dev/null
sleep 4
N=$(find "$D/$ANNA" -name '*.busta' -size 200c 2>/dev/null | wc -l)
[ "$N" -ge 1 ] && pass "posta per Anna in linea ma riservata: la rubrica NON gliela porta subito (aspetta il ritiro a intervallo fisso)" || fail "consegnata subito (busta da 200 byte sparita)"
for _ in $(seq 1 30); do [ -z "$(find "$D/$ANNA" -name '*.busta' -size 200c 2>/dev/null)" ] && break; sleep 1; done
[ -z "$(find "$D/$ANNA" -name '*.busta' -size 200c 2>/dev/null)" ] && pass "al ritiro successivo la prende (e scarta la busta illeggibile)" || fail "busta mai ritirata"

kill $FOTO 2>/dev/null; wait $FOTO 2>/dev/null
MIS=$(grep -E "/($ANNA|$BRUNO)$" "$TMP/misure.txt" | awk '$1!=200{print $1}' | sort -u)
NM=$(grep -E "/($ANNA|$BRUNO)$" "$TMP/misure.txt" | awk '$1!=200' | wc -l)
[ "$(echo "$MIS" | grep -c .)" = 1 ] && [ "$NM" -ge 3 ] \
  && pass "nella casella buste vere e finte sono tutte della stessa misura ($MIS byte, $NM osservazioni)" || fail "misure diverse: $(echo $MIS)"

R=$(NYCT_FINO="^(OK|ERR)" controllo 23642 "$SA" 10 "SEND $BRUNO $(head -c 3900 /dev/zero | tr '\0' a | xxd -p | tr -d '\n')")
echo "$R" | grep -q "^ERR .*troppo lungo" && pass "messaggio oltre i 3800 caratteri: rifiutato con un motivo chiaro" || fail "lungo: $R"
R=$(NYCT_FINO="^RISERVATA" controllo 23642 "$SA" 10 "RISERVATA 0")
echo "$R" | grep -q "^RISERVATA 0 " && [ "$(cat "$TMP/anna/riservata")" = 0 ] && pass "RISERVATA 0: si torna alla chat normale" || fail "spegnimento: $R"

# dal terminale: nyctalus-cli impostazioni riservata si|no
CLI() { HOME="$TMP" NYCTALUS_CHAT_PORT=23642 NYCTALUS_CHAT_TOKEN="$SA" python3 "$ROOT/pc/cli/nyctalus-cli" "$@" 2>&1; }
CLI impostazioni riservata si | grep -q "modalità riservata: *sì" && CLI impostazioni | grep -q "modalità riservata: *sì" \
  && CLI impostazioni riservata no | grep -q "modalità riservata: *no" \
  && pass "nyctalus-cli: «impostazioni riservata si|no» la accende e la spegne, «impostazioni» la mostra" || fail "CLI: $(CLI impostazioni)"

# rete senza casella: la modalità non si può accendere
mkdir -p "$TMP/solo"
NYCTALUS_CHAT_TOKEN=solosegreto0123456789abcdef "$BIN" chat --data "$TMP/solo" --backend 127.0.0.1:23661 --control 127.0.0.1:23662 \
  --socks 127.0.0.1:23640 > "$TMP/solo.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/solo.log" '[a-z2-7]{56}\.nyct' 15 > /dev/null
R=$(NYCT_FINO="^(ERR|RISERVATA)" controllo 23662 solosegreto0123456789abcdef 10 "RISERVATA 1")
echo "$R" | grep -q "^ERR .*casella" && pass "rete senza casella: la modalità riservata non si accende e lo dice" || fail "senza casella: $R"
HOME="$TMP" NYCTALUS_CHAT_PORT=23662 NYCTALUS_CHAT_TOKEN=solosegreto0123456789abcdef python3 "$ROOT/pc/cli/nyctalus-cli" impostazioni 2>&1 | grep -q "non disponibile" \
  && pass "nyctalus-cli: in una rete senza casella dice «non disponibile»" || fail "CLI senza casella"
[ "$KO" -eq 0 ]
