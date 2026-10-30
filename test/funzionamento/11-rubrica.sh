#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# La rubrica della rete: la rete di casa la accende da sola e ne mette l'indirizzo nel
# codice di rete; due utenti (come PC e telefono) si iscrivono con un nome attraverso la
# chat cifrata, vedono l'elenco con chi è in linea, uno si cancella. La rubrica non
# conserva la cronologia delle richieste.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · rubrica della rete"
assicura_binario
H="$TMP/gestore"; mkdir -p "$H"
NYCTALUS_RUBRICA_PORTE=19730,19731,19732,19733 HOME="$H" NYCTALUS_BIN="$BIN" \
  python3 "$ROOT/pc/gestore-rete/nyctalus-rete" casa --ip 127.0.0.1 --porta 19720 --porta-base 19721 --non-importare \
  < /dev/null > "$TMP/casa.log" 2>&1 & PIDS+=($!)
for _ in $(seq 1 120); do grep -q "Rete di casa attiva" "$TMP/casa.log" && break; sleep 0.5; done
grep -q "Rete di casa attiva" "$TMP/casa.log" && pass "rete di casa accesa (con la rubrica)" || { fail "rete di casa: $(tail -3 "$TMP/casa.log")"; exit 1; }
COD=$(grep -oE 'NYCT1:[A-Za-z0-9_-]+' "$TMP/casa.log" | head -1)
RUB=$(python3 -c "import base64,json,sys;c=sys.argv[1][6:];print(json.loads(base64.urlsafe_b64decode(c+'='*(-len(c)%4))).get('rubrica',''))" "$COD")
[[ "$RUB" =~ ^[a-z2-7]{56}\.nyct$ ]] && pass "il codice di rete contiene l'indirizzo della rubrica" || { fail "rubrica non nel codice"; exit 1; }
PUB=$(python3 -c "import json;print(json.load(open('$H/.nyctalus-rete/casa/autorita.json'))['pubblica'])")
curl -s http://127.0.0.1:19720/directory.txt -o "$TMP/dir.txt"

utente() { # nome base_porte segreto → avvia rete+chat+servizio, mette l'indirizzo in $ADDR
  local n="$1" b="$2" seg="$3"; mkdir -p "$TMP/$n"
  "$BIN" start --directory "$TMP/dir.txt" --authority "$PUB" --socks 127.0.0.1:$b --control 127.0.0.1:$((b+3)) > "$TMP/$n.rete.log" 2>&1 & PIDS+=($!)
  NYCTALUS_CHAT_TOKEN="$seg" "$BIN" chat --data "$TMP/$n" --backend 127.0.0.1:$((b+1)) --control 127.0.0.1:$((b+2)) \
    --socks 127.0.0.1:$b > "$TMP/$n.chat.log" 2>&1 & PIDS+=($!)
  ADDR=$(attendi_regex "$TMP/$n.chat.log" '[a-z2-7]{56}\.nyct' 15)
  "$BIN" site --backend 127.0.0.1:$((b+1)) --identity "$TMP/$n/identita" --directory "$TMP/dir.txt" --authority "$PUB" \
    --hops 3 > "$TMP/$n.site.log" 2>&1 & PIDS+=($!)
}
SA=annasegreto0123456789abcdef01; utente anna 19740 "$SA"; ANNA=$ADDR
SB=brunosegreto0123456789abcdef0; utente bruno 19750 "$SB"; BRUNO=$ADDR
sleep 6
# richiesta alla rubrica: SEND col JSON, risposta = MSG dalla rubrica
chiedi() { # porta segreto json → la risposta (JSON) della rubrica
  NYCT_FINO="^MSG $RUB " controllo "$1" "$2" 60 SUB "SEND $RUB $(printf %s "$3" | xxd -p | tr -d '\n')" \
    | grep -oE "^MSG $RUB [0-9]+ [0-9a-f]+" | head -1 | awk '{print $4}' | xxd -r -p
}
R=$(chiedi 19742 "$SA" '{"op":"iscrivi","nome":"Anna"}')
echo "$R" | grep -q '"op":"iscritto","nome":"Anna"' && pass "Anna iscritta con il suo nome (attraverso la chat cifrata)" || fail "iscrizione: $R"
R=$(chiedi 19752 "$SB" '{"op":"iscrivi","nome":"Bruno"}')
echo "$R" | grep -q '"iscritto"' && pass "Bruno iscritto" || fail "iscrizione Bruno: $R"
R=$(chiedi 19742 "$SA" '{"op":"elenco"}')
python3 - "$R" "$ANNA" "$BRUNO" <<'PY' && pass "elenco: Anna e Bruno, con indirizzo e «in linea»" || fail "elenco: $R"
import json, sys
r = json.loads(sys.argv[1]); u = {x["a"]: x for x in r["utenti"]}
assert r["op"] == "elenco" and r["iscritto"] is True
assert u[sys.argv[2]]["n"] == "Anna" and u[sys.argv[3]]["n"] == "Bruno" and u[sys.argv[2]]["on"] and u[sys.argv[3]]["on"]
PY
R=$(chiedi 19742 "$SA" '{"op":"iscrivi","nome":"<script>"}')
echo "$R" | grep -q '"op":"errore"' && pass "nome non valido rifiutato" || fail "nome non valido accettato: $R"
R=$(chiedi 19752 "$SB" '{"op":"cancella"}'); R=$(chiedi 19742 "$SA" '{"op":"elenco"}')
echo "$R" | grep -q '"Bruno"' && fail "Bruno ancora in elenco dopo la cancellazione" || pass "cancellazione: Bruno non è più in rubrica"
D="$H/.nyctalus-rete/casa/rubrica"
[ -z "$(cat "$D"/cronologia/* 2>/dev/null)" ] && pass "la rubrica non conserva la cronologia delle richieste" || fail "cronologia conservata: $(ls "$D/cronologia")"
[ "$(stat -c %a "$D/rubrica.json")" = 600 ] && grep -q '"Anna"' "$D/rubrica.json" && ! grep -q '"Bruno"' "$D/rubrica.json" \
  && pass "elenco degli iscritti salvato (privato, 600)" || fail "rubrica.json"
# le interfacce: riquadro «Rubrica» dell'app Android, apertura diretta nella web-app,
# pulsante «Rubrica» nel menu del PC, finestra della rubrica nella chat del PC
J="$ROOT/mobile/android/app/src/main/java/org/nyctalus/mobile/MainActivity.java"; W="$ROOT/mobile/webapp/app.js"
grep -q 'casella("👥", "Rubrica"' "$J" && grep -q '#rubrica' "$J" && grep -q 'nyctApriRubrica' "$J" \
  && pass "app Android: riquadro «👥 Rubrica» nella schermata d'avvio" || fail "riquadro Rubrica nell'APK"
grep -q 'location.hash === "#rubrica"' "$W" && grep -q 'window.nyctApriRubrica' "$W" && grep -q 'elencoInAttesa' "$W" \
  && pass "web-app: si apre direttamente sulla rubrica (anche prima che la chat sia collegata)" || fail "apertura diretta della rubrica"
grep -q '_apri_rubrica' "$ROOT/pc/launcher/nyctalus_launcher/app.py" && grep -q 'apri-rubrica' "$ROOT/pc/chat/nyctalus_chat/app.py" \
  && pass "PC: pulsante «Rubrica» nel menu, apre la finestra della chat" || fail "pulsante Rubrica del menu PC"
PYTHONPATH="$ROOT/pc/chat:$ROOT/pc/browser" python3 -c "import nyctalus_chat.rubrica as r; assert r.FinestraRubrica" 2>/dev/null \
  && pass "PC: finestra della rubrica caricabile" || fail "finestra rubrica del PC"
[ "$KO" -eq 0 ]
