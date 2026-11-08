#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# RIPIEGO SU TCP (PIANO-ANONIMATO, invisibilità punto 2b): dove UDP è bloccato i pacchetti QUIC
# viaggiano dentro TCP verso l'entrata, camuffati come sempre. I nodi ascoltano anche in TCP sulla
# stessa porta; il client prova UDP e, se nessuno risponde, passa da solo a TCP.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · ripiego su TCP dove UDP è bloccato"
assicura_binario
B=22800
K=$(python3 -c "import os;print(os.urandom(32).hex())")
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
righe=""
for p in $B $((B+1)) $((B+2)); do
  "$BIN" node --listen 127.0.0.1:$p --obfuscation "$K" > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
done
for p in $B $((B+1)) $((B+2)); do
  L=$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+")
  [ -z "$L" ] && { fail "nodo $p non partito"; exit 1; }
  righe="$righe${righe:+;}$L"
done
DIR="$TMP/dir.txt"
"$BIN" directory --authority-secret "$SEC" --entries "$righe" > "$DIR" 2>/dev/null
ss -ltn 2>/dev/null | grep -q "127.0.0.1:$B " && pass "il nodo ascolta anche in TCP sulla stessa porta ($B)" || fail "nessun ascolto TCP sulla porta $B"
mkdir -p "$TMP/www"; head -c 1000000 /dev/urandom > "$TMP/www/f.bin"
python3 -m http.server $((B+20)) --bind 127.0.0.1 --directory "$TMP/www" > /dev/null 2>&1 & PIDS+=($!)

# 1) forzato su TCP
NYCTALUS_TCP=1 "$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$K" --socks 127.0.0.1:$((B+10)) --control 127.0.0.1:$((B+11)) --hops 3 > "$TMP/c.log" 2>&1 & PIDS+=($!)
CPID=$!
attendi_regex "$TMP/c.log" "SOCKS5 proxy" 30 > /dev/null
curl -s -m 30 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/f.bin -o "$TMP/giu.bin"
cmp -s "$TMP/www/f.bin" "$TMP/giu.bin" && grep -q "entry over TCP" "$TMP/c.log" \
  && pass "con NYCTALUS_TCP=1 si entra in TCP e 1 MB arriva identico (3 salti, camuffato)" || fail "TCP forzato: $(tail -3 "$TMP/c.log")"
GUARDIA=$(grep -oE "circuit 0: 127\.0\.0\.1:[0-9]+" "$TMP/c.log" | grep -oE "[0-9]+$")
ss -tnp 2>/dev/null | grep "pid=$CPID," | grep -q "127.0.0.1:$GUARDIA" && pass "il collegamento all'entrata è davvero TCP (porta $GUARDIA)" \
  || fail "nessuna connessione TCP del client verso $GUARDIA"

# 2) ripiego AUTOMATICO: un ponte raggiungibile SOLO in TCP (dietro un inoltro TCP, in UDP nessuno risponde)
"$BIN" node --listen 127.0.0.1:$((B+5)) --data "$TMP/ponte" --obfuscation "$K" --directory "$DIR" --authority "$PUB" --hidden-bridge > "$TMP/ponte.log" 2>&1 & PIDS+=($!)
PONTE=$(attendi_regex "$TMP/ponte.log" "127\.0\.0\.1:$((B+5)),[0-9a-f]{64},[0-9a-f]{64},[0-9a-f]{64}")
python3 - $((B+6)) $((B+5)) <<'PY' & PIDS+=($!)
import socket, sys, threading
s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1); s.bind(("127.0.0.1", int(sys.argv[1]))); s.listen()
def tubo(a, b):
    try:
        while (d := a.recv(65536)):
            b.sendall(d)
    except OSError:
        pass
    a.close(); b.close()
while True:
    c, _ = s.accept(); n = socket.create_connection(("127.0.0.1", int(sys.argv[2])))
    threading.Thread(target=tubo, args=(c, n), daemon=True).start(); threading.Thread(target=tubo, args=(n, c), daemon=True).start()
PY
sleep 1
SOLO_TCP="127.0.0.1:$((B+6)),${PONTE#*,}"
NYCTALUS_PONTI="$SOLO_TCP" "$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$K" --socks 127.0.0.1:$((B+12)) --control 127.0.0.1:$((B+13)) --hops 3 > "$TMP/c2.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/c2.log" "SOCKS5 proxy" 40 > /dev/null
R=$(curl -s -m 30 --socks5-hostname 127.0.0.1:$((B+12)) http://127.0.0.1:$((B+20))/f.bin -o "$TMP/giu2.bin" -w "%{http_code}")
cmp -s "$TMP/www/f.bin" "$TMP/giu2.bin" && grep -q "did not answer: trying TCP" "$TMP/c2.log" \
  && pass "UDP senza risposta: il client passa DA SOLO a TCP e scarica (ripiego automatico)" || fail "ripiego automatico: $R $(tail -3 "$TMP/c2.log")"

# 3) sonda sulla porta TCP: byte a caso, nessuna risposta
R=$(python3 -c "
import socket,os
s=socket.create_connection(('127.0.0.1',$B)); s.settimeout(3)
try:
    for _ in range(20): s.sendall(os.urandom(1300))
except OSError: print('chiusa dal nodo'); raise SystemExit
try: d=s.recv(100); print('ricevuti', len(d)) if d else print('chiusa dal nodo')
except socket.timeout: print('silenzio')
")
[ "$R" = "silenzio" ] && pass "una sonda TCP con byte a caso: nessuna risposta e il nodo non chiude (nessuna impronta)" || fail "sonda TCP: $R"
[ "$KO" -eq 0 ]
