# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Prova l'orchestratore in esecuzione: stato, ponte WebSocket, caricamento,
lettura a pezzi (Range). Stampa righe «OK descrizione» / «KO descrizione».
uso: prova_orchestratore.py PORTA SEGRETO"""
import base64
import http.client
import json
import os
import socket
import sys
import time

porta, tok = int(sys.argv[1]), sys.argv[2]
esito = lambda ok, d: print(("OK " if ok else "KO ") + d)


def r(m, u, corpo=None, h=None):
    c = http.client.HTTPConnection("127.0.0.1", porta, timeout=15)
    hh = {"Cookie": "nyct=" + tok, **(h or {})}
    c.request(m, u, body=corpo, headers=hh)
    x = c.getresponse()
    return x.status, dict(x.getheaders()), x.read()


st, _h, b = r("GET", "/api/stato")
info = json.loads(b)
esito(info.get("stato") == "pronto" and info.get("indirizzo", "").endswith(".nyct"), "stato «pronto» con il Contact ID")
st, h, _b = r("GET", "/?t=" + tok)
esito(st == 303 and "Max-Age" in h.get("Set-Cookie", ""), "il segreto diventa un cookie persistente")
st, _h, b = r("GET", "/")
esito(st == 200 and b"<title>Nyctalus</title>" in b, "la web-app viene servita")

# ponte WebSocket → motore
s = socket.create_connection(("127.0.0.1", porta), timeout=10)
s.sendall(("GET /ws HTTP/1.1\r\nHost: x\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: %s\r\n"
           "Sec-WebSocket-Version: 13\r\nCookie: nyct=%s\r\n\r\n" % (base64.b64encode(os.urandom(16)).decode(), tok)).encode())
esito(s.recv(4096).startswith(b"HTTP/1.1 101"), "WebSocket aperto")


def manda(t):
    d = t.encode(); m = os.urandom(4)
    s.sendall(bytes([0x81, 0x80 | len(d)]) + m + bytes(x ^ m[i % 4] for i, x in enumerate(d)))


manda("WHOAMI"); manda("LIST")
time.sleep(1); s.settimeout(1); buf = b""
try:
    while True:
        buf += s.recv(65536)
except Exception:
    pass
testi, i = [], 0
while i + 2 <= len(buf):
    n = buf[i + 1] & 0x7F; i += 2
    if n == 126:
        n = int.from_bytes(buf[i:i + 2], "big"); i += 2
    testi.append(buf[i:i + n].decode()); i += n
esito(any(t.startswith("ME ") for t in testi) and "ENDC" in testi, "il motore risponde attraverso il ponte")
esito(not any("AUTH" in t for t in testi), "il segreto del motore non arriva mai alla web-app")

# caricamento + lettura a pezzi
dati = os.urandom(300000)
st, _h, b = r("POST", "/api/carica?nome=prova.bin", dati, {"Origin": "http://127.0.0.1:%d" % porta})
perc = json.loads(b).get("percorso", "")
esito(st == 200 and os.path.basename(perc) == "prova.bin", "file caricato dal «telefono»")
st, h, b = r("GET", "/api/file?p=" + perc.encode().hex(), h={"Range": "bytes=1000-1999"})
esito(st == 206 and b == dati[1000:2000], "lettura a pezzi (Range) esatta")
st, _h, b = r("GET", "/api/file?p=" + perc.encode().hex())
esito(st == 200 and b == dati, "lettura completa identica")
