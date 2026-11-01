# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Tema globale del PC: mini menu e chat aperti insieme; il tema scelto in uno arriva al volo
all'altro (e al file condiviso). Stampa «OK …» / «KO …». Va eseguita con HOME e
XDG_CONFIG_HOME temporanei (mai la configurazione vera)."""
import gi
gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk  # noqa: E402
import nxs_tema  # noqa: E402
from nyctalus_chat import app as chatapp  # noqa: E402
from nyctalus_launcher import app as lapp  # noqa: E402

esito = lambda ok, d: print(("OK " if ok else "KO ") + d, flush=True)
chat = chatapp.Chat()
lau = lapp.Launcher()
css = lambda p: p.to_string()
rgb = lambda h: "rgb(%d,%d,%d)" % tuple(int(h[i:i + 2], 16) for i in (1, 3, 5))   # GTK riscrive #rrggbb così


class Voce:                       # finto RadioMenuItem attivo
    def get_active(self):
        return True


def passo1():
    esito(chat.ui["tema"] == "notte" and nxs_tema.leggi() == "notte", "all'inizio tema predefinito (notte) per tutti")
    lau._scegli_tema("viola")                       # dal menu ⚙ del mini menu
    GLib.timeout_add(800, passo2)
    return False


def passo2():
    v = nxs_tema.TEMI["viola"]
    esito(nxs_tema.leggi() == "viola", "mini menu → Tema «Viola»: scritto nel tema condiviso")
    esito(chat.ui["tema"] == "viola" and rgb(v["BG"]) in css(chat.prov), "la chat aperta cambia colori al volo (viola)")
    esito(rgb(v["PANEL"]) in css(lau.prov), "il pannello del mini menu prende i colori del tema")
    chat._scelto_aspetto(Voce(), "tema", "chiaro")    # dal menu della chat
    GLib.timeout_add(800, passo3)
    return False


def passo3():
    c = nxs_tema.TEMI["chiaro"]
    esito(nxs_tema.leggi() == "chiaro", "chat → «Chiaro»: vale per tutti i programmi")
    esito(rgb(c["PANEL"]) in css(lau.prov), "il mini menu si accorge del cambio fatto nella chat")
    Gtk.main_quit()
    return False


GLib.timeout_add(1200, passo1)
GLib.timeout_add(15000, lambda: (esito(False, "tempo scaduto"), Gtk.main_quit()))
Gtk.main()
