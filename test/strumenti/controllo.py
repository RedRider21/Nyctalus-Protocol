# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Parla con la porta di controllo del motore della chat (per le prove).

uso: controllo.py PORTA SEGRETO|- ATTESA_SEC COMANDO [COMANDO...]
Manda AUTH (se c'è il segreto) e i comandi, poi stampa ogni riga ricevuta
entro ATTESA_SEC secondi. Nei comandi, {hex:testo} diventa l'esadecimale di testo.
"""
import re
import socket
import sys
import time

porta, segreto, attesa = int(sys.argv[1]), sys.argv[2], float(sys.argv[3])
comandi = [re.sub(r"\{hex:(.*?)\}", lambda m: m.group(1).encode().hex(), c) for c in sys.argv[4:]]
s = socket.create_connection(("127.0.0.1", porta), timeout=5)
if segreto != "-":
    s.sendall(("AUTH %s\n" % segreto).encode())
for c in comandi:
    s.sendall((c + "\n").encode())
fine = time.time() + attesa
buf = b""
s.settimeout(0.3)
while time.time() < fine:
    try:
        d = s.recv(65536)
        if not d:
            break
        buf += d
    except socket.timeout:
        pass
for riga in buf.decode("utf-8", "replace").splitlines():
    print(riga)
