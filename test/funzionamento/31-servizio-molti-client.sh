#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Un servizio .nyct serve MOLTI client collegati insieme e a lungo (come la rubrica con le
# sessioni della chat). Prima ogni «posto» del servizio restava occupato finché il client non se
# ne andava: dal 5° client collegato in poi nessuno entrava più (trovato dalla simulazione in
# Shadow, punto K). Qui 8 client, sulla rete di prova, tutti collegati nello stesso momento.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · un servizio .nyct con molti client contemporanei"
assicura_binario
rete_di_prova 22300
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:22380 --hops 3 --control 127.0.0.1:22390 > "$TMP/c.log" 2>&1 & PIDS+=($!)
# il «backend»: un eco riga per riga, una connessione per client
python3 -c "
import socketserver
class E(socketserver.StreamRequestHandler):
    def handle(self):
        for r in self.rfile:
            self.wfile.write(r)
socketserver.ThreadingTCPServer.allow_reuse_address = True
socketserver.ThreadingTCPServer(('127.0.0.1', 22381), E).serve_forever()" & PIDS+=($!)
ADDR=$("$BIN" address --identity "$TMP/servizio" | grep -oE '[a-z2-7]{56}\.nyct' | tail -1)
"$BIN" site --backend 127.0.0.1:22381 --identity "$TMP/servizio" --directory "$DIR" --authority "$PUB" --hops 3 > "$TMP/s.log" 2>&1 & PIDS+=($!)
for _ in $(seq 1 60); do grep -q "online" "$TMP/s.log" && break; sleep 0.5; done
sleep 4
R=$(timeout 200 python3 "$STRUMENTI/prova_molti_client.py" 22380 "$ADDR" 8 2>&1 | grep "^OK")
[ "$R" = "OK 8" ] && pass "8 client collegati insieme e a lungo allo stesso servizio: serviti tutti (il servizio ha 4 «posti» d'attesa)" \
  || fail "client serviti: $R su 8 · $(tail -3 "$TMP/s.log")"
[ "$KO" -eq 0 ]
