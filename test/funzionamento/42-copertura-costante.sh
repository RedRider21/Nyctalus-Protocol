#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# COPERTURA COSTANTE (scaletta punto 4): con `--constant-rate N` il traffico va a ritmo fisso in
# entrambi i versi (celle vere o finte): chi guarda la linea vede un flusso piatto, da fermi e
# durante uno scaricamento. Un relè fra client e nodo conta i byte al secondo in ciascun verso.
# Rete di un nodo e 1 salto (solo per poter misurare la linea client↔nodo isolata). Tutto locale.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · copertura costante (linea piatta)"
assicura_binario
B=23120; R=23130
K=$(python3 -c "import os;print(os.urandom(32).hex())")
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
NYCTALUS_DIAGNOSI=1 "$BIN" node --listen 127.0.0.1:$B --announce 127.0.0.1:$R --data "$TMP/n" --obfuscation "$K" > "$TMP/n.log" 2>&1 & PIDS+=($!)
L=$(attendi_regex "$TMP/n.log" "127\.0\.0\.1:$R,[0-9a-f]+,[0-9a-f]+") || true
[ -z "$L" ] && { fail "nodo non partito"; exit 1; }
"$BIN" directory --authority-secret "$SEC" --entries "$L" > "$TMP/dir.txt" 2>/dev/null
# relè che conta i byte per mezzo secondo: «su» (client→nodo) e «giu» (nodo→client)
python3 - "$R" "$B" "$TMP/linea.txt" <<'PY' > /dev/null 2>&1 & PIDS+=($!)
import select, socket, sys, time
r, b, f = int(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
pub = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); pub.bind(("127.0.0.1", r))
nodo = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); nodo.bind(("127.0.0.1", 0))
cliente = None; su = giu = 0; t0 = time.time()
with open(f, "w") as out:
    while True:
        pronti, _, _ = select.select([pub, nodo], [], [], 0.05)
        for s in pronti:
            d, da = s.recvfrom(65535)
            if s is pub:
                cliente = da; su += len(d); nodo.sendto(d, ("127.0.0.1", b))
            elif cliente:
                giu += len(d); pub.sendto(d, cliente)
        if time.time() - t0 >= 0.5:
            out.write("%.1f %d %d\n" % (time.time(), su * 2, giu * 2)); out.flush(); su = giu = 0; t0 = time.time()
PY
mkdir -p "$TMP/www"; head -c 600000 /dev/urandom > "$TMP/www/f.bin"
python3 -m http.server $((B+20)) --bind 127.0.0.1 --directory "$TMP/www" > /dev/null 2>&1 & PIDS+=($!)
media() { python3 - "$TMP/linea.txt" "$1" "$2" "$3" <<'PY'
import sys
righe=[l.split() for l in open(sys.argv[1]) if l.strip()]
da, a, col = float(sys.argv[2]), float(sys.argv[3]), int(sys.argv[4])
v=[int(x[col]) for x in righe if da <= float(x[0]) <= a]
print(int(sum(v)/len(v)) if v else 0)
PY
}
export NYCTALUS_GUARDIE="$TMP/guardie"
"$BIN" circuit --directory "$TMP/dir.txt" --authority "$PUB" --obfuscation "$K" --hops 1 --circuits 1 --constant-rate 60 --constant-always \
    --socks 127.0.0.1:$((B+10)) --control 127.0.0.1:$((B+11)) > "$TMP/c.log" 2>&1 & CPID=$!; PIDS+=($CPID)
attendi_regex "$TMP/c.log" "SOCKS5 proxy" 25 > /dev/null
grep -q "Constant cover: 60 cells/s each way, always" "$TMP/c.log" && pass "il client parte con la copertura costante (60 celle/s, sempre)" || fail "avvio: $(tail -3 "$TMP/c.log")"
curl -s -m 10 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/ > /dev/null 2>&1   # attiva il gruppo all'uscita
sleep 1; A=$(date +%s.%N); sleep 4; Z=$(date +%s.%N)
SU_FERMO=$(media "$A" "$Z" 1); GIU_FERMO=$(media "$A" "$Z" 2)
grep -q "copertura costante 60 celle/s (sempre)" "$TMP/n.log" && pass "l'uscita ha ricevuto la richiesta (ritorno a ritmo fisso)" || fail "uscita: nessuna richiesta"
[ "$SU_FERMO" -gt 50000 ] && [ "$GIU_FERMO" -gt 50000 ] && pass "da fermi la linea NON è vuota: su ${SU_FERMO} B/s, giù ${GIU_FERMO} B/s" || fail "da fermi: su $SU_FERMO giù $GIU_FERMO"
A=$(date +%s.%N)
curl -s -m 60 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/f.bin -o "$TMP/f.bin"
Z=$(date +%s.%N)
cmp -s "$TMP/f.bin" "$TMP/www/f.bin" && pass "lo scaricamento arriva integro ($(python3 -c "print(round($Z-$A,1))") s: limitato dal ritmo)" || fail "file diverso"
GIU_USO=$(media "$A" "$Z" 2); SU_USO=$(media "$A" "$Z" 1)
python3 -c "import sys;sys.exit(0 if 0.75 <= $GIU_USO/$GIU_FERMO <= 1.3 and 0.75 <= $SU_USO/$SU_FERMO <= 1.3 else 1)" \
  && pass "linea piatta: durante lo scaricamento giù ${GIU_USO} B/s, su ${SU_USO} B/s (da fermi ${GIU_FERMO}/${SU_FERMO})" \
  || fail "non piatta: giù uso $GIU_USO vs fermo $GIU_FERMO, su uso $SU_USO vs fermo $SU_FERMO"
kill $CPID 2>/dev/null; wait $CPID 2>/dev/null
# l'uscita smette da sola quando il client sparisce (scadenza del collegamento, 15 s)
sleep 18
A=$(date +%s.%N); sleep 2; Z=$(date +%s.%N)
DOPO=$(media "$A" "$Z" 2)
[ "$DOPO" -lt 5000 ] && pass "client chiuso: l'uscita smette da sola le celle finte (giù ${DOPO} B/s)" || fail "l'uscita continua a mandare: $DOPO B/s"

# senza copertura: da fermi la linea è quasi muta, durante lo scaricamento un picco
"$BIN" circuit --directory "$TMP/dir.txt" --authority "$PUB" --obfuscation "$K" --hops 1 --circuits 1 \
    --socks 127.0.0.1:$((B+12)) --control 127.0.0.1:$((B+13)) > "$TMP/c2.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/c2.log" "SOCKS5 proxy" 25 > /dev/null
curl -s -m 10 --socks5-hostname 127.0.0.1:$((B+12)) http://127.0.0.1:$((B+20))/ > /dev/null 2>&1
sleep 1; A=$(date +%s.%N); sleep 3; Z=$(date +%s.%N)
GIU_MUTO=$(media "$A" "$Z" 2)
[ "$GIU_MUTO" -lt 5000 ] && pass "per confronto, senza copertura da fermi la linea è quasi muta (giù ${GIU_MUTO} B/s): si vedrebbe quando usi la rete" || fail "senza copertura: $GIU_MUTO"
[ "$KO" -eq 0 ]
