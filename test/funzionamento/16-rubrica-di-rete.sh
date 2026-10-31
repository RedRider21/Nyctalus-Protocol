#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Rubrica e casella per una rete VERA (non quella di casa): il gestore crea la rete con i suoi
# nodi; su un'altra macchina (es. il server) «nyctalus-rete rubrica --codice» le accende; il
# gestore rifà il codice includendole («codice --rubrica --casella»); un utente si iscrive.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · rubrica e casella di una rete vera"
assicura_binario
G="$TMP/gestore"; SRV="$TMP/server"; mkdir -p "$G" "$SRV"
RETE() { local h="$1"; shift; HOME="$h" NYCTALUS_BIN="$BIN" python3 "$ROOT/pc/gestore-rete/nyctalus-rete" "$@"; }
RETE "$G" crea --nome "Rete di prova" > /dev/null || { fail "crea"; exit 1; }
PUB=$(python3 -c "import json;print(json.load(open('$G/.nyctalus-rete/autorita.json'))['pubblica'])")
righe=()
for p in 20801 20802 20803 20804; do
  mkdir -p "$TMP/n$p"
  "$BIN" node --listen 127.0.0.1:$p --data "$TMP/n$p" > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
done
for p in 20801 20802 20803 20804; do righe+=("$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+" 15)"); done
RETE "$G" aggiungi "${righe[@]}" > /dev/null && RETE "$G" firma > /dev/null || { fail "aggiungi/firma"; exit 1; }
RETE "$G" servi --porta 20800 --ip 127.0.0.1 > "$TMP/servi.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/servi.log" "pubblico" 15 > /dev/null
URL=http://127.0.0.1:20800/directory.txt
COD=$(RETE "$G" codice --url "$URL" | grep -oE 'NYCT1:[A-Za-z0-9_-]+')
leggi() { python3 -c "import base64,json,sys;c=sys.argv[1][6:];print(json.loads(base64.urlsafe_b64decode(c+'='*(-len(c)%4))).get(sys.argv[2],''))" "$1" "$2"; }
[ -z "$(leggi "$COD" rubrica)" ] && pass "rete nuova: il codice non ha ancora la rubrica" || fail "rubrica inattesa"

# sul «server»: rubrica e casella con il solo codice (niente chiave dell'autorità)
NYCTALUS_RUBRICA_PORTE=20810,20811,20812,20813,20814 RETE "$SRV" rubrica --codice "$COD" > "$TMP/rub.log" 2>&1 & PIDS+=($!)
for _ in $(seq 1 240); do grep -q "Casella:" "$TMP/rub.log" && break; sleep 0.5; done
RUB=$(grep -oE "Rubrica: [a-z2-7]{56}\.nyct" "$TMP/rub.log" | cut -d' ' -f2); CAS=$(grep -oE "Casella: [a-z2-7]{56}\.nyct" "$TMP/rub.log" | cut -d' ' -f2)
[ -n "$RUB" ] && [ -n "$CAS" ] && pass "nyctalus-rete rubrica --codice: rubrica e casella accese, indirizzi stampati" || fail "rubrica: $(tail -4 "$TMP/rub.log")"
grep -q "codice --url URL --rubrica $RUB --casella $CAS" "$TMP/rub.log" && pass "dice al gestore il comando esatto per includerle" || fail "istruzioni per il gestore"
[ ! -e "$SRV/.nyctalus-rete/autorita.json" ] && pass "sul server nessuna chiave dell'autorità" || fail "autorità creata sul server"

# il gestore rifà il codice: ora le include (e le ricorda)
COD2=$(RETE "$G" codice --url "$URL" --rubrica "$RUB" --casella "$CAS" | grep -oE 'NYCT1:[A-Za-z0-9_-]+')
[ "$(leggi "$COD2" rubrica)" = "$RUB" ] && [ "$(leggi "$COD2" casella)" = "$CAS" ] && pass "codice di rete con rubrica e casella" || fail "codice: $COD2"
COD3=$(RETE "$G" codice --url "$URL" | grep -oE 'NYCT1:[A-Za-z0-9_-]+')
[ "$COD3" = "$COD2" ] && pass "le ricorda: il codice rifatto le include ancora" || fail "dimenticate"
RETE "$G" codice --url "$URL" --rubrica sbagliato > /dev/null 2>&1 && fail "indirizzo non valido accettato" || pass "indirizzo .nyct non valido rifiutato"

# un utente della rete si iscrive alla rubrica (attraverso i nodi del gestore)
curl -s "$URL" -o "$TMP/dir.txt"
U=annasegreto0123456789abcdef01; mkdir -p "$TMP/anna"
"$BIN" start --directory "$TMP/dir.txt" --authority "$PUB" --socks 127.0.0.1:20820 --control 127.0.0.1:20823 > /dev/null 2>&1 & PIDS+=($!)
NYCTALUS_CHAT_TOKEN=$U "$BIN" chat --data "$TMP/anna" --backend 127.0.0.1:20821 --control 127.0.0.1:20822 --socks 127.0.0.1:20820 > "$TMP/anna.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/anna.log" '[a-z2-7]{56}\.nyct' 15 > /dev/null
"$BIN" site --backend 127.0.0.1:20821 --identity "$TMP/anna/identita" --directory "$TMP/dir.txt" --authority "$PUB" --hops 3 > /dev/null 2>&1 & PIDS+=($!)
sleep 6
R=$(NYCT_FINO="^MSG $RUB " controllo 20822 "$U" 120 SUB "SEND $RUB $(printf '{"op":"iscrivi","nome":"Anna"}' | xxd -p | tr -d '\n')" \
    | grep -oE "^MSG $RUB [0-9]+ [0-9a-f]+" | head -1 | awk '{print $4}' | xxd -r -p)
echo "$R" | grep -q '"op":"iscritto","nome":"Anna"' && pass "un utente della rete si iscrive alla rubrica sul server" || fail "iscrizione: $R"
U2=$(RETE "$SRV" rubrica --servizio --stampa)
echo "$U2" | grep -q "^ExecStart=.*nyctalus-rete rubrica$" && echo "$U2" | grep -q "^Restart=always" \
  && pass "--servizio: unità systemd dell'utente, si riavvia sempre" || fail "unità: $U2"
[ "$KO" -eq 0 ]
