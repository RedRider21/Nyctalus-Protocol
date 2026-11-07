#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Protezione di serie (PIANO-ANONIMATO, Tappa 4): una rete nuova nasce con CAMUFFAMENTO (traffico
# reso rumore con la chiave della rete) e RUMORE DI FONDO. Il codice di rete li porta ai client
# (PC, telefono/Windows, installatore dei nodi). Con la chiave ci si collega; senza no. Rubrica e
# casella funzionano camuffate. Si spegne e riaccende (la chiave resta). Tutto in 127.0.0.1.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · protezione di serie (camuffamento e rumore di fondo)"
assicura_binario
H="$TMP/gestore"; mkdir -p "$H"
G="$ROOT/pc/gestore-rete/nyctalus-rete"
NYCTALUS_RUBRICA_PORTE=25630,25631,25632,25633,25634 HOME="$H" NYCTALUS_BIN="$BIN" \
  python3 "$G" casa --ip 127.0.0.1 --porta 25620 --porta-base 25621 --non-importare < /dev/null > "$TMP/casa.log" 2>&1 & PIDS+=($!)
for _ in $(seq 1 240); do grep -q "Rete di casa attiva" "$TMP/casa.log" && break; sleep 0.5; done
grep -q "Rete di casa attiva" "$TMP/casa.log" || { fail "rete di casa: $(tail -3 "$TMP/casa.log")"; exit 1; }
COD=$(grep -oE 'NYCT1:[A-Za-z0-9_-]+' "$TMP/casa.log" | head -1)
leggi() { python3 -c "import base64,json,sys;c=sys.argv[1][6:];print(json.loads(base64.urlsafe_b64decode(c+'='*(-len(c)%4))).get(sys.argv[2],''))" "$1" "$2"; }
CAM=$(leggi "$COD" camuffamento); RUM=$(leggi "$COD" rumore); CAS=$(leggi "$COD" casella)
PUB=$(python3 -c "import json;print(json.load(open('$H/.nyctalus-rete/casa/autorita.json'))['pubblica'])")
[[ "$CAM" =~ ^[0-9a-f]{64}$ ]] && [ "$RUM" = "0.2" ] && pass "rete nuova: camuffamento e rumore di fondo accesi di serie, nel codice di rete (rumore 0,2/s)" || fail "codice: cam=$CAM rum=$RUM"
[ "$(stat -c %a "$H/.nyctalus-rete/casa/protezione.json")" = 600 ] && pass "la chiave di camuffamento sta in un file privato (600)" || fail "permessi di protezione.json"
N=$(pgrep -af "nyctalus node --listen 0.0.0.0:2562[1-4]" | grep -c -- "--obfuscation $CAM --cover 0.2")
[ "$N" = 4 ] && pass "i 4 nodi della rete girano camuffati e con il rumore di fondo" || fail "nodi camuffati: $N"

curl -sf http://127.0.0.1:25620/directory.txt -o "$TMP/dir.txt"
"$BIN" start --directory "$TMP/dir.txt" --authority "$PUB" --socks 127.0.0.1:25670 --control 127.0.0.1:25671 --obfuscation "$CAM" --cover 0.2 > "$TMP/con.log" 2>&1 & PIDS+=($!)
"$BIN" start --directory "$TMP/dir.txt" --authority "$PUB" --socks 127.0.0.1:25680 --control 127.0.0.1:25681 > "$TMP/senza.log" 2>&1 & PIDS+=($!)
sleep 4
OK=0; for _ in $(seq 1 10); do curl -sf --max-time 20 --socks5-hostname 127.0.0.1:25670 http://127.0.0.1:25620/directory.txt -o "$TMP/via.txt" && cmp -s "$TMP/via.txt" "$TMP/dir.txt" && OK=1 && break; sleep 2; done
[ "$OK" = 1 ] && pass "client CON la chiave della rete: si collega e naviga attraverso 3 salti" || fail "con la chiave: $(tail -3 "$TMP/con.log")"
curl -sf --max-time 15 --socks5-hostname 127.0.0.1:25680 http://127.0.0.1:25620/directory.txt -o /dev/null \
  && fail "un client SENZA chiave è entrato" || pass "client SENZA la chiave: i nodi non gli rispondono (il traffico sembra rumore)"
OK=0; for _ in $(seq 1 20); do curl -sf --max-time 20 --socks5-hostname 127.0.0.1:25670 "http://$CAS/" > /dev/null && OK=1 && break; sleep 3; done
[ "$OK" = 1 ] && pass "rubrica e casella della rete funzionano camuffate (casella raggiungibile)" || fail "casella non raggiungibile con la chiave"

# i client prendono chiave e rumore dal codice: PC (importa) e telefono/Windows (orchestratore)
mkdir -p "$TMP/cliente"
HOME="$TMP/cliente" NYCTALUS_BIN="$BIN" python3 "$G" importa "$COD" > /dev/null 2>&1
python3 -c "import json,sys;c=json.load(open(sys.argv[1]));sys.exit(0 if c.get('camuffamento')==sys.argv[2] and c.get('rumore')==0.2 else 1)" \
  "$TMP/cliente/.nyctalus-browser/config.json" "$CAM" && pass "PC: «importa» salva chiave e rumore (li usano chat e browser all'avvio)" || fail "importa: $(cat "$TMP/cliente/.nyctalus-browser/config.json")"
grep -q 'PROT+=(--obfuscation "$CAM")' "$ROOT/strumenti/avvia-chat.sh" && grep -q 'opz_prot(cfg)' "$ROOT/mobile/orchestratore/nyctalus_mobile.py" \
  && pass "avvio della chat (PC) e servizio del telefono passano camuffamento e rumore al motore" || fail "avvio senza protezione"
O=$(cd "$TMP" && HOME="$TMP/cliente" python3 -c "import sys;sys.path.insert(0,sys.argv[1]);import nyctalus_mobile as m;r=m.leggi_codice(sys.argv[2]);print(' '.join(m.opz_prot(r)))" "$ROOT/mobile/orchestratore" "$COD" 2>&1)
[ "$O" = "--obfuscation $CAM --cover 0.2" ] && pass "telefono/Windows: dal codice → --obfuscation CHIAVE --cover 0.2" || fail "orchestratore: $O"

# installatore di un nodo con --codice (radice finta, systemctl finto)
RADICE="$TMP/radice"; F="$TMP/finti"; mkdir -p "$RADICE" "$F"
printf '#!/bin/sh\nexit 0\n' > "$F/systemctl"; printf '#!/bin/sh\nexit 0\n' > "$F/useradd"; chmod +x "$F"/*
PK="$TMP/Nyctalus-Server"; mkdir -p "$PK/bin"; cp "$BIN" "$PK/bin/nyctalus"
cp "$ROOT/server/installa-nodo.sh" "$PK/installa.sh"; cp "$ROOT/server/nyctalus-server" "$ROOT/server/disinstalla.sh" "$PK/"
env NYCTALUS_RADICE="$RADICE" PATH="$F:$PATH" bash "$PK/installa.sh" --ip 127.0.0.1 --nodi 1 --porta 25690 --si --codice "$COD" > "$TMP/inst.log" 2>&1
grep -q "NYCTALUS_OPZIONI=.*--obfuscation $CAM" "$RADICE/etc/nyctalus/server.conf" && grep -q "NYCTALUS_OPZIONI=.*--cover 0.2" "$RADICE/etc/nyctalus/server.conf" \
  && pass "installatore del nodo con --codice: prende chiave e rumore della rete" || fail "installatore: $(grep OPZIONI "$RADICE/etc/nyctalus/server.conf" 2>&1; tail -2 "$TMP/inst.log")"

# spegnere e riaccendere: il codice cambia, la chiave resta la stessa
P() { HOME="$H" NYCTALUS_BIN="$BIN" python3 "$G" protezione "$@" --casa 2>&1; }
P no > /dev/null; C2=$(cat "$H/.nyctalus-rete/casa/codice.txt")
[ -z "$(leggi "$C2" camuffamento)" ] && pass "«protezione no»: il codice non ha più la chiave" || fail "protezione no"
P si --rumore 0.5 > /dev/null; C3=$(cat "$H/.nyctalus-rete/casa/codice.txt")
[ "$(leggi "$C3" camuffamento)" = "$CAM" ] && [ "$(leggi "$C3" rumore)" = "0.5" ] \
  && pass "«protezione si --rumore 0.5»: stessa chiave di prima, rumore 0,5" || fail "riaccensione: $(leggi "$C3" camuffamento) $(leggi "$C3" rumore)"
P si --rumore 99 | grep -q "fra 0,1 e 10" && pass "rumore fuori misura rifiutato" || fail "rumore 99 accettato"
[ "$KO" -eq 0 ]
