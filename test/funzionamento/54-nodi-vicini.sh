#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# NODI VICINI (router senza «NAT loopback»): chi ospita la rete, e chi sta nella stessa casa, non
# raggiunge i nodi con l'indirizzo PUBBLICO di casa. Qui l'indirizzo pubblico è finto e non
# risponde (198.51.100.7, documentazione): esattamente un router che non rimanda indietro.
# - casa --internet: i nodi si parlano lo stesso (NYCTALUS_VICINI pubblico=127.0.0.1), la rubrica parte;
# - il codice porta l'indirizzo interno di chi ospita («lan»);
# - `nyctalus-rete vicini` sul PC che ospita → «pubblico=127.0.0.1» e l'elenco su 127.0.0.1;
#   su un altro dispositivo di casa → «pubblico=IP interno»; lontano da casa → niente;
# - il motore con la regola costruisce il tunnel; senza, i nodi non rispondono.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · nodi vicini (rete ospitata in casa, router senza NAT loopback)"
assicura_binario
G="$ROOT/pc/gestore-rete/nyctalus-rete"
PUB=198.51.100.7
LAN=$(python3 -c "import socket;s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM);s.connect(('192.0.2.1',9));print(s.getsockname()[0])" 2>/dev/null)
H="$TMP/ospite"; mkdir -p "$H"
export NYCTALUS_BIN="$BIN" NYCTALUS_RUBRICA_PORTE=24530,24531,24532,24533,24534
HOME="$H" python3 "$G" casa --internet $PUB --porta 24520 --porta-base 24521 --senza-uscita < /dev/null > "$TMP/casa.log" 2>&1 &
CASA=$!; PIDS+=($CASA)
for _ in $(seq 1 240); do grep -q "Lascia questa finestra aperta" "$TMP/casa.log" && break; sleep 0.5; done
grep -q "Rete raggiungibile da Internet" "$TMP/casa.log" && ! grep -q "la rubrica.*non è partita\|casella cifrata non è partita" "$TMP/casa.log" \
  && pass "casa --internet con l'IP pubblico che non torna indietro: rete accesa, rubrica e casella partite (i nodi si parlano dall'interno)" \
  || fail "casa: $(tail -5 "$TMP/casa.log")"
grep -q "0/4 nodes reachable" "$TMP/casa.log" && pass "la prova «dal nome pubblico» resta onesta: da qui l'indirizzo pubblico non risponde (0/4)" || fail "prova dal nome pubblico: $(grep reachable "$TMP/casa.log")"
COD=$(cat "$H/.nyctalus-rete/internet/codice.txt")
CJ=$(python3 -c "import sys,base64,json;b=sys.argv[1][6:];print(json.dumps(json.loads(base64.urlsafe_b64decode(b+'='*(-len(b)%4)))))" "$COD")
if python3 -c "import ipaddress,sys;a=ipaddress.ip_address(sys.argv[1]);sys.exit(0 if any(a in ipaddress.ip_network(n) for n in ('10.0.0.0/8','172.16.0.0/12','192.168.0.0/16')) else 1)" "$LAN" 2>/dev/null; then
  echo "$CJ" | grep -q "\"lan\": \"$LAN\"" && pass "il codice di rete porta l'indirizzo in casa di chi ospita ($LAN)" || fail "lan nel codice: $CJ"
else
  salta "questo computer non ha un indirizzo di rete locale privato: il campo «lan» non si prova"
fi

# 1) il PC che ospita: anche la sua chat usa la rete (importa) → vicini = 127.0.0.1
HOME="$H" python3 "$G" importa "$COD" > /dev/null 2>&1
V1=$(HOME="$H" python3 "$G" vicini); U1=$(HOME="$H" python3 "$G" vicini --url)
[ "$V1" = "$PUB=127.0.0.1" ] && [ "$U1" = "http://127.0.0.1:24520/directory.txt" ] \
  && pass "sul PC che ospita: «vicini» → $V1, elenco da $U1" || fail "vicini sul PC che ospita: «$V1» «$U1»"

# 2) un altro dispositivo della stessa casa (non ospita niente): usa l'indirizzo interno del codice
if [ -n "$LAN" ] && echo "$CJ" | grep -q '"lan"'; then
  H2="$TMP/telefono"; mkdir -p "$H2"
  HOME="$H2" python3 "$G" importa "$COD" > /dev/null 2>&1
  V2=$(HOME="$H2" python3 "$G" vicini)
  [ "$V2" = "$PUB=$LAN" ] && pass "un altro dispositivo in casa: «vicini» → $V2 (indirizzo interno di chi ospita)" || fail "vicini in casa: «$V2»"
fi
# 2-bis) il telefono (orchestratore di Termux) fa lo stesso conto
if [ -n "$LAN" ] && echo "$CJ" | grep -q '"lan"'; then
  VT=$(HOME="$TMP/tel-termux" python3 -c "
import sys; sys.path.insert(0, sys.argv[1])
import nyctalus_mobile as m
r = m.leggi_codice(sys.argv[2]); print(m.vicini(r)[0]); print(m.vicini(dict(r, lan='10.255.255.1'))[0] or '-')" "$ROOT/mobile/orchestratore" "$COD" 2>&1 | tr '\n' ' ')
  [ "$VT" = "$PUB=$LAN - " ] && pass "telefono in casa: stessa regola ($PUB=$LAN); lontano da casa nessuna" || fail "telefono: «$VT»"
fi
# 3) lontano da casa (l'indirizzo interno non risponde): nessuna regola, strada normale
H3="$TMP/lontano"; mkdir -p "$H3"
LONTANO=$(python3 -c "import sys,base64,json;b=sys.argv[1][6:];c=json.loads(base64.urlsafe_b64decode(b+'='*(-len(b)%4)));c['lan']='10.255.255.1';print('NYCT1:'+base64.urlsafe_b64encode(json.dumps(c).encode()).decode().rstrip('='))" "$COD")
HOME="$H3" python3 "$G" importa "$LONTANO" > /dev/null 2>&1
V3=$(HOME="$H3" timeout 20 python3 "$G" vicini)
[ -z "$V3" ] && pass "lontano da casa (indirizzo interno che non risponde): nessuna regola, si usa l'indirizzo pubblico" || fail "lontano: «$V3»"

# 4) il motore: senza la regola i nodi non rispondono, con la regola sì e il tunnel si costruisce
AUTH=$(python3 -c "import json,sys;print(json.load(open(sys.argv[1]))['authority'])" "$H/.nyctalus-rete/internet/nyctalus-rete.json")
CAM=$(python3 -c "import json,sys;print(json.load(open(sys.argv[1])).get('camuffamento',''))" "$H/.nyctalus-rete/internet/nyctalus-rete.json")
OP=(); [ -n "$CAM" ] && OP=(--obfuscation "$CAM")
curl -sf "$U1" -o "$TMP/dir.txt"
N0=$("$BIN" nodes --directory "$TMP/dir.txt" --authority "$AUTH" --probe --timeout 2 "${OP[@]}" 2>&1 | grep -oE "[0-9]+/[0-9]+ nodes reachable")
N1=$(NYCTALUS_VICINI="$V1" "$BIN" nodes --directory "$TMP/dir.txt" --authority "$AUTH" --probe --timeout 3 "${OP[@]}" 2>&1 | grep -oE "[0-9]+/[0-9]+ nodes reachable")
[ "$N0" = "0/4 nodes reachable" ] && [ "$N1" = "4/4 nodes reachable" ] && pass "motore: senza la regola 0/4 nodi, con NYCTALUS_VICINI 4/4" || fail "sonde: senza «$N0», con «$N1»"
NYCTALUS_VICINI="$V1" NYCTALUS_GUARDIE="$TMP/guardie" "$BIN" start --directory "$TMP/dir.txt" --authority "$AUTH" --socks 127.0.0.1:24540 "${OP[@]}" > "$TMP/tunnel.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/tunnel.log" "SOCKS5 proxy" 60 > /dev/null && pass "il tunnel si costruisce verso i nodi annunciati con l'indirizzo pubblico di casa" || fail "tunnel: $(tail -3 "$TMP/tunnel.log")"
kill -INT $CASA 2>/dev/null; for _ in $(seq 1 40); do kill -0 $CASA 2>/dev/null || break; sleep 0.25; done
[ "$KO" -eq 0 ]
