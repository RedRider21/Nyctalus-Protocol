#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Dati DIMOSTRATIVI per gli screenshot: una HOME finta con identità usa e getta,
contatti inventati, conversazioni e foto generate. Nessun dato reale.
Uso: dati_dimostrativi.py HOME_FINTA BINARIO_NYCTALUS   → stampa il Contact ID."""
import json
import math
import os
import random
import re
import secrets
import subprocess
import sys
import time
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

HOME, BIN = Path(sys.argv[1]), sys.argv[2]
DATI = HOME / ".nyctalus-chat"
random.seed(7)


def identita(cartella):
    """Un Contact ID vero (chiave generata dal motore), in una cartella usa e getta."""
    cartella.mkdir(parents=True, exist_ok=True)
    porte = [random.randint(21000, 29000) for _ in range(3)]
    p = subprocess.Popen([BIN, "chat", "--data", str(cartella), "--backend", "127.0.0.1:%d" % porte[0],
                          "--control", "127.0.0.1:%d" % porte[1], "--socks", "127.0.0.1:%d" % porte[2]],
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    addr = None
    for riga in p.stdout:
        m = re.search(r"[a-z2-7]{56}\.nyct", riga)
        if m:
            addr = m.group(0)
            break
    p.terminate()
    p.wait()
    return addr


def paesaggio(percorso, w, h, cielo, terra, sole=True):
    """Una foto «di montagna al tramonto», disegnata."""
    im = Image.new("RGB", (w, h))
    d = ImageDraw.Draw(im)
    for y in range(h):
        t = y / h
        d.line([(0, y), (w, y)], fill=tuple(int(cielo[0][i] * (1 - t) + cielo[1][i] * t) for i in range(3)))
    if sole:
        r = h // 9
        d.ellipse([w * 0.66 - r, h * 0.42 - r, w * 0.66 + r, h * 0.42 + r], fill=(255, 226, 160))
    for k, (base, col) in enumerate(((0.55, terra[0]), (0.66, terra[1]), (0.78, terra[2]))):
        punti = [(0, h)]
        for x in range(0, w + 40, 40):
            punti.append((x, h * base + math.sin(x / (90 + 40 * k) + k) * h * 0.06 + random.uniform(-8, 8)))
        punti.append((w, h))
        d.polygon(punti, fill=col)
    im.filter(ImageFilter.GaussianBlur(0.6)).save(percorso, quality=88)


def main():
    me = identita(DATI)
    contatti = []
    for nome in ("Alice", "Bruno", "Chiara", "Davide", "Elena"):
        contatti.append((nome, identita(HOME / "altri" / nome)))
    (DATI / "contatti").write_text("".join("%s %s\n" % (a, n.encode().hex()) for n, a in contatti))

    ora = int(time.time())
    galleria = HOME / "galleria" / "Camera"
    galleria.mkdir(parents=True, exist_ok=True)
    foto = []
    temi = [(((40, 60, 120), (250, 160, 110)), ((70, 70, 110), (50, 55, 90), (30, 35, 60))),
            (((20, 120, 190), (180, 220, 240)), ((60, 120, 80), (40, 95, 60), (25, 70, 45))),
            (((90, 40, 110), (240, 120, 90)), ((80, 50, 90), (55, 35, 70), (30, 22, 45))),
            (((10, 20, 50), (60, 80, 140)), ((30, 40, 70), (20, 28, 52), (12, 16, 32))),
            (((200, 120, 60), (250, 210, 140)), ((140, 90, 60), (110, 70, 45), (80, 50, 30))),
            (((60, 140, 200), (230, 240, 250)), ((200, 210, 220), (150, 165, 180), (100, 115, 130)))]
    for i, (cielo, terra) in enumerate(temi * 3):
        f = galleria / ("IMG_2026%04d.jpg" % (1001 + i))
        paesaggio(f, 1200, 900, cielo, terra, sole=i % 2 == 0)
        os.utime(f, (ora - i * 3600, ora - i * 3600))
        foto.append(f)

    def riga_testo(direzione, ts, testo):
        return "%s %d %s\n" % (direzione, ts, testo.encode().hex())

    def riga_foto(direzione, ts, sorgente, chi):
        cart = DATI / "allegati" / chi[:16]
        cart.mkdir(parents=True, exist_ok=True)
        dest = cart / sorgente.name
        dest.write_bytes(sorgente.read_bytes())
        return "%s %d %s %d image/jpeg %s %s\n" % ("fin" if direzione == "in" else "fout", ts, secrets.token_hex(8),
                                                    dest.stat().st_size, sorgente.name.encode().hex(),
                                                    str(dest).encode().hex())

    conversazioni = {
        "Alice": [("in", 95, "Ciao! Hai provato la nuova versione?"),
                  ("out", 93, "Sì, ora funziona anche sul telefono 🦇"),
                  ("in", 90, "Grandioso. Ti mando la foto di ieri"),
                  ("in", 89, foto[0]),
                  ("out", 60, "Bellissima! Che posto è?"),
                  ("in", 58, "Sopra Piano Battaglia, al tramonto"),
                  ("out", 20, foto[1]),
                  ("out", 19, "E questa è di stamattina"),
                  ("in", 5, "Viaggia tutto cifrato e senza server, vero?"),
                  ("out", 3, "Esatto: nessun server, nessun metadato. Solo noi due.")],
        "Bruno": [("out", 600, "Ci vediamo domani alle 9?"), ("in", 590, "Perfetto, a domani 👍")],
        "Chiara": [("in", 1500, "Ti ho mandato il documento"), ("out", 1490, "Ricevuto, grazie!")],
        "Davide": [("in", 3000, "Il nodo sul server è attivo")],
        "Elena": [("out", 5000, "Buon viaggio!")],
    }
    cron = DATI / "cronologia"
    cron.mkdir(parents=True, exist_ok=True)
    for nome, addr in contatti:
        righe = ""
        for direzione, minuti, cosa in conversazioni[nome]:
            ts = ora - minuti * 60
            righe += riga_foto(direzione, ts, cosa, addr) if isinstance(cosa, Path) else riga_testo(direzione, ts, cosa)
        (cron / ("%s.log" % addr)).write_text(righe)
    (DATI / "ui.json").write_text(json.dumps({"tema": "notte", "sfondo": "pipistrelli", "larghezza": 1040,
                                              "altezza": 700, "non_letti": {contatti[1][1]: 1}}))
    json.dump({"me": me, "contatti": contatti}, open(HOME / "demo.json", "w"))
    print(me)


if __name__ == "__main__":
    main()
