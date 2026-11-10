#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# FORMA «VIDEOCHIAMATA» (PIANO-ANONIMATO, invisibilità punto 2): il traffico ha la forma di una
# chiamata WebRTC: Binding Request/Success di STUN (ICE) e pacchetti SRTP video con l'etichetta di
# autenticazione fatta con la chiave della rete. Fra i nodi c'è un OSSERVATORE (al posto di tshark)
# che inoltra e classifica ogni datagramma, come un filtro DPI. Tutto in locale (127.0.0.1).
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · forma videochiamata (WebRTC: STUN + SRTP)"
assicura_binario
B=22620; R=22720
K=$(python3 -c "import os;print(os.urandom(32).hex())")
export NYCTALUS_WEBRTC=1
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
# l'osservatore: porte pubbliche R.. → porte vere B..
python3 "$ROOT/simulazione/osserva_webrtc.py" --coppie "$R:$B,$((R+1)):$((B+1)),$((R+2)):$((B+2)),$((R+3)):$((B+3))" \
    --rapporto "$TMP/osservato.json" > "$TMP/oss.log" 2>&1 & PIDS+=($!)
righe=""
for i in 0 1 2 3; do
  p=$((B+i)); a=$((R+i))
  "$BIN" node --listen 127.0.0.1:$p --announce 127.0.0.1:$a --data "$TMP/n$p" --obfuscation "$K" > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
done
for i in 0 1 2 3; do
  p=$((B+i)); a=$((R+i))
  L=$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$a,[0-9a-f]+,[0-9a-f]+")
  [ -z "$L" ] && { fail "nodo $p non partito: $(tail -2 "$TMP/n$p.log")"; exit 1; }
  righe="$righe${righe:+;}$L"
done
grep -q "transport form: video call" "$TMP/n$B.log" && pass "i nodi partono nella forma videochiamata" || fail "forma videochiamata non attiva"
DIR="$TMP/dir.txt"
"$BIN" directory --authority-secret "$SEC" --entries "$righe" > "$DIR" 2>/dev/null
mkdir -p "$TMP/www"; head -c 400000 /dev/urandom | base64 > "$TMP/www/grande.txt"; echo "ciao dal sito" > "$TMP/www/index.html"
python3 -m http.server $((B+20)) --bind 127.0.0.1 --directory "$TMP/www" > /dev/null 2>&1 & PIDS+=($!)

# 1) navigare attraverso 3 salti (tutto il traffico fra client e nodi passa dall'osservatore)
"$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$K" --socks 127.0.0.1:$((B+10)) --control 127.0.0.1:$((B+11)) --hops 3 > "$TMP/c.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/c.log" "SOCKS5 proxy" 20 > /dev/null
R1=$(curl -s -m 20 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R1" = "ciao dal sito" ] && pass "si naviga attraverso 3 salti con i nodi in forma videochiamata" || fail "navigazione: $R1 · $(tail -3 "$TMP/c.log")"
curl -s -m 40 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/grande.txt -o "$TMP/scaricato.txt"
cmp -s "$TMP/scaricato.txt" "$TMP/www/grande.txt" && pass "scaricati 540 KB integri nella forma videochiamata" || fail "file grande diverso"
sleep 6   # almeno un ricontrollo STUN della linea (ogni ~4,5 s)

# 2) cosa vede chi osserva la rete
sleep 1.5
OSS=$(cat "$TMP/osservato.json" 2>/dev/null)
v() { python3 -c "import json,sys;d=json.loads(sys.argv[1]);print(eval(sys.argv[2]))" "$OSS" "$1"; }
[ "$(v "d['pacchetti']>200")" = True ] && [ "$(v "d['riconosciuti_percento']")" = "100.0" ] \
  && pass "osservatore: $(v "d['pacchetti']") pacchetti, 100% riconosciuti come STUN o SRTP (nessun «altro»)" || fail "osservatore: $OSS"
[ "$(v "d['classi'].get('stun-richiesta',0)>=4 and d['classi'].get('stun-risposta',0)>=4")" = True ] \
  && pass "STUN: Binding Request e Binding Success con FINGERPRINT corretto ($(v "d['classi'].get('stun-richiesta',0)") richieste, ricontrolli periodici)" || fail "STUN: $OSS"
[ "$(v "d['flussi_srtp']>=4 and d['sequenze_rotte']==0")" = True ] \
  && pass "SRTP: $(v "d['flussi_srtp']") flussi (SSRC), numeri di sequenza regolari" || fail "SRTP: $OSS"

# 3) chi sonda un nodo senza la chiave non riceve niente
S=$(python3 - "$B" <<'PY'
import os, socket, struct, sys
p = int(sys.argv[1]); s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.settimeout(1.5)
def prova(d):
    s.sendto(d, ("127.0.0.1", p))
    try:
        s.recvfrom(4096); return "risposta"
    except socket.timeout:
        return "silenzio"
stun = struct.pack("!HHI", 1, 0, 0x2112A442) + os.urandom(12)                        # STUN senza integrità
falso_mi = struct.pack("!HHI", 1, 24, 0x2112A442) + os.urandom(12) + struct.pack("!HH", 8, 20) + os.urandom(20)
rtp = bytes([0x80, 96]) + os.urandom(10) + os.urandom(200)                             # RTP con etichetta falsa
quic = bytes([0xC0]) + b"\x00\x00\x00\x01" + os.urandom(1195)                          # QUIC Initial
print(" ".join(prova(d) for d in (stun, falso_mi, rtp, quic, os.urandom(300))))
PY
)
[ "$S" = "silenzio silenzio silenzio silenzio silenzio" ] && pass "sonde (STUN senza integrità o falsa, RTP falso, QUIC, rumore): il nodo resta muto" || fail "sonde: $S"
R1=$(curl -s -m 20 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R1" = "ciao dal sito" ] && pass "dopo le sonde la rete funziona ancora" || fail "dopo le sonde: $R1"

# 4) con un'altra chiave non si entra
"$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$(python3 -c "import os;print(os.urandom(32).hex())")" \
    --socks 127.0.0.1:$((B+12)) --control 127.0.0.1:$((B+13)) --hops 3 > "$TMP/c2.log" 2>&1 & PIDS+=($!)
sleep 6
R1=$(curl -s -m 8 --socks5-hostname 127.0.0.1:$((B+12)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R1" != "ciao dal sito" ] && pass "con la chiave sbagliata non si entra nella rete" || fail "entrato con la chiave sbagliata"
# 5) senza chiave la forma non parte
"$BIN" node --listen 127.0.0.1:$((B+6)) --data "$TMP/senza" > "$TMP/senza.log" 2>&1
grep -q "needs the network key" "$TMP/senza.log" && pass "nodo senza chiave di rete: rifiuta di partire nella forma videochiamata" || fail "senza chiave: $(tail -2 "$TMP/senza.log")"
# 6) --webrtc e --mimic insieme: rifiutato
NYCTALUS_WEBRTC=0 "$BIN" node --listen 127.0.0.1:$((B+7)) --data "$TMP/doppia" --obfuscation "$K" --webrtc --mimic > "$TMP/doppia.log" 2>&1
grep -q "pick one" "$TMP/doppia.log" && pass "--webrtc con --mimic: rifiutato (una forma sola)" || fail "doppia forma: $(tail -2 "$TMP/doppia.log")"

# 7) ripiego su TCP: un vero TLS (come la forma HTTP/3). Seconda reticella senza osservatore
#    (l'osservatore inoltra solo UDP): 3 nodi diretti, client con NYCTALUS_TCP=1
righe2=""
for p in $((B+30)) $((B+31)) $((B+32)); do
  "$BIN" node --listen 127.0.0.1:$p --data "$TMP/t$p" --obfuscation "$K" > "$TMP/t$p.log" 2>&1 & PIDS+=($!)
done
for p in $((B+30)) $((B+31)) $((B+32)); do
  L=$(attendi_regex "$TMP/t$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+"); righe2="$righe2${righe2:+;}$L"
done
"$BIN" directory --authority-secret "$SEC" --entries "$righe2" > "$TMP/dir2.txt" 2>/dev/null
NYCTALUS_TCP=1 "$BIN" circuit --directory "$TMP/dir2.txt" --authority "$PUB" --obfuscation "$K" --socks 127.0.0.1:$((B+16)) --control 127.0.0.1:$((B+17)) --hops 3 > "$TMP/ct.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/ct.log" "SOCKS5 proxy" 30 > /dev/null
R1=$(curl -s -m 25 --socks5-hostname 127.0.0.1:$((B+16)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R1" = "ciao dal sito" ] && grep -q "entry over TCP" "$TMP/ct.log" && pass "ripiego su TCP nella forma videochiamata: si naviga dentro un TLS" \
  || fail "ripiego TCP: $R1 · $(tail -3 "$TMP/ct.log")"
S=$(echo | timeout 10 openssl s_client -connect 127.0.0.1:$B -alpn http/1.1 2>/dev/null)
echo "$S" | grep -q "ALPN protocol: http/1.1" && pass "openssl sulla porta TCP del nodo: stretta TLS normale (ALPN http/1.1)" || fail "openssl: $(echo "$S" | grep -E 'ALPN|error' | head -3)"

# 8) gestore, codice di rete, telefono, installatore dei server (HOME finte: mai i dati veri)
unset NYCTALUS_WEBRTC
G="$ROOT/pc/gestore-rete/nyctalus-rete"; HG="$TMP/gestore"; mkdir -p "$HG"
HOME="$HG" NYCTALUS_BIN="$BIN" python3 "$G" crea --nome prova > /dev/null 2>&1
HOME="$HG" NYCTALUS_BIN="$BIN" python3 "$G" protezione si --forma webrtc > "$TMP/prot.log" 2>&1
grep -q "videochiamata" "$TMP/prot.log" && pass "gestore: «protezione si --forma webrtc» (Forma: videochiamata)" || fail "gestore: $(tail -3 "$TMP/prot.log")"
HOME="$HG" NYCTALUS_BIN="$BIN" python3 "$G" codice --url http://127.0.0.1:9/directory.txt > "$TMP/cod.log" 2>&1
COD=$(grep -oE 'NYCT1:[A-Za-z0-9_-]+' "$TMP/cod.log" | head -1)
leggi() { python3 -c "import base64,json,sys;c=sys.argv[1][6:];print(json.loads(base64.urlsafe_b64decode(c+'='*(-len(c)%4))).get(sys.argv[2],''))" "$1" "$2"; }
[ "$(leggi "$COD" forma)" = webrtc ] && pass "la forma videochiamata finisce nel codice di rete" || fail "codice: $(tail -3 "$TMP/cod.log")"
CAM=$(leggi "$COD" camuffamento)
mkdir -p "$TMP/cliente"
HOME="$TMP/cliente" python3 "$G" importa "$COD" > /dev/null 2>&1
python3 -c "import json,sys;c=json.load(open(sys.argv[1]));sys.exit(0 if c.get('forma')=='webrtc' else 1)" "$TMP/cliente/.nyctalus-browser/config.json" \
  && pass "PC: «importa» salva la forma videochiamata (chat, browser e Firefox la passano al motore)" || fail "importa: $(cat "$TMP/cliente/.nyctalus-browser/config.json")"
O=$(cd "$TMP" && HOME="$TMP/cliente" python3 -c "import sys;sys.path.insert(0,sys.argv[1]);import nyctalus_mobile as m;print(' '.join(m.opz_prot(m.leggi_codice(sys.argv[2]))))" "$ROOT/mobile/orchestratore" "$COD" 2>&1)
[ "$O" = "--obfuscation $CAM --webrtc --cover 0.2" ] && pass "telefono/Windows: dal codice → --webrtc" || fail "orchestratore: $O"
RADICE="$TMP/radice"; F="$TMP/finti"; mkdir -p "$RADICE" "$F"
printf '#!/bin/sh\nexit 0\n' > "$F/systemctl"; printf '#!/bin/sh\nexit 0\n' > "$F/useradd"; chmod +x "$F"/*
PK="$TMP/Nyctalus-Server"; mkdir -p "$PK/bin"; cp "$BIN" "$PK/bin/nyctalus"
cp "$ROOT/server/installa-nodo.sh" "$PK/installa.sh"; cp "$ROOT/server/nyctalus-server" "$ROOT/server/disinstalla.sh" "$PK/"
env NYCTALUS_RADICE="$RADICE" PATH="$F:$PATH" bash "$PK/installa.sh" --ip 127.0.0.1 --nodi 1 --si --codice "$COD" > "$TMP/inst.log" 2>&1
grep -q "NYCTALUS_OPZIONI=.*--webrtc" "$RADICE/etc/nyctalus/server.conf" && pass "server: «installa.sh --codice» prende la forma videochiamata" \
  || fail "installatore: $(grep OPZIONI "$RADICE/etc/nyctalus/server.conf" 2>&1; tail -3 "$TMP/inst.log")"
[ "$KO" -eq 0 ]
