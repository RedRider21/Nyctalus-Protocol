# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Prova una chiamata fra due motori della chat (porte di controllo di A e B):
invito, risposta, 2 secondi di «audio» (pacchetti da 20 ms) nei due sensi, fine; poi un
invito rifiutato. Stampa righe «OK descrizione» / «KO descrizione».
uso: prova_chiamata.py PORTA_A SEGRETO_A PORTA_B SEGRETO_B INDIRIZZO_A INDIRIZZO_B"""
import os
import queue
import socket
import sys
import threading
import time

pa, sa, pb, sb, ind_a, ind_b = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4], sys.argv[5], sys.argv[6]
esito = lambda ok, d: print(("OK " if ok else "KO ") + d, flush=True)


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
            d = self.s.recv(65536)
            if not d:
                return
            buf += d
            while b"\n" in buf:
                r, buf = buf.split(b"\n", 1)
                self.q.put((time.time(), r.decode()))

    def aspetta(self, prefisso, secondi):
        fine = time.time() + secondi
        while time.time() < fine:
            try:
                t, r = self.q.get(timeout=0.2)
            except queue.Empty:
                continue
            if r.startswith(prefisso):
                return r
        return None


A, B = Motore(pa, sa), Motore(pb, sb)
time.sleep(1)
A.manda("CALL " + ind_b)
r = A.aspetta("CALLID " + ind_b, 120)
cid = r.split()[2] if r else ""
esito(bool(cid), "A chiama B: invito partito (id %s)" % cid)
r = B.aspetta("CALLIN " + ind_a, 60)
esito(r == "CALLIN %s %s" % (ind_a, cid), "B riceve l'invito di A")
B.manda("CALLOK %s %s" % (ind_a, cid))
r = A.aspetta("CALLOK " + ind_b, 60)
esito(r == "CALLOK %s %s" % (ind_b, cid), "B accetta: A lo sa")

# 2 secondi di audio nei due sensi: 100 pacchetti da 20 ms ciascuno
pacchetti = [os.urandom(60) for _ in range(100)]
partenza = {}
def parla(chi, verso):
    for i, p in enumerate(pacchetti, 1):
        partenza[(chi, i)] = time.time()
        chi.manda("AUD %s %d %s" % (verso, i, p.hex()))
        time.sleep(0.02)
threading.Thread(target=parla, args=(A, ind_b)).start()
threading.Thread(target=parla, args=(B, ind_a)).start()
def raccogli(m, da, chi_parla):
    presi, ritardi = {}, []
    fine = time.time() + 15
    while time.time() < fine and len(presi) < 100:
        try:
            t, r = m.q.get(timeout=0.3)
        except queue.Empty:
            continue
        p = r.split(" ")
        if p[0] == "AUD" and p[1] == da:
            seq = int(p[2])
            presi[seq] = bytes.fromhex(p[3])
            ritardi.append(t - partenza.get((chi_parla, seq), t))
    return presi, ritardi
ris_b, rit_b = raccogli(B, ind_a, A)
ris_a, rit_a = raccogli(A, ind_b, B)
giusti_b = sum(1 for i, p in enumerate(pacchetti, 1) if ris_b.get(i) == p)
giusti_a = sum(1 for i, p in enumerate(pacchetti, 1) if ris_a.get(i) == p)
mediano = lambda v: sorted(v)[len(v) // 2] if v else 99
esito(giusti_b >= 95, "audio A → B: %d/100 pacchetti identici, ritardo mediano %d ms" % (giusti_b, mediano(rit_b) * 1000))
esito(giusti_a >= 95, "audio B → A: %d/100 pacchetti identici, ritardo mediano %d ms" % (giusti_a, mediano(rit_a) * 1000))
A.manda("CALLEND %s %s" % (ind_b, cid))
r = B.aspetta("CALLEND " + ind_a, 30)
esito(r == "CALLEND %s %s" % (ind_a, cid), "A riaggancia: B lo sa")

# invito rifiutato
A.manda("CALL " + ind_b)
r = A.aspetta("CALLID", 60)
cid2 = r.split()[2] if r else ""
B.aspetta("CALLIN", 30)
B.manda("CALLNO %s %s" % (ind_a, cid2))
r = A.aspetta("CALLNO " + ind_b, 30)
esito(bool(cid2) and r == "CALLNO %s %s" % (ind_b, cid2) and cid2 != cid, "invito rifiutato: A lo sa (id nuovo per ogni chiamata)")
B.manda("CALLOK %s ../../x" % ind_a)
r = B.aspetta("ERR", 10)
esito(bool(r), "id di chiamata non valido rifiutato")
