# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""«Info» del messaggio nella chat del PC: dalle spunte del motore (con l'ora) si ricava quando
un nostro messaggio è stato messo in casella, consegnato e visualizzato. Finto motore sulla porta
di controllo. Stampa «OK …» / «KO …»."""
import os
import socket
import sys
import threading
import time

import gi
gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk  # noqa: E402
from nyctalus_chat import app as chatapp  # noqa: E402

esito = lambda ok, d: print(("OK " if ok else "KO ") + d, flush=True)
BOB = "b" * 56 + ".nyct"


def finto_motore(porta):
    srv = socket.socket()
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", porta))
    srv.listen(8)

    def cliente(c):
        buf = b""
        while True:
            d = c.recv(4096)
            if not d:
                return
            buf += d
            while b"\n" in buf:
                riga, buf = buf.split(b"\n", 1)
                if riga.startswith(b"WHOAMI"):
                    c.sendall(b"ME " + b"a" * 56 + b".nyct\n")
    threading.Thread(target=lambda: [threading.Thread(target=cliente, args=(srv.accept()[0],), daemon=True).start()
                                     for _ in iter(int, 1)], daemon=True).start()


finto_motore(int(os.environ["NYCTALUS_CHAT_PORT"]))
chat = chatapp.Chat()


def passo():
    ora = int(time.time())
    m1, m2, m3 = ora - 600, ora - 300, ora - 60
    chat.attivo = BOB
    for riga in ("CAS %s %d %d" % (BOB, m1, ora - 590), "SP %s %d 2 %d" % (BOB, m1, ora - 500),
                 "SP %s %d 2 %d" % (BOB, m2, ora - 290), "SP %s %d 3 %d" % (BOB, m2, ora - 200),
                 "SP %s %d 2" % (BOB, m3)):                                # riga senza ora (motore vecchio)
        chat._evento(riga) if hasattr(chat, "_evento") else None
    r1, st1 = chat._righe_info(m1)
    esito([x[1] for x in r1] == ["Inviato", "Nella casella cifrata", "Consegnato", "Visualizzato"]
          and r1[1][2].startswith("oggi alle") and r1[2][2].startswith("oggi alle") and r1[3][3] and st1 == "3",
          "messaggio passato dalla casella: in casella, consegnato e visualizzato con l'ora (%s)" % [x[2] for x in r1])
    r2, _ = chat._righe_info(m2)
    esito(len(r2) == 3 and r2[2][3] and r2[2][2].startswith("oggi alle"), "messaggio letto: «Visualizzato» con l'ora della lettura")
    r3, st3 = chat._righe_info(m3)
    esito(st3 == "2" and r3[1][3] and r3[1][2] == "" and not r3[2][3],
          "riga senza ora (motore vecchio): consegnato «ora non disponibile», non ancora visualizzato")
    esito(chatapp.quando_leggibile(1700000000).startswith("14 nov 2023 alle"), "date lontane con giorno, mese e anno")
    sys.stdout.flush()
    os._exit(0)


GLib.timeout_add(1500, passo)
GLib.timeout_add(30000, lambda: (esito(False, "tempo scaduto"), sys.stdout.flush(), os._exit(0)))
Gtk.main()
