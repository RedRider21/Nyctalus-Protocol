# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""N client collegati INSIEME e A LUNGO allo stesso servizio .nyct (come le sessioni della chat con
la rubrica): ognuno apre il suo collegamento attraverso la rete, manda una riga, riceve l'eco e
resta collegato finché TUTTI non hanno ricevuto la loro. Stampa «OK n» con quanti ce l'hanno fatta.
uso: prova_molti_client.py PORTA_SOCKS INDIRIZZO.nyct N"""
import socket
import struct
import sys
import threading
import time

socks, indirizzo, n = int(sys.argv[1]), sys.argv[2], int(sys.argv[3])
riusciti, aperti, tutti = [], [], threading.Event()


def client(i):
    try:
        s = socket.create_connection(("127.0.0.1", socks), timeout=120)
        s.sendall(b"\x05\x01\x00")
        s.recv(2)
        h = indirizzo.encode()
        s.sendall(b"\x05\x01\x00\x03" + bytes([len(h)]) + h + struct.pack("!H", 80))
        r = s.recv(10)
        if len(r) < 2 or r[1] != 0:
            return
        aperti.append(s)
        s.sendall(b"ciao %d\n" % i)
        dati = b""
        while not dati.endswith(b"\n"):
            d = s.recv(100)
            if not d:
                return
            dati += d
        if dati == b"ciao %d\n" % i:
            riusciti.append(i)
        tutti.wait(150)                  # resta collegato: i posti del servizio restano occupati
    except OSError:
        pass


fili = [threading.Thread(target=client, args=(i,), daemon=True) for i in range(n)]
for f in fili:
    f.start()
    time.sleep(0.3)
fine = time.time() + 150
while time.time() < fine and len(riusciti) < n:
    time.sleep(0.5)
print("OK %d" % len(riusciti), flush=True)
tutti.set()
