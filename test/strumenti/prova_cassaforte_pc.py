#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Cassaforte nella chat del PC, con il motore VERO e dati di prova già cifrati: la chat chiede la
passphrase, una sbagliata viene rifiutata, quella giusta apre la chat (si ricollega da sola) con
contatti e indirizzo; poi dal menu si cambia la passphrase. Le finestre non si aprono davvero: i
campi li riempie la prova. Stampa «OK …» / «KO …».
uso: prova_cassaforte_pc.py PASSPHRASE CONTATTO.nyct   (motore su NYCTALUS_CHAT_PORT)"""
import os
import sys

import gi

gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk  # noqa: E402
from nyctalus_chat import app as chatapp  # noqa: E402

esito = lambda ok, d: print(("OK " if ok else "KO ") + d, flush=True)
GIUSTA, BOB = sys.argv[1], sys.argv[2]
richieste = []          # (titolo, testo) di ogni finestra con campi
risposte = [["non è questa"], [GIUSTA]]


def campi_finti(self, titolo, testo, etichette, conferma=None):
    richieste.append((titolo, testo or "", len(etichette)))
    return risposte.pop(0) if risposte else None


avvisi = []
chatapp.Chat._chiedi_campi = campi_finti
chatapp.Chat._avviso_semplice = lambda self, t: avvisi.append(t)
chat = chatapp.Chat()


def dopo_sblocco():
    esito(len(richieste) >= 2 and richieste[0][2] == 1, "all'avvio la chat chiede la passphrase (%d richieste)" % len(richieste))
    esito(len(richieste) >= 2 and "sbagliata" in richieste[1][1].lower(), "passphrase sbagliata: la richiede di nuovo dicendo «sbagliata»")
    esito(bool(getattr(chat, "me", "")) and chat.me.endswith(".nyct"), "passphrase giusta: la chat si ricollega e ha il suo Contact ID")
    esito(BOB in getattr(chat, "contatti", {}) or any(BOB in str(k) for k in getattr(chat, "righe", {})),
          "i contatti cifrati si vedono dopo lo sblocco")
    esito(getattr(chat, "cassaforte", "") == "aperta", "stato della protezione: «aperta» (voce del menu)")
    # dal menu: cambio della passphrase (scelta 1 nella finestra, poi tre campi)
    risposte.extend([[GIUSTA, "passphrase cambiata dal PC", "passphrase cambiata dal PC"]])

    class FintaScelta:
        def __init__(self, *a, **k): pass
        def format_secondary_text(self, *a): pass
        def add_button(self, *a): pass
        def run(self): return 1
        def destroy(self): pass
    vera = Gtk.MessageDialog
    Gtk.MessageDialog = FintaScelta
    chat._protezione_dati()
    Gtk.MessageDialog = vera
    GLib.timeout_add(6000, fine)
    return False


def fine():
    esito(any("cambiata" in a.lower() for a in avvisi), "«Cambia passphrase» dal menu: il motore conferma (%s)" % (avvisi[-1:] or ["nessun avviso"]))
    sys.stdout.flush()
    os._exit(0)


GLib.timeout_add(9000, dopo_sblocco)
GLib.timeout_add(40000, lambda: (esito(False, "tempo scaduto"), sys.stdout.flush(), os._exit(0)))
Gtk.main()
