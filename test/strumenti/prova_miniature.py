# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Miniature e galleria del telefono: prima la miniatura EXIF già nella foto (istantanea),
poi Pillow, poi ffmpeg; mai la foto intera se grande; elenco di MIGLIAIA di foto in
tempi brevi e tenuto in memoria. Uso: prova_miniature.py CARTELLA_TEMP → «OK …» / «KO …»"""
import io
import os
import shutil
import struct
import sys
import time
from pathlib import Path

d = Path(sys.argv[1]); d.mkdir(parents=True, exist_ok=True); os.environ["HOME"] = str(d)
g = d / "galleria"; g.mkdir()
os.environ["NYCTALUS_GALLERIA"] = str(g); os.environ["NYCTALUS_GALLERIA_SOLO"] = "1"
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "mobile" / "orchestratore"))
import nyctalus_mobile as m          # noqa: E402

esito = lambda ok, testo: print(("OK " if ok else "KO ") + testo)
try:
    from PIL import Image
except ImportError:
    print("SALTA Pillow assente sul PC: non si possono creare le foto di prova"); sys.exit(0)


def foto_con_exif(percorso, orient, lato=(4000, 3000)):
    """JPEG grande con miniatura EXIF 160x120 (IFD1) e orientamento, come le fotocamere."""
    b = io.BytesIO(); Image.new("RGB", (160, 120), (0, 200, 0)).save(b, "JPEG"); mini = b.getvalue()
    ifd0 = struct.pack(">H", 1) + struct.pack(">HHIHH", 0x0112, 3, 1, orient, 0) + struct.pack(">I", 26)
    ifd1 = struct.pack(">H", 2) + struct.pack(">HHII", 0x0201, 4, 1, 26 + 30) \
        + struct.pack(">HHII", 0x0202, 4, 1, len(mini)) + struct.pack(">I", 0)
    app1 = b"Exif\x00\x00" + b"MM\x00\x2a" + struct.pack(">I", 8) + ifd0 + ifd1 + mini
    b = io.BytesIO(); Image.new("RGB", lato, (200, 30, 30)).save(b, "JPEG", quality=95); grande = b.getvalue()
    Path(percorso).write_bytes(grande[:2] + b"\xff\xe1" + struct.pack(">H", len(app1) + 2) + app1 + grande[2:])
    return mini


# 1) foto della fotocamera: la miniatura EXIF, subito, con l'orientamento
f = d / "IMG_ruotata.jpg"; mini = foto_con_exif(f, 6)
t0 = time.time(); r = m.miniatura(f); dt = time.time() - t0
dati = r.read_bytes() if r else b""
esito(r is not None and r != f and dati.endswith(mini[2:]) and len(dati) < 20000,
      "foto della fotocamera: miniatura già dentro la foto (%d byte, %.1f ms)" % (len(dati), dt * 1000))
esito(b"Exif\x00\x00MM" in dati and struct.pack(">HHIHH", 0x0112, 3, 1, 6, 0) in dati,
      "la miniatura porta l'orientamento della foto (il browser la gira)")
# 2) foto senza miniatura EXIF: Pillow la rimpicciolisce e la raddrizza
s = d / "senza_exif.jpg"; Image.frombytes("RGB", (1500, 1000), os.urandom(1500 * 1000 * 3)).save(s, "JPEG", quality=95)
r = m.miniatura(s)
with Image.open(r) as im:
    esito(r != s and max(im.size) <= m.LATO_MINIATURA, "senza miniatura EXIF: Pillow la riduce a %dx%d" % im.size)
# 3) niente Pillow né ffmpeg (Termux appena installato): mai la foto intera se grande
sys.modules["PIL"] = None
shutil.rmtree(m.CACHE, ignore_errors=True)
vero_which = shutil.which
m.shutil.which = lambda c: None if c == "ffmpeg" else vero_which(c)
esito(m.miniatura(s) is None, "senza Pillow né ffmpeg: la foto grande non viene mandata intera (%d KB)" % (s.stat().st_size // 1024))
p = d / "piccola.png"; Image.new("RGB", (64, 64)).save(p)
esito(m.miniatura(p) == p, "immagine già piccola: va bene così com'è")
(d / "rotta.jpg").write_bytes(b"\xff\xd8\xff\xe1\x00\x10Exif\x00\x00MM\x00\x2a\xff\xff")
try:
    m.miniatura(d / "rotta.jpg")
    esito(True, "JPEG rovinato: nessun errore")
except Exception as e:          # noqa: BLE001
    esito(False, "JPEG rovinato fa cadere la miniatura: %r" % e)

# 4) galleria enorme: 2000 foto in 20 cartelle
for c in range(20):
    cd = g / ("DCIM-%02d" % c); cd.mkdir()
    for k in range(100):
        (cd / ("IMG_%04d.jpg" % k)).write_bytes(b"\xff\xd8\xff\xd9")
t0 = time.time(); v = m.elenco_galleria_recente(); prima = time.time() - t0
t0 = time.time(); m.elenco_galleria_recente(); seconda = time.time() - t0
esito(len(v) == m.GALLERIA_MAX and prima < 3, "2000 foto: elenco dei %d più recenti in %.2f s" % (len(v), prima))
esito(seconda < 0.01, "riaprendo la galleria l'elenco è già in memoria (%.1f ms)" % (seconda * 1000))
