# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Miniature della galleria del telefono: con Pillow, SENZA Pillow e senza ffmpeg
(miniatura EXIF già nella foto, con l'orientamento), e mai la foto intera se grande.
Uso: prova_miniature.py CARTELLA_TEMP   → righe «OK …» / «KO …»"""
import io
import os
import shutil
import struct
import sys
from pathlib import Path

d = Path(sys.argv[1]); d.mkdir(parents=True, exist_ok=True); os.environ["HOME"] = str(d)
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
    tiff = b"MM\x00\x2a" + struct.pack(">I", 8) + ifd0 + ifd1 + mini
    app1 = b"Exif\x00\x00" + tiff
    b = io.BytesIO(); Image.new("RGB", lato, (200, 30, 30)).save(b, "JPEG", quality=95); grande = b.getvalue()
    Path(percorso).write_bytes(grande[:2] + b"\xff\xe1" + struct.pack(">H", len(app1) + 2) + app1 + grande[2:])
    return mini


f = d / "IMG_ruotata.jpg"; mini = foto_con_exif(f, 6)
# 1) con Pillow: miniatura piccola e già raddrizzata (4000x3000 ruotata di 90° → verticale)
r = m.miniatura(f)
with Image.open(r) as im:
    esito(r != f and max(im.size) <= m.LATO_MINIATURA and im.size[1] > im.size[0],
          "con Pillow: miniatura %dx%d, raddrizzata, %d byte invece di %d" % (im.size + (r.stat().st_size, f.stat().st_size)))
# 2) senza Pillow e senza ffmpeg (come un Termux appena installato)
sys.modules["PIL"] = None
shutil.rmtree(m.CACHE, ignore_errors=True)
vero_which = shutil.which
m.shutil.which = lambda c: None if c == "ffmpeg" else vero_which(c)
r = m.miniatura(f)
dati = r.read_bytes() if r else b""
esito(r is not None and r != f and dati.endswith(mini[2:]) and len(dati) < 20000,
      "senza Pillow né ffmpeg: usata la miniatura EXIF già nella foto (%d byte)" % len(dati))
esito(b"Exif\x00\x00MM" in dati and struct.pack(">HHIHH", 0x0112, 3, 1, 6, 0) in dati,
      "la miniatura EXIF porta l'orientamento della foto (il browser la gira)")
# 3) foto grande senza miniatura EXIF: niente foto intera (meglio un'icona)
g = d / "senza_exif.jpg"; Image.frombytes("RGB", (1500, 1000), os.urandom(1500 * 1000 * 3)).save(g, "JPEG", quality=95)
esito(m.miniatura(g) is None, "foto grande senza miniatura: non si manda la foto intera (%d KB)" % (g.stat().st_size // 1024))
p = d / "piccola.png"; Image.new("RGB", (64, 64)).save(p)
esito(m.miniatura(p) == p, "immagine già piccola: va bene così com'è")
# 4) file rovinati non fanno cadere niente
(d / "rotta.jpg").write_bytes(b"\xff\xd8\xff\xe1\x00\x10Exif\x00\x00MM\x00\x2a\xff\xff")
try:
    m.miniatura(d / "rotta.jpg")
    esito(True, "JPEG rovinato: nessun errore")
except Exception as e:          # noqa: BLE001
    esito(False, "JPEG rovinato fa cadere la miniatura: %r" % e)
