# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Prova il flusso /api/notifiche dell'orchestratore (quello che ascolta il servizio di
notifica dell'app Android): ci si manda un messaggio e deve arrivare la riga di notifica.
Stampa righe «OK descrizione» / «KO descrizione».   uso: prova_notifiche.py PORTA SEGRETO"""
import base64
import http.client
import json
import os
import socket
import sys
import time

porta, tok = int(sys.argv[1]), sys.argv[2]
esito = lambda ok, d: print(("OK " if ok else "KO ") + d)

c = http.client.HTTPConnection("127.0.0.1", porta, timeout=10)
c.request("GET", "/api/notifiche")
esito(c.getresponse().status == 403, "senza il segreto dell'app il flusso delle notifiche è negato")

# il flusso, come lo apre l'app (cookie con il segreto)
f = socket.create_connection(("127.0.0.1", porta), timeout=60)
f.sendall(("GET /api/notifiche HTTP/1.1\r\nHost: x\r\nCookie: nyct=%s\r\n\r\n" % tok).encode())
flusso = f.makefile("rb")
testa = flusso.readline()
while flusso.readline() not in (b"\r\n", b""):
    pass
prima = json.loads(flusso.readline() or b"{}")
esito(testa.startswith(b"HTTP/1.1 200") and prima == {"tipo": "motore", "ok": True}, "flusso delle notifiche aperto e collegato al motore")

# un messaggio a se stessi attraverso il ponte WebSocket della web-app
s = socket.create_connection(("127.0.0.1", porta), timeout=10)
s.sendall(("GET /ws HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: %s\r\n"
           "Sec-WebSocket-Version: 13\r\nCookie: nyct=%s\r\n\r\n" % (base64.b64encode(os.urandom(16)).decode(), tok)).encode())
s.recv(4096)


def manda(t):
    d = t.encode(); m = os.urandom(4)
    lung = bytes([0x80 | len(d)]) if len(d) < 126 else bytes([0x80 | 126]) + len(d).to_bytes(2, "big")
    s.sendall(bytes([0x81]) + lung + m + bytes(x ^ m[i % 4] for i, x in enumerate(d)))


manda("WHOAMI")
time.sleep(1)
buf = s.recv(65536)
io = buf[buf.find(b"ME ") + 3:].split(b"\n")[0].split(b" ")[0].decode()
manda("SEND %s %s" % (io, "Ciao dalla notifica".encode().hex()))
notifica, fine = None, time.time() + 60
while time.time() < fine:
    riga = flusso.readline()
    if not riga:
        break
    o = json.loads(riga)
    if o.get("tipo") == "msg":
        notifica = o
        break
esito(bool(notifica) and notifica.get("da") == io and notifica.get("testo") == "Ciao dalla notifica",
      "messaggio in arrivo → riga di notifica con mittente e testo (%s)" % (notifica or "nessuna"))
