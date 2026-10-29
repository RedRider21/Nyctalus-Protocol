#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Screenshot della chat del PC sui dati dimostrativi (HOME finta già pronta, motore
avviato fuori con NYCTALUS_CHAT_PORT/TOKEN). Uso: foto_chat_pc.py CARTELLA_IMMAGINI"""
import json
import os
import subprocess
import sys
from pathlib import Path

import gi
gi.require_version("Gtk", "3.0")
gi.require_version("GdkX11", "3.0")
from gi.repository import Gdk, GdkX11, GLib, Gtk  # noqa: F401,E402

from nyctalus_chat import app as chatapp, media  # noqa: E402

OUT = Path(sys.argv[1])
demo = json.load(open(Path.home() / "demo.json"))
alice = dict((n, a) for n, a in demo["contatti"])["Alice"]


def scatta(finestra, nome):
    subprocess.run(["import", "-window", str(finestra.get_window().get_xid()), str(OUT / nome)], check=False)


chat = chatapp.Chat()


def apri_alice():
    riga = chat.righe_contatto.get(alice)
    if riga is not None:
        chat.lista.select_row(riga)
    GLib.timeout_add(2500, foto_chat)
    return False


def foto_chat():
    scatta(chat, "pc-chat.png")
    chiavi = ("titolo", "galleria", "fotocamera", "scheda_file", "scheda_altre", "scheda_foto", "scheda_video",
              "didascalia", "exif", "invia", "invia_n", "selezionati", "vuoto")
    dlg = media.SelettoreAllegati(chat, {k: chatapp._t("chat.gal." + k) for k in chiavi}, True)

    def scegli():
        if len(dlg.tile) > 4:
            dlg._tocca(dlg.tile[1])
            dlg._tocca(dlg.tile[4])
        GLib.timeout_add(1500, lambda: (scatta(dlg, "pc-allegati.png"), dlg.response(Gtk.ResponseType.CANCEL), False)[2])
        return False
    GLib.timeout_add(3500, scegli)
    dlg.run()
    dlg.destroy()
    Gtk.main_quit()
    return False


GLib.timeout_add(3000, apri_alice)
Gtk.main()
