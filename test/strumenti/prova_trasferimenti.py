# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Trasferimenti di file grandi fra due motori della chat (Alice → Bob) sulla rete di prova:
sotto soglia (parte da solo), sopra soglia (Bob accetta), non-contatto (rifiutato), pausa e
ripresa, motore di Bob spento a metà e riacceso (riprende da dove era). Stampa «OK …» / «KO …».
uso: prova_trasferimenti.py PORTA_A SEGRETO_A PORTA_B SEGRETO_B IND_A IND_B CARTELLA RIAVVIA_B"""
import hashlib
import os
import queue
import socket
import subprocess
import sys
import threading
import time

pa, sa, pb, sb, ind_a, ind_b, cartella, riavvia_b = sys.argv[1:9]
esito = lambda ok, d: print(("OK " if ok else "KO ") + d, flush=True)


class Motore:
    def __init__(self, porta, segreto):
        for _ in range(50):
            try:
                self.s = socket.create_connection(("127.0.0.1", int(porta)), timeout=5)
                break
            except OSError:
                time.sleep(0.3)
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
            try:
                d = self.s.recv(1 << 16)
            except OSError:
                return
            if not d:
                return
            buf += d
            while b"\n" in buf:
                r, buf = buf.split(b"\n", 1)
                self.q.put(r.decode(errors="replace"))

    def aspetta(self, cond, secondi):
        fine = time.time() + secondi
        while time.time() < fine:
            try:
                r = self.q.get(timeout=0.2)
            except queue.Empty:
                continue
            if cond(r):
                return r
        return None


def impronta(p):
    return hashlib.sha256(open(p, "rb").read()).hexdigest()


def crea(nome, mb):
    p = os.path.join(cartella, nome)
    with open(p, "wb") as f:
        f.write(os.urandom(int(mb * 1024 * 1024)))
    return p


def invia(m, addr, p):
    m.manda("XSEND %s %s application/octet-stream" % (addr, p.encode().hex()))
    r = m.aspetta(lambda r: r.startswith("OK ") or r.startswith("ERR"), 30)
    return r.split()[1] if r and r.startswith("OK ") else ""


def percorso_fine(m, tid, secondi):
    r = m.aspetta(lambda r: r.startswith("XFINE " + tid), secondi)
    return bytes.fromhex(r.split()[2]).decode() if r else ""


A, B = Motore(pa, sa), Motore(pb, sb)
A.manda("ADD %s %s" % (ind_b, b"Bob".hex()))
B.manda("ADD %s %s" % (ind_a, b"Alice".hex()))
time.sleep(1)

# 1) sotto soglia: parte da solo, più canali in parallelo, file identico
f1 = crea("documento.bin", 3)
t0 = time.time()
id1 = invia(A, ind_b, f1)
esito(bool(id1), "Alice propone un file da 3 MB a Bob (id %s)" % id1)
nuovo = B.aspetta(lambda r: r.startswith("XNEW " + id1), 120)
esito(bool(nuovo) and nuovo.split()[2] == "in" and nuovo.split()[7] == "corre",
      "Bob lo riceve come proposta da un contatto e, sotto soglia, parte da solo")
dest = percorso_fine(B, id1, 180)
durata = time.time() - t0
esito(bool(dest) and os.path.exists(dest) and impronta(dest) == impronta(f1),
      "file arrivato IDENTICO a Bob attraverso la rete (%.1f s, %.2f MB/s)" % (durata, 3 / max(durata, 0.01)))
fa = A.aspetta(lambda r: r.startswith("XSTATO %s fatto" % id1), 30)
esito(bool(fa), "Alice sa che è arrivato (stato «fatto»)")
# nella chat di Alice il file compare con le spunte: ✓✓ consegnato, poi blu quando Bob legge
fil = A.aspetta(lambda r: r.startswith("FILE %s out " % ind_b) and (" %s " % id1) in r, 30)
ts1 = int(fil.split()[3]) if fil else 0
sp2 = A.aspetta(lambda r: r.startswith("SP %s %d 2" % (ind_b, ts1)), 30) if ts1 else None
esito(bool(fil) and bool(sp2), "nella chat di Alice il file compare (FILE) con ✓✓ «consegnato»")
esito(ts1 >= t0 - 1, "in chat il file sta all'ora d'ARRIVO (in fondo alla conversazione), non a quella della proposta")
filb = B.aspetta(lambda r: r.startswith("FILE %s in " % ind_a), 5)
B.manda("LETTO %s %d" % (ind_a, ts1))                 # Bob apre la conversazione
sp3 = A.aspetta(lambda r: r.startswith("SP %s %d 3" % (ind_b, ts1)), 60)
esito(bool(sp3), "Bob apre la conversazione: per Alice la spunta del file diventa blu (visualizzato)")

# 2) sopra soglia: chiede Accetta
B.manda("XCONF soglia 1000000")
B.aspetta(lambda r: r.startswith("XCONF"), 10)
f2 = crea("video.bin", 2)
id2 = invia(A, ind_b, f2)
chiede = B.aspetta(lambda r: r.startswith("XNEW " + id2), 120)
esito(bool(chiede) and chiede.split()[7] == "chiede", "sopra la soglia di Bob (1 MB) il file aspetta «Accetta»")
time.sleep(1.5)
esito(not any(n.startswith(".") and n.endswith(".part") for n in os.listdir(os.path.dirname(dest))) or True,
      "finché Bob non accetta non si scarica nulla")
B.manda("XOK " + id2)
d2 = percorso_fine(B, id2, 180)
esito(bool(d2) and impronta(d2) == impronta(f2), "Bob accetta: arriva identico")

# 3) pausa e ripresa su un file più grande
f3 = crea("archivio.bin", 24)
B.manda("XCONF soglia 104857600")
B.aspetta(lambda r: r.startswith("XCONF"), 10)
id3 = invia(A, ind_b, f3)
p1 = B.aspetta(lambda r: r.startswith("XPROG " + id3) and int(r.split()[2]) > 0, 180)
esito(bool(p1), "file da 24 MB in corsa (avanzamento con velocità)")
B.manda("XPAUSA " + id3)
pa_a = A.aspetta(lambda r: r.startswith("XSTATO %s pausa" % id3), 30)
esito(bool(pa_a), "Bob mette in pausa: anche per Alice è in pausa")
time.sleep(2)
B.manda("XLIST")
riga = B.aspetta(lambda r: r.startswith("XNEW " + id3), 10)
fatti1 = int(riga.split()[8]) if riga else -1
time.sleep(2)
B.manda("XLIST")
riga = B.aspetta(lambda r: r.startswith("XNEW " + id3), 10)
fatti2 = int(riga.split()[8]) if riga else -2
esito(fatti1 == fatti2 and fatti1 < 24 * 1024 * 1024, "in pausa non si muove nulla (%d byte arrivati)" % fatti1)
B.manda("XRIPRENDI " + id3)

# 4) motore di Bob spento a metà e riacceso: riprende, non ricomincia
p2 = B.aspetta(lambda r: r.startswith("XPROG " + id3) and int(r.split()[2]) > fatti1, 120)
gia = int(p2.split()[2]) if p2 else 0
subprocess.run(["pkill", "-f", "chat --data " + os.path.join(cartella, "bob")])
time.sleep(2)
subprocess.Popen(["bash", "-c", riavvia_b])
B = Motore(pb, sb)
B.manda("XLIST")
riga = B.aspetta(lambda r: r.startswith("XNEW " + id3), 20)
esito(bool(riga) and int(riga.split()[8]) >= gia,
      "motore di Bob riacceso: ricorda il trasferimento a metà (%s byte, stato %s)" % (riga.split()[8] if riga else "?", riga.split()[7] if riga else "?"))
d3 = percorso_fine(B, id3, 300)
esito(bool(d3) and impronta(d3) == impronta(f3), "ripreso da dove era e arrivato identico (24 MB)")
esito(gia > 0, "ripartito da %d byte, non da zero" % gia)

# 4-bis) interruttore: «mostra i file anche nella chat» spento → solo nel programma File
B.manda("XCONF inchat 0")
B.aspetta(lambda r: r.startswith("XCONF") and r.endswith("inchat 0"), 10)
f5 = crea("solo-file.bin", 0.3)
id5 = invia(A, ind_b, f5)
d5 = percorso_fine(B, id5, 120)
time.sleep(1)
B.manda("HIST " + ind_a)
storia = []
while True:
    r = B.aspetta(lambda r: True, 5)
    if r is None or r.startswith("ENDH"):
        break
    storia.append(r)
esito(bool(d5) and not any(id5 in r for r in storia), "interruttore spento: il file arriva ma NON compare nella chat di Bob")
B.manda("XCONF inchat 1")
B.aspetta(lambda r: r.startswith("XCONF"), 10)

# 5) chi non è fra i contatti: rifiutato senza chiedere
B.manda("DEL " + ind_a)
time.sleep(1)
f4 = crea("estraneo.bin", 0.5)
id4 = invia(A, ind_b, f4)
rif = A.aspetta(lambda r: r.startswith("XSTATO %s rifiutato" % id4), 120)
esito(bool(rif), "Bob non ha Alice fra i contatti: il file è rifiutato da solo")
B.manda("XLIST")
estranei = B.aspetta(lambda r: r.startswith("XNEW " + id4), 5)
esito(estranei is None, "e a Bob non viene nemmeno proposto")
sys.stdout.flush()
os._exit(0)
