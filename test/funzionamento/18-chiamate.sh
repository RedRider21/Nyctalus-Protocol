#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Chiamate vocali, motore: su una rete di prova (circuiti a 3 salti, punto d'incontro)
# Alice chiama Bob, Bob risponde, 2 secondi di audio a pacchetti da 20 ms nei due sensi
# arrivano identici con poco ritardo, poi fine; un secondo invito viene rifiutato.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · chiamate (motore)"
assicura_binario
rete_di_prova 21000
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:21080 --hops 3 --control 127.0.0.1:21090 > "$TMP/c.log" 2>&1 & PIDS+=($!)
sleep 2
SA=alicesegreto0123456789abcdef; SB=bobsegreto0123456789abcdef00
identita alice 21011 21021 21080 "$SA"; ALICE=$ADDR
identita bob 21012 21022 21080 "$SB"; BOB=$ADDR
[ -n "$ALICE" ] && [ -n "$BOB" ] || { fail "identità non partite"; exit 1; }
sleep 4
python3 "$STRUMENTI/prova_chiamata.py" 21021 "$SA" 21022 "$SB" "$ALICE" "$BOB" > "$TMP/r.txt" 2>&1 || echo "KO prova interrotta: $(tail -1 "$TMP/r.txt")" >> "$TMP/r.txt"
while read -r esito resto; do
  case "$esito" in OK) pass "$resto" ;; KO) fail "$resto" ;; esac
done < "$TMP/r.txt"
# PREPARA: aprendo la chat di un contatto la strada verso di lui si prepara in sottofondo, così la
# chiamata squilla subito. Due chiamanti nuovi verso Bob: Carla chiama a freddo, Dario prepara prima.
SC=carlasegreto0123456789abcdef; SD=dariosegreto0123456789abcdef
identita carla 21013 21023 21080 "$SC"; CARLA=$ADDR
identita dario 21014 21024 21080 "$SD"; DARIO=$ADDR
sleep 4
python3 - 21022 "$SB" "$BOB" 21023 "$SC" 21024 "$SD" > "$TMP/prep.txt" 2>&1 <<'PY'
import socket, sys, time, threading
bp, bs, bob, cp, cs, dp, ds = sys.argv[1:8]
def apri(porta, seg):
    s = socket.create_connection(("127.0.0.1", int(porta))); s.sendall(("AUTH %s\n" % seg).encode()); return s, s.makefile("r")
arrivi = {}
def ascolta_bob():
    s, f = apri(bp, bs); s.sendall(b"SUB\n")
    for riga in f:
        p = riga.split()
        if p and p[0] == "CALLIN":
            arrivi.setdefault(p[1], time.time())
threading.Thread(target=ascolta_bob, daemon=True).start()
time.sleep(1)
def chiama(porta, seg, prepara):
    s, f = apri(porta, seg)
    if prepara:
        s.sendall(("PREPARA %s\n" % bob).encode()); time.sleep(12)
    t0 = time.time(); s.sendall(("CALL %s\n" % bob).encode())
    me = None
    s2, f2 = apri(porta, seg); s2.sendall(b"WHOAMI\n")
    for riga in f2:
        if riga.startswith("ME ") or riga.split()[0].endswith(".nyct"):
            me = riga.split()[-1]; break
    fine = time.time() + 60
    while time.time() < fine and me not in arrivi:
        time.sleep(0.05)
    return (arrivi[me] - t0) if me in arrivi else None
fredda = chiama(cp, cs, False)
calda = chiama(dp, ds, True)
print("FREDDA %s CALDA %s" % (fredda, calda))
PY
F=$(awk '/FREDDA/{print $2}' "$TMP/prep.txt"); C=$(awk '/FREDDA/{print $4}' "$TMP/prep.txt")
python3 -c "import sys;f,c=sys.argv[1],sys.argv[2];sys.exit(0 if c not in ('None','') and float(c)<1.5 and (f in ('None','') or float(c)<=float(f)) else 1)" "$F" "$C" \
  && pass "PREPARA (aprendo la chat): lo squillo arriva in $(printf '%.2f' "$C") s (a freddo: $( [ "$F" = None ] && echo 'non arrivato' || printf '%.2f s' "$F"))" || fail "PREPARA: $(cat "$TMP/prep.txt")"
[ "$KO" -eq 0 ]
