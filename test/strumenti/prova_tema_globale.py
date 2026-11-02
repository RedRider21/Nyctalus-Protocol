# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Tema globale del PC: mini menu e chat aperti insieme; il tema scelto in uno arriva al volo
all'altro (e al file condiviso). Stampa «OK …» / «KO …». Va eseguita con HOME e
XDG_CONFIG_HOME temporanei (mai la configurazione vera)."""
import os
import sys
import gi
gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk  # noqa: E402
import nxs_tema  # noqa: E402
from nyctalus_chat import app as chatapp  # noqa: E402
from nyctalus_launcher import app as lapp  # noqa: E402

esito = lambda ok, d: print(("OK " if ok else "KO ") + d, flush=True)


def finto_motore(porta):
    """Porta di controllo finta: la chat si collega e riceve il suo indirizzo (la prova riguarda
    solo i temi; senza motore la chat aprirebbe la finestra modale «motore non raggiungibile»)."""
    import socket
    import threading
    srv = socket.socket()
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", porta))
    srv.listen(8)

    def cliente(c):
        buf = b""
        try:
            while True:
                d = c.recv(4096)
                if not d:
                    return
                buf += d
                while b"\n" in buf:
                    riga, buf = buf.split(b"\n", 1)
                    if riga.startswith(b"WHOAMI"):
                        c.sendall(b"ME " + b"a" * 56 + b".nyct\n")
        except OSError:
            pass

    def accetta():
        while True:
            c, _ = srv.accept()
            threading.Thread(target=cliente, args=(c,), daemon=True).start()
    threading.Thread(target=accetta, daemon=True).start()


finto_motore(int(os.environ.get("NYCTALUS_CHAT_PORT", "21999")))
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
    lau._scegli_tema("menta")
    GLib.timeout_add(800, passo4)
    return False


def passo4():
    m = nxs_tema.TEMI["menta"]
    esito(chat.ui["tema"] == "menta" and rgb(m["BG"]) in css(chat.prov), "tema nuovo «Menta» (verde): la chat lo prende al volo")
    nxs_tema.scrivi_personalizzato("#ff8800", False)    # come dalla finestra «Personalizzato…»
    GLib.timeout_add(800, passo5)
    return False


def passo5():
    p = nxs_tema.deriva("#ff8800", False)
    esito(nxs_tema.leggi() == "personalizzato" and chat.ui["tema"] == "personalizzato" and rgb(p["BG"]) in css(chat.prov),
          "tema PERSONALIZZATO (#ff8800, chiaro): la chat si ricolora con la tavolozza ricavata dal colore")
    esito(rgb(p["PANEL"]) in css(lau.prov) and not nxs_tema.scuro(), "anche il mini menu, in versione chiara")
    nxs_tema.scrivi_personalizzato("#2266ff", True)     # un altro colore personalizzato
    GLib.timeout_add(800, passo6)
    return False


def passo6():
    p = nxs_tema.deriva("#2266ff", True)
    esito(rgb(p["BG"]) in css(chat.prov), "cambiando solo il colore personalizzato, i programmi aperti si aggiornano")
    return fine()


def fine(ok=True):
    if not ok:
        esito(False, "tempo scaduto")
    sys.stdout.flush()
    os._exit(0)             # chiusura netta: niente deadlock di GTK/thread allo shutdown
    return False


GLib.timeout_add(1200, passo1)
GLib.timeout_add(30000, lambda: fine(False))
Gtk.main()
