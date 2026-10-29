# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Fotocamera e anteprima d'invio con sorgenti di PROVA (mai la webcam).
Stampa: «foto PERCORSO», «video PERCORSO», «anteprima ok»."""
import os
os.environ["NYCTALUS_CAMERA_SRC"] = "videotestsrc is-live=true pattern=ball"
os.environ["NYCTALUS_MIC_SRC"] = "audiotestsrc is-live=true wave=sine"
import gi  # noqa: E402
gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk  # noqa: E402
from nyctalus_chat import media  # noqa: E402

testi = {k: k for k in ("titolo_foto", "titolo_video", "scatta", "registra", "stop", "annulla", "errore", "attendi")}
win = Gtk.Window()
esito = {}


def foto():
    cam = media.Fotocamera(win, "foto", testi)
    GLib.timeout_add(2500, lambda: (cam.scatto.clicked(), False)[1])
    cam.run(); esito["foto"] = cam.risultato; cam.destroy()
    GLib.idle_add(video)


def video():
    cam = media.Fotocamera(win, "video", testi)

    def avvia():
        cam.scatto.clicked()
        GLib.timeout_add(3200, lambda: (cam.scatto.clicked(), False)[1])
        return False
    GLib.timeout_add(1500, avvia)
    cam.run(); esito["video"] = cam.risultato; cam.destroy()
    GLib.idle_add(anteprima)


def anteprima():
    voci = [(esito.get("foto"), "image/jpeg")] if esito.get("foto") else []
    t = {"titolo": "Invia %d file", "titolo1": "Invia", "didascalia": "didascalia", "exif": "exif",
         "invia": "Invia", "annulla": "Annulla"}
    if voci:
        dlg = media.AnteprimaInvio(win, voci, t, True, lambda n: str(n))
        ok = dlg.voce_didascalia is not None and dlg.exif.get_active()
        GLib.timeout_add(800, lambda: (dlg.response(Gtk.ResponseType.CANCEL), False)[1])
        dlg.run(); dlg.destroy()
        esito["anteprima"] = ok
    Gtk.main_quit()


GLib.idle_add(foto)
Gtk.main()
print("foto", esito.get("foto") or "")
print("video", esito.get("video") or "")
print("anteprima", "ok" if esito.get("anteprima") else "ko")
