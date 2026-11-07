#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Solo per le simulazioni: avvia rubrica.py come al solito, ma ogni 180 s stampa dove si trova
ciascun thread (per scoprire blocchi). uso: come rubrica.py"""
import faulthandler
import sys
from pathlib import Path

faulthandler.dump_traceback_later(180, repeat=True, file=sys.stderr)
sys.argv[0] = str(Path(__file__).resolve().parents[1] / "pc" / "gestore-rete" / "rubrica.py")
sys.path.insert(0, str(Path(sys.argv[0]).parent))
import time  # noqa: E402
import rubrica  # noqa: E402

orig_evento, orig_manda = rubrica.Rubrica.evento, rubrica.Rubrica.manda
def _testo(riga):
    p = riga.split(" ")
    if p[0] == "MSG" and len(p) >= 4:
        try:
            return "MSG %s %s" % (p[1][:8], bytes.fromhex(p[3]).decode(errors="replace")[:600] if "RICEVUTI" in bytes.fromhex(p[3]).decode(errors="replace")[:20] else bytes.fromhex(p[3]).decode(errors="replace")[:60])
        except ValueError:
            pass
    return riga[:90]


rubrica.Rubrica.evento = lambda self, riga: (print("%.1f <- %s" % (time.monotonic(), _testo(riga)), file=sys.stderr, flush=True), orig_evento(self, riga))[1]
rubrica.Rubrica.manda = lambda self, riga: (print("%.1f -> %s (%d)" % (time.monotonic(), riga[:60], len(riga)), file=sys.stderr, flush=True), orig_manda(self, riga))[1]
r = rubrica.Rubrica(sys.argv[1], int(sys.argv[2]))
if len(sys.argv) > 3:
    rubrica.servi_casella(r, int(sys.argv[3]))
r.gira()
