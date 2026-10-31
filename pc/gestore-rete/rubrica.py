#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""La RUBRICA di una rete Nyctalus.

È un «utente automatico» della rete: un motore della chat con un suo indirizzo .nyct, e
questo programma collegato alla sua porta di controllo. Iscriversi, chiedere l'elenco e
cancellarsi sono messaggi della chat cifrata verso la rubrica: la chat autentica già il
mittente, quindi nessuno può iscriversi a nome di un altro.

Richieste (testo JSON in un messaggio):   {"op": "iscrivi", "nome": "Daniele"}
                                          {"op": "cancella"}
                                          {"op": "elenco"}
                                          {"op": "ci_sono"}      (sono collegato)
Risposte:  {"op": "iscritto", "nome": …} · {"op": "cancellato"} ·
           {"op": "elenco", "utenti": [{"a": indirizzo, "n": nome, "on": bool, "v": ts}, …]}
           {"op": "errore", "msg": …}

Della rubrica resta solo l'elenco degli iscritti (nome, indirizzo, quando si sono iscritti):
la cronologia dei messaggi con la rubrica viene cancellata subito.

CASELLA CIFRATA (posta per chi non è in linea):
  - deposito: POST /deposita?a=DESTINATARIO sulla PORTA_CASELLA, che la rete raggiunge con un
    suo indirizzo .nyct (un `site` a parte). Il mittente arriva dai circuiti SENZA presentarsi:
    la casella sa solo che c'è posta per quell'indirizzo. La busta (Noise X) la apre solo il
    destinatario, che ci trova dentro la prova di chi l'ha scritta.
  - consegna: il motore del destinatario chiede «CASELLA RITIRA» dalla sua chat (autenticata:
    solo lui ritira la sua posta); si risponde «CASELLA POSTA id busta…», lui conferma con
    «CASELLA RICEVUTI id…» e le buste si cancellano. Se è in linea, la posta parte da sola.
  - limiti: busta ≤ 64 KB, 200 buste per destinatario, 50 MB in tutto, 14 giorni.
Uso: rubrica.py CARTELLA_DATI PORTA_CONTROLLO [PORTA_CASELLA]   (segreto in NYCTALUS_CHAT_TOKEN)
"""
import http.server
import json
import os
import re
import socket
import sys
import threading
import time
import urllib.parse
from pathlib import Path

MAX_ISCRITTI = 1000
MAX_NOME = 40
NOME_VALIDO = re.compile(r"^[^\x00-\x1f\x7f<>]{1,40}$")
INDIRIZZO = re.compile(r"^[a-z2-7]{56}\.nyct$")
ID_BUSTA = re.compile(r"^[0-9]{10}-[0-9a-f]{16}$")
BUSTA_MAX = 64 * 1024
BUSTE_PER_DESTINATARIO = 200
CASELLA_MAX = 50 * 1024 * 1024
CASELLA_GIORNI = 14
LOTTO_POSTA = 40 * 1024          # buste per messaggio di consegna (in esadecimale, sotto i 64 KB)


def hx(t):
    return t.encode().hex()


class Rubrica:
    def __init__(self, cartella, porta):
        self.file = Path(cartella) / "rubrica.json"
        self.porta = porta
        try:
            self.iscritti = json.loads(self.file.read_text())
        except Exception:      # noqa: BLE001
            self.iscritti = {}
        self.online, self.visti = set(), {}
        self.s = None
        self.blocco = threading.Lock()           # il deposito arriva da un altro thread
        self.casella = Path(cartella) / "casella"
        self.casella.mkdir(mode=0o700, exist_ok=True)

    def salva(self):
        tmp = self.file.with_suffix(".tmp")
        tmp.write_text(json.dumps(self.iscritti, ensure_ascii=False, indent=1))
        os.chmod(tmp, 0o600)
        tmp.replace(self.file)

    def manda(self, riga):
        with self.blocco:
            if self.s is not None:
                self.s.sendall((riga + "\n").encode())

    def rispondi(self, addr, obj):
        self.manda("SEND %s %s" % (addr, hx(json.dumps(obj, ensure_ascii=False, separators=(",", ":")))))

    def elenco(self):
        utenti = [{"a": a, "n": d["nome"], "on": a in self.online, "v": self.visti.get(a, d.get("v", 0))}
                  for a, d in self.iscritti.items()]
        utenti.sort(key=lambda u: (not u["on"], u["n"].lower()))
        return utenti

    # --- casella cifrata ------------------------------------------------------------
    def buste(self, addr):
        d = self.casella / addr
        return sorted(d.glob("*.busta")) if d.is_dir() else []

    def pulisci_casella(self):
        limite = time.time() - CASELLA_GIORNI * 86400
        for f in self.casella.glob("*/*.busta"):
            if f.stat().st_mtime < limite:
                f.unlink(missing_ok=True)

    def deposita(self, addr, busta):
        """Dal servizio di deposito (thread HTTP). Restituisce None o il motivo del rifiuto."""
        if not INDIRIZZO.match(addr):
            return "indirizzo non valido"
        if not 96 <= len(busta) <= BUSTA_MAX:
            return "busta di dimensione non valida"
        self.pulisci_casella()
        if len(self.buste(addr)) >= BUSTE_PER_DESTINATARIO:
            return "la casella di questo destinatario è piena"
        if sum(f.stat().st_size for f in self.casella.glob("*/*.busta")) + len(busta) > CASELLA_MAX:
            return "la casella della rete è piena"
        d = self.casella / addr
        d.mkdir(mode=0o700, exist_ok=True)
        nome = "%d-%s" % (int(time.time()), os.urandom(8).hex())
        tmp = d / (nome + ".tmp")
        tmp.write_bytes(busta)
        os.chmod(tmp, 0o600)
        tmp.replace(d / (nome + ".busta"))
        if addr in self.online:                  # è in linea: gliela si porta subito
            self.consegna(addr)
        return None

    def consegna(self, addr):
        lotto, peso = [], 0
        for f in self.buste(addr):
            voce = "%s %s" % (f.stem, f.read_bytes().hex())
            if lotto and peso + len(voce) > LOTTO_POSTA:
                break
            lotto.append(voce)
            peso += len(voce)
        if lotto:
            self.manda("SEND %s %s" % (addr, hx("CASELLA POSTA " + " ".join(lotto))))
            self.manda("CLEAR %s" % addr)

    def casella_richiesta(self, addr, testo):
        parti = testo.split()
        if parti[:2] == ["CASELLA", "RITIRA"]:
            self.consegna(addr)
        elif parti[:2] == ["CASELLA", "RICEVUTI"]:
            for i in parti[2:]:
                if ID_BUSTA.match(i):
                    (self.casella / addr / (i + ".busta")).unlink(missing_ok=True)
            self.consegna(addr)                  # se ne restano, il lotto successivo
        self.manda("CLEAR %s" % addr)

    def richiesta(self, addr, testo):
        if testo.startswith("CASELLA "):
            return self.casella_richiesta(addr, testo)
        try:
            r = json.loads(testo)
            op = r.get("op")
        except Exception:      # noqa: BLE001
            return self.rispondi(addr, {"op": "errore", "msg": "richiesta non valida"})
        if op == "iscrivi":
            nome = str(r.get("nome", "")).strip()
            if not NOME_VALIDO.match(nome):
                return self.rispondi(addr, {"op": "errore", "msg": "nome non valido (1-%d caratteri)" % MAX_NOME})
            if addr not in self.iscritti and len(self.iscritti) >= MAX_ISCRITTI:
                return self.rispondi(addr, {"op": "errore", "msg": "rubrica piena"})
            self.iscritti[addr] = {"nome": nome, "dal": int(time.time()), "v": int(time.time())}
            self.salva()
            self.rispondi(addr, {"op": "iscritto", "nome": nome})
        elif op == "cancella":
            if self.iscritti.pop(addr, None) is not None:
                self.salva()
            self.rispondi(addr, {"op": "cancellato"})
        elif op == "elenco":
            self.rispondi(addr, {"op": "elenco", "utenti": self.elenco(), "iscritto": addr in self.iscritti})
        elif op == "ci_sono":
            pass                                  # la sessione aperta basta a risultare in linea
        else:
            self.rispondi(addr, {"op": "errore", "msg": "operazione sconosciuta"})
        # niente cronologia delle richieste: la rubrica non tiene traccia di chi chiede cosa
        self.manda("CLEAR %s" % addr)

    def gira(self):
        while True:
            try:
                self.s = socket.create_connection(("127.0.0.1", self.porta), timeout=10)
                self.s.settimeout(None)
                segreto = os.environ.get("NYCTALUS_CHAT_TOKEN", "")
                if segreto:
                    self.manda("AUTH " + segreto)
                self.manda("SUB")
                buf = b""
                while True:
                    d = self.s.recv(65536)
                    if not d:
                        break
                    buf += d
                    while b"\n" in buf:
                        riga, buf = buf.split(b"\n", 1)
                        self.evento(riga.decode("utf-8", "replace"))
            except OSError:
                pass
            with self.blocco:
                self.s = None
            time.sleep(2)

    def evento(self, riga):
        p = riga.split(" ")
        if p[0] == "MSG" and len(p) >= 4:
            try:
                testo = bytes.fromhex(p[3]).decode("utf-8")
            except ValueError:
                return
            self.richiesta(p[1], testo)
        elif p[0] == "ST" and len(p) >= 3:
            (self.online.add if p[2] == "1" else self.online.discard)(p[1])
            self.visti[p[1]] = int(time.time())
            if p[2] == "1":
                self.consegna(p[1])              # appena in linea: la sua posta
        elif p[0] == "V" and len(p) >= 3:
            self.visti[p[1]] = int(p[2])
        elif p[0] == "FILE" and len(p) >= 3 and p[2] == "in":
            self.manda("CLEAR %s" % p[1])          # la rubrica non accetta allegati


def servi_casella(rubrica, porta):
    """Il servizio di deposito: lo raggiunge la rete con l'indirizzo .nyct della casella."""
    class Deposito(http.server.BaseHTTPRequestHandler):
        protocol_version = "HTTP/1.0"

        def rispondi(self, codice, testo):
            corpo = testo.encode()
            self.send_response(codice)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Length", str(len(corpo)))
            self.end_headers()
            self.wfile.write(corpo)

        def do_POST(self):
            u = urllib.parse.urlsplit(self.path)
            if u.path != "/deposita":
                return self.rispondi(404, "sconosciuto")
            try:
                n = int(self.headers.get("Content-Length", "0"))
            except ValueError:
                n = -1
            if not 0 < n <= BUSTA_MAX:
                return self.rispondi(413, "busta di dimensione non valida")
            busta = self.rfile.read(n)
            addr = urllib.parse.parse_qs(u.query).get("a", [""])[0]
            errore = rubrica.deposita(addr, busta)
            self.rispondi(200 if errore is None else 400, errore or "depositata")

        def do_GET(self):
            self.rispondi(200, "Casella cifrata Nyctalus: qui si lasciano solo buste chiuse.")

        def log_message(self, *a):        # nessun registro di chi deposita
            pass

    srv = http.server.ThreadingHTTPServer(("127.0.0.1", porta), Deposito)
    threading.Thread(target=srv.serve_forever, daemon=True).start()


if __name__ == "__main__":
    r = Rubrica(sys.argv[1], int(sys.argv[2]))
    if len(sys.argv) > 3:
        servi_casella(r, int(sys.argv[3]))
    r.gira()
