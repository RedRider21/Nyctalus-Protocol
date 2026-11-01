# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Scenario della prova del traffico: Alice e Bob (due apparecchi) si scambiano un messaggio,
un file e una videochiamata (voce + immagini). Ogni contenuto porta una PAROLA SPIA, che poi
si cerca nella cattura dei pacchetti. Scrive in FASI i momenti di inizio/fine di ogni fase
(per l'analisi di tempi e dimensioni). Stampa «OK …» / «KO …».
uso: prova_traffico.py PORTA_A SEGRETO_A PORTA_B SEGRETO_B IND_A IND_B CARTELLA FASI"""
import os
import queue
import socket
import sys
import threading
import time

pa, sa, pb, sb, ind_a, ind_b, cartella, fasi = sys.argv[1:9]
esito = lambda ok, d: print(("OK " if ok else "KO ") + d, flush=True)
SPIA = {
    "testo": b"SPIA-TESTO-Kq7Zx",
    "file": b"SPIA-FILE-Wm3Rt",
    "voce": b"SPIA-VOCE-Hj5Pd",
    "video": b"SPIA-VIDEO-Ny8Lc",
}
registro = open(fasi, "w")


def fase(nome, quando):
    registro.write("%s %s %.6f\n" % (nome, quando, time.time()))
    registro.flush()


class Motore:
    def __init__(self, porta, segreto):
        self.s = socket.create_connection(("127.0.0.1", int(porta)), timeout=5)
        self.s.settimeout(None)
        self.q = queue.Queue()
        self.manda("AUTH " + segreto)
        self.manda("SUB")
        threading.Thread(target=self._leggi, daemon=True).start()

    def manda(self, riga):
        self.s.sendall((riga + "\n").encode())

    def _leggi(self):
        buf = b""
        while True:
            d = self.s.recv(1 << 20)
            if not d:
                return
            buf += d
            while b"\n" in buf:
                r, buf = buf.split(b"\n", 1)
                self.q.put(r.decode(errors="replace"))

    def aspetta(self, prefisso, secondi):
        fine = time.time() + secondi
        while time.time() < fine:
            try:
                r = self.q.get(timeout=0.2)
            except queue.Empty:
                continue
            if r.startswith(prefisso):
                return r
        return None

    def conta(self, prefisso, secondi):
        n, fine = 0, time.time() + secondi
        while time.time() < fine:
            try:
                r = self.q.get(timeout=0.2)
            except queue.Empty:
                continue
            if r.startswith(prefisso):
                n += 1
        return n


A, B = Motore(pa, sa), Motore(pb, sb)
fase("riposo", "inizio"); time.sleep(6); fase("riposo", "fine")

# 1) messaggio
fase("testo", "inizio")
testo = SPIA["testo"] + " ciao Bob, questo è un segreto".encode()
A.manda("SEND %s %s" % (ind_b, testo.hex()))
r = B.aspetta("MSG " + ind_a, 90)
esito(bool(r) and bytes.fromhex(r.split()[3]) == testo, "messaggio con parola spia consegnato a Bob")
time.sleep(2); fase("testo", "fine")

# 2) file (60 KB: parola spia ripetuta + testo leggibile)
fase("file", "inizio")
f = os.path.join(cartella, "documento-segreto.txt")
with open(f, "wb") as h:
    h.write((SPIA["file"] + b" riga di un documento riservato\n") * 1800)
A.manda("SENDFILE %s %s text/plain" % (ind_b, f.encode().hex()))
r = B.aspetta("FILE %s in" % ind_a, 120)
arrivato = b""
if r:
    percorso = bytes.fromhex(r.split()[8]).decode()
    for _ in range(100):                              # il file si completa poco dopo l'annuncio
        try:
            arrivato = open(percorso, "rb").read()
        except OSError:
            arrivato = b""
        if len(arrivato) >= os.path.getsize(f):
            break
        time.sleep(0.2)
esito(arrivato == open(f, "rb").read(), "file con parola spia arrivato a Bob identico (%d byte)" % len(arrivato))
time.sleep(3); fase("file", "fine")

# 3) videochiamata: invito video, voce 20 ms per 6 s, immagini per 6 s
fase("chiamata", "inizio")
A.manda("CALL %s v" % ind_b)
r = A.aspetta("CALLID " + ind_b, 60)
cid = r.split()[2] if r else ""
r = B.aspetta("CALLIN " + ind_a, 60)
B.manda("CALLOK %s %s" % (ind_a, cid))
esito(bool(A.aspetta("CALLOK " + ind_b, 60)), "videochiamata accettata")
voce = (SPIA["voce"] * 4)[:60]
immagine = (SPIA["video"] + b" fotogramma ") * 160      # ~4,5 KB, come un'immagine VP8 piccola


def parla(m, verso):
    for i in range(1, 301):
        m.manda("AUD %s %d %s" % (verso, i, voce.hex()))
        time.sleep(0.02)


def filma(m, verso):
    m.manda("VIDSTATO %s 1" % verso)
    for i in range(1, 91):
        m.manda("VID %s %d %d %s" % (verso, i, 1 if i % 30 == 1 else 0, immagine.hex()))
        time.sleep(1 / 15)


ricevuti = {}
def ascolta(m, nome):
    ricevuti[nome] = m.conta("AUD ", 9) if nome.endswith("voce") else m.conta("VID ", 9)


B2, A2 = Motore(pb, sb), Motore(pa, sa)          # interfacce separate per contare voce e video
fili = [threading.Thread(target=parla, args=(A, ind_b)), threading.Thread(target=parla, args=(B, ind_a)),
        threading.Thread(target=filma, args=(A, ind_b)), threading.Thread(target=filma, args=(B, ind_a)),
        threading.Thread(target=ascolta, args=(B, "B voce")), threading.Thread(target=ascolta, args=(B2, "B video"))]
for t in fili:
    t.start()
for t in fili:
    t.join()
esito(ricevuti.get("B voce", 0) > 200, "voce con parola spia arrivata a Bob (%d pacchetti)" % ricevuti.get("B voce", 0))
esito(ricevuti.get("B video", 0) > 50, "immagini con parola spia arrivate a Bob (%d)" % ricevuti.get("B video", 0))
A.manda("CALLEND %s %s" % (ind_b, cid))
time.sleep(1); fase("chiamata", "fine")
fase("dopo", "inizio"); time.sleep(4); fase("dopo", "fine")
for k, v in SPIA.items():
    registro.write("spia %s %s\n" % (k, v.decode()))
registro.close()
