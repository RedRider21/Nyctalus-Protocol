# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""La chiamata nella chat GTK del PC. Alice = la finestra vera (microfono e altoparlante di
prova: NYCTALUS_MIC_SRC / NYCTALUS_ALTOPARLANTE); Bob = un motore guidato da qui, che risponde e
rimanda indietro la voce che riceve (eco). Stampa «OK …» / «KO …».
uso (con NYCTALUS_CHAT_PORT/TOKEN di Alice): prova_chiamata_pc.py PORTA_BOB SEGRETO_BOB IND_ALICE IND_BOB FOTO"""
import socket
import subprocess
import sys
import threading
import time

pb, sb, ind_a, ind_b, foto = sys.argv[1:6]
esito = lambda ok, d: print(("OK " if ok else "KO ") + d, flush=True)

import gi  # noqa: E402
gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk  # noqa: E402
from nyctalus_chat import app as chatapp  # noqa: E402

bob = {"inviti": [], "audio": 0, "fine": [], "s": None}


def motore_bob():
    s = socket.create_connection(("127.0.0.1", int(pb)))
    bob["s"] = s
    s.sendall(("AUTH %s\nSUB\n" % sb).encode())
    buf = b""
    while True:
        d = s.recv(65536)
        if not d:
            return
        buf += d
        while b"\n" in buf:
            r, buf = buf.split(b"\n", 1)
            p = r.decode().split(" ")
            if p[0] == "CALLIN":
                bob["inviti"].append(p[2])
                s.sendall(("CALLOK %s %s\n" % (ind_a, p[2])).encode())     # Bob risponde
            elif p[0] == "AUD":
                bob["audio"] += 1
                s.sendall(("AUD %s %s %s\n" % (ind_a, p[2], p[3])).encode())  # eco
            elif p[0] == "CALLEND":
                bob["fine"].append(p[2])


threading.Thread(target=motore_bob, daemon=True).start()
chat = chatapp.Chat()
ch = chat.chiamate


def passo1():
    chat.scrivi_a(ind_b, "Bob")
    ch.chiama(ind_b)
    GLib.timeout_add(9000, passo2)
    return False


def passo2():
    a = ch.c and ch.c.get("audio")
    esito(bool(bob["inviti"]) and ch.c and ch.c["stato"] == "attiva", "📞 dalla chat: Bob riceve l'invito e risponde, la chiamata è attiva")
    esito(bob["audio"] > 100, "la voce del microfono (Opus da 20 ms) arriva a Bob: %d pacchetti" % bob["audio"])
    esito(bool(a) and a.ricevuti > 100, "la voce di Bob torna e si sente: %d pacchetti suonati" % (a.ricevuti if a else 0))
    esito(ch.barra.get_reveal_child() and "Bob" in ch.etichetta.get_text(), "barra della chiamata: «%s» %s" % (ch.etichetta.get_text(), ch.qualita.get_text()))
    ch.muto.set_active(True)
    n = a.inviati if a else 0
    GLib.timeout_add(1500, lambda: passo3(n))
    return False


def passo3(n):
    a = ch.c["audio"]
    esito(a.inviati - n <= 2, "🎤 muto: dal microfono non parte più nulla")
    ch.muto.set_active(False)
    ch._volume(1.5)
    esito(abs(a.ascolto.get_by_name("vol").get_property("volume") - 1.5) < 0.01, "🔊 volume regolabile durante la chiamata")
    w = chat.get_window()
    subprocess.run(["import", "-window", str(w.get_xid()), foto], capture_output=True)
    ch.riaggancia()
    GLib.timeout_add(4000, passo4)
    return False


def passo4():
    esito(bool(bob["fine"]) and ch.c is None and not ch.barra.get_reveal_child(), "✆ riaggancia: Bob lo sa, la barra sparisce")
    Gtk.main_quit()
    return False


GLib.timeout_add(7000, passo1)
GLib.timeout_add(60000, lambda: (esito(False, "tempo scaduto"), Gtk.main_quit()))
Gtk.main()
