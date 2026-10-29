# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Selettore allegati della chat del PC: griglia, miniature, selezione numerata.
Usa NYCTALUS_GALLERIA (galleria finta). Stampa righe «chiave valore».
Con NYCT_FOTO=cartella salva anche uno screenshot."""
import os, subprocess
import gi
gi.require_version("Gtk", "3.0"); gi.require_version("GdkX11", "3.0")
from gi.repository import Gdk, GdkX11, GLib, Gtk  # noqa: F401
from nyctalus_chat import app as chatapp, media

pr = Gtk.CssProvider(); pr.load_from_data(chatapp._css("ardesia", "pipistrelli").encode())
Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(), pr, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
chiavi = ("titolo", "galleria", "fotocamera", "scheda_file", "scheda_altre", "scheda_foto", "scheda_video",
          "didascalia", "exif", "invia", "invia_n", "selezionati", "vuoto")
dlg = media.SelettoreAllegati(None, {k: chatapp._t("chat.gal." + k) for k in chiavi}, True)
esito = {}

def passi():
    esito["caselle"] = len(dlg.tile)
    esito["miniature"] = sum(1 for t in dlg.tile if t.pixbuf is not None)
    dlg._tocca(dlg.tile[2]); dlg._tocca(dlg.tile[0])
    esito["numeri"] = "%d,%d" % (dlg.tile[2].numero, dlg.tile[0].numero)     # in ordine di tocco
    esito["bottone"] = dlg.invia.get_label()
    esito["conta"] = dlg.conta.get_text()
    esito["barra"] = dlg.barra.get_visible() and not dlg.schede.get_visible()
    esito["ordine"] = ",".join(os.path.basename(p) for p in dlg.scelti)
    GLib.timeout_add(600, foto)
    return False

def foto():
    if os.environ.get("NYCT_FOTO"):
        subprocess.run(["import", "-window", str(dlg.get_window().get_xid()), os.environ["NYCT_FOTO"] + "/selettore-pc.png"])
    dlg.response(Gtk.ResponseType.CANCEL)
    return False

GLib.timeout_add(3500, passi)
dlg.run(); dlg.destroy()
for k, v in esito.items():
    print(k, v)
