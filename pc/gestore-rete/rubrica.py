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
Uso: rubrica.py CARTELLA_DATI PORTA_CONTROLLO      (segreto in NYCTALUS_CHAT_TOKEN)
"""
import json
import os
import re
import socket
import sys
import time
from pathlib import Path

MAX_ISCRITTI = 1000
MAX_NOME = 40
NOME_VALIDO = re.compile(r"^[^\x00-\x1f\x7f<>]{1,40}$")


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

    def salva(self):
        tmp = self.file.with_suffix(".tmp")
        tmp.write_text(json.dumps(self.iscritti, ensure_ascii=False, indent=1))
        os.chmod(tmp, 0o600)
        tmp.replace(self.file)

    def manda(self, riga):
        self.s.sendall((riga + "\n").encode())

    def rispondi(self, addr, obj):
        self.manda("SEND %s %s" % (addr, hx(json.dumps(obj, ensure_ascii=False, separators=(",", ":")))))

    def elenco(self):
        utenti = [{"a": a, "n": d["nome"], "on": a in self.online, "v": self.visti.get(a, d.get("v", 0))}
                  for a, d in self.iscritti.items()]
        utenti.sort(key=lambda u: (not u["on"], u["n"].lower()))
        return utenti

    def richiesta(self, addr, testo):
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
        elif p[0] == "V" and len(p) >= 3:
            self.visti[p[1]] = int(p[2])
        elif p[0] == "FILE" and len(p) >= 3 and p[2] == "in":
            self.manda("CLEAR %s" % p[1])          # la rubrica non accetta allegati


if __name__ == "__main__":
    Rubrica(sys.argv[1], int(sys.argv[2])).gira()
