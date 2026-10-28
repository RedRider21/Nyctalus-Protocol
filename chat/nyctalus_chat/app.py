# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Finestra della chat `.nyct`.

Interfaccia leggera: non fa crittografia e non vede mai le chiavi. Parla col
**motore** (`nyctalus chat`) su una porta di controllo locale (solo loopback)
con un semplice protocollo a righe di testo:

    inviati:   WHOAMI · LIST · SUB · ADD <addr> <hexnick> · HIST <addr> · SEND <addr> <hextesto>
    ricevuti:  ME <addr> · C <addr> <hexnick> · ENDC · P <in|out> <ts> <hex> · ENDH
               · MSG <addr> <ts> <hex> · OUT <addr> <ts> <hex> · ST <addr> <0|1> · NEW <addr>

Testi e soprannomi viaggiano in esadecimale, per restare al sicuro da spazi e
a-capo nel protocollo a righe.
"""
import os
import socket
import threading
import time
from datetime import datetime

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, GLib, Gtk, Pango  # noqa: E402

try:
    from nxs_i18n import t as _t
except Exception:                # noqa: BLE001
    def _t(key, **kw):
        return _ITA.get(key, key)

# Testi italiani di riserva, se nxs_i18n non è nel percorso.
_ITA = {
    "chat.titolo": "Nyctalus · Chat",
    "chat.contatti": "Contatti",
    "chat.aggiungi": "Aggiungi contatto",
    "chat.il_mio_id": "Il tuo Contact ID",
    "chat.copia": "Copia",
    "chat.copiato": "Copiato!",
    "chat.scrivi": "Scrivi un messaggio…",
    "chat.invia": "Invia",
    "chat.online": "in linea",
    "chat.offline": "non in linea",
    "chat.nessun_contatto": "Nessun contatto. Premi « + » e incolla un indirizzo .nyct.",
    "chat.scegli": "Scegli un contatto per iniziare a chattare.",
    "chat.addr": "Indirizzo .nyct",
    "chat.nome": "Nome (soprannome locale)",
    "chat.annulla": "Annulla",
    "chat.ok": "Aggiungi",
    "chat.addr_non_valido": "Indirizzo .nyct non valido.",
    "chat.motore_assente": "Motore non raggiungibile",
    "chat.motore_assente_corpo": "Il motore della chat non è in esecuzione.\nAvvia la chat dallo script o dal launcher.",
    "chat.nuovo_contatto": "Nuovo contatto ti ha scritto",
    "chat.aggiungi_sconosciuto": "Qualcuno ti ha scritto da un indirizzo non in rubrica. Aggiungilo per rispondere.",
}

CTRL_HOST = os.environ.get("NYCTALUS_CHAT_HOST", "127.0.0.1")
CTRL_PORT = int(os.environ.get("NYCTALUS_CHAT_PORT", "1091"))

CSS = b"""
.chat-root { background: #0b1020; }
.chat-lato { background: #0e1424; border-right: 1px solid rgba(90,169,255,0.14); }
.chat-lato-testa { padding: 12px 14px; color: #e6ecff; font-weight: 700; }
.chat-id { background: #0b1020; border-top: 1px solid rgba(90,169,255,0.14); padding: 10px 12px; }
.chat-id label { color: #7f8db3; font-size: 11px; }
.chat-id .valore { color: #5aa9ff; font-family: monospace; font-size: 11px; }
row.contatto { padding: 10px 14px; }
row.contatto:selected { background: rgba(90,169,255,0.16); }
.nome { color: #e6ecff; font-weight: 600; }
.pallino-on  { color: #7ff0a8; }
.pallino-off { color: #46506e; }
.chat-testa { background: #0e1424; border-bottom: 1px solid rgba(90,169,255,0.14); padding: 10px 16px; }
.chat-testa .nome { font-size: 15px; }
.chat-testa .sotto { color: #7f8db3; font-size: 11px; font-family: monospace; }
.conversazione { background: #0b1020; }
.bolla { padding: 8px 12px; border-radius: 14px; margin: 3px 8px; }
.bolla-in  { background: #182238; color: #e6ecff; }
.bolla-out { background: #2a4a7f; color: #ffffff; }
.bolla .ora { color: rgba(255,255,255,0.45); font-size: 10px; }
.compositore { background: #0e1424; border-top: 1px solid rgba(90,169,255,0.14); padding: 8px; }
.compositore entry { background: #0b1020; color: #e6ecff; border-radius: 12px;
                     border: 1px solid rgba(90,169,255,0.18); padding: 8px 12px; }
.invia-btn { background: #2f6fe0; color: #fff; border-radius: 12px; padding: 8px 16px; border: none; }
.invia-btn:hover { background: #3b7cf0; }
.aggiungi-btn { background: rgba(90,169,255,0.14); color: #e6ecff; border: none; border-radius: 10px; }
.vuoto { color: #7f8db3; }
"""


def ora_hhmm(ts):
    try:
        return datetime.fromtimestamp(int(ts)).strftime("%H:%M")
    except Exception:            # noqa: BLE001
        return ""


class Chat(Gtk.Window):
    def __init__(self):
        super().__init__(title=_t("chat.titolo"))
        self.set_default_size(820, 560)
        try:
            self.set_icon_name("nyctalus")
        except Exception:        # noqa: BLE001
            pass

        self.me = ""
        self.contatti = {}       # addr -> nome
        self.online = set()
        self.attivo = None
        self.righe_contatto = {} # addr -> Gtk.ListBoxRow
        self.sock = None

        prov = Gtk.CssProvider()
        prov.load_from_data(CSS)
        Gtk.StyleContext.add_provider_for_screen(
            Gdk.Screen.get_default(), prov, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        self.get_style_context().add_class("chat-root")

        self._costruisci()
        self.connect("destroy", Gtk.main_quit)
        self.show_all()
        self._mostra_vuoto()

        if not self._connetti():
            self._errore_motore()

    # --- interfaccia -----------------------------------------------------------
    def _costruisci(self):
        radice = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        self.add(radice)

        # colonna sinistra: contatti + mio Contact ID
        lato = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        lato.get_style_context().add_class("chat-lato")
        lato.set_size_request(260, -1)
        radice.pack_start(lato, False, False, 0)

        testa = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        etc = Gtk.Label(label=_t("chat.contatti"), xalign=0)
        etc.get_style_context().add_class("chat-lato-testa")
        testa.pack_start(etc, True, True, 0)
        agg = Gtk.Button(label="  +  ")
        agg.get_style_context().add_class("aggiungi-btn")
        agg.set_tooltip_text(_t("chat.aggiungi"))
        agg.connect("clicked", lambda _w: self._dialogo_aggiungi())
        testa.pack_end(agg, False, False, 8)
        lato.pack_start(testa, False, False, 6)

        scorri = Gtk.ScrolledWindow()
        scorri.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.lista = Gtk.ListBox()
        self.lista.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self.lista.connect("row-selected", self._scelto_contatto)
        scorri.add(self.lista)
        lato.pack_start(scorri, True, True, 0)

        self.vuoto_contatti = Gtk.Label(label=_t("chat.nessun_contatto"))
        self.vuoto_contatti.get_style_context().add_class("vuoto")
        self.vuoto_contatti.set_line_wrap(True)
        self.vuoto_contatti.set_xalign(0)
        self.vuoto_contatti.set_margin_start(14)
        self.vuoto_contatti.set_margin_end(14)
        lato.pack_start(self.vuoto_contatti, False, False, 6)

        idbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        idbox.get_style_context().add_class("chat-id")
        idbox.pack_start(Gtk.Label(label=_t("chat.il_mio_id"), xalign=0), False, False, 0)
        riga_id = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        self.mio_id = Gtk.Label(label="…", xalign=0)
        self.mio_id.get_style_context().add_class("valore")
        self.mio_id.set_ellipsize(Pango.EllipsizeMode.MIDDLE)
        self.mio_id.set_selectable(True)
        riga_id.pack_start(self.mio_id, True, True, 0)
        self.btn_copia = Gtk.Button(label=_t("chat.copia"))
        self.btn_copia.get_style_context().add_class("aggiungi-btn")
        self.btn_copia.connect("clicked", self._copia_id)
        riga_id.pack_end(self.btn_copia, False, False, 0)
        idbox.pack_start(riga_id, False, False, 4)
        lato.pack_start(idbox, False, False, 0)

        # colonna destra: conversazione
        destra = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        radice.pack_start(destra, True, True, 0)

        self.testa_conv = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.testa_conv.get_style_context().add_class("chat-testa")
        self.conv_nome = Gtk.Label(label="", xalign=0)
        self.conv_nome.get_style_context().add_class("nome")
        self.conv_sotto = Gtk.Label(label="", xalign=0)
        self.conv_sotto.get_style_context().add_class("sotto")
        self.testa_conv.pack_start(self.conv_nome, False, False, 0)
        self.testa_conv.pack_start(self.conv_sotto, False, False, 0)
        destra.pack_start(self.testa_conv, False, False, 0)

        self.scorri_conv = Gtk.ScrolledWindow()
        self.scorri_conv.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.conv = Gtk.ListBox()
        self.conv.set_selection_mode(Gtk.SelectionMode.NONE)
        self.conv.get_style_context().add_class("conversazione")
        self.scorri_conv.add(self.conv)
        destra.pack_start(self.scorri_conv, True, True, 0)

        self.comp = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        self.comp.get_style_context().add_class("compositore")
        self.ingresso = Gtk.Entry()
        self.ingresso.set_placeholder_text(_t("chat.scrivi"))
        self.ingresso.connect("activate", lambda _w: self._invia())
        self.comp.pack_start(self.ingresso, True, True, 0)
        inviab = Gtk.Button(label=_t("chat.invia"))
        inviab.get_style_context().add_class("invia-btn")
        inviab.connect("clicked", lambda _w: self._invia())
        self.comp.pack_end(inviab, False, False, 0)
        destra.pack_start(self.comp, False, False, 0)

    def _mostra_vuoto(self):
        self.conv_nome.set_text("")
        self.conv_sotto.set_text(_t("chat.scegli"))
        self.comp.set_sensitive(False)

    # --- connessione al motore -------------------------------------------------
    def _connetti(self):
        try:
            self.sock = socket.create_connection((CTRL_HOST, CTRL_PORT), timeout=5)
            self.sock.settimeout(None)
        except Exception:        # noqa: BLE001
            self.sock = None
            return False
        threading.Thread(target=self._lettore, daemon=True).start()
        self._cmd("WHOAMI")
        self._cmd("LIST")
        self._cmd("SUB")
        return True

    def _errore_motore(self):
        d = Gtk.MessageDialog(transient_for=self, modal=True, message_type=Gtk.MessageType.WARNING,
                              buttons=Gtk.ButtonsType.OK, text=_t("chat.motore_assente"))
        d.format_secondary_text(_t("chat.motore_assente_corpo"))
        d.run()
        d.destroy()

    def _cmd(self, riga):
        if not self.sock:
            return
        try:
            self.sock.sendall((riga + "\n").encode())
        except Exception:        # noqa: BLE001
            pass

    def _lettore(self):
        buf = b""
        while True:
            try:
                dati = self.sock.recv(4096)
            except Exception:    # noqa: BLE001
                break
            if not dati:
                break
            buf += dati
            while b"\n" in buf:
                riga, buf = buf.split(b"\n", 1)
                testo = riga.decode(errors="replace").strip()
                if testo:
                    GLib.idle_add(self._evento, testo)
        GLib.idle_add(self._mostra_vuoto)

    # --- eventi dal motore -----------------------------------------------------
    def _evento(self, riga):
        parti = riga.split(" ")
        tipo = parti[0]
        if tipo == "ME" and len(parti) >= 2:
            self.me = parti[1]
            self.mio_id.set_text(self.me)
        elif tipo == "C" and len(parti) >= 2:
            addr = parti[1]
            nome = _da_hex(parti[2]) if len(parti) >= 3 and parti[2] else addr[:10]
            self._upsert_contatto(addr, nome)
        elif tipo == "ST" and len(parti) >= 3:
            addr, stato = parti[1], parti[2]
            if stato == "1":
                self.online.add(addr)
            else:
                self.online.discard(addr)
            self._aggiorna_pallino(addr)
            if addr == self.attivo:
                self._aggiorna_testa()
        elif tipo == "MSG" and len(parti) >= 4:
            addr, ts, hx = parti[1], parti[2], parti[3]
            if addr == self.attivo:
                self._bolla("in", _da_hex(hx), ts)
        elif tipo == "OUT" and len(parti) >= 4:
            addr, ts, hx = parti[1], parti[2], parti[3]
            if addr == self.attivo:
                self._bolla("out", _da_hex(hx), ts)
        elif tipo == "P" and len(parti) >= 4:
            direzione, ts, hx = parti[1], parti[2], parti[3]
            self._bolla("out" if direzione == "out" else "in", _da_hex(hx), ts)
        elif tipo == "NEW" and len(parti) >= 2:
            self._contatto_sconosciuto(parti[1])
        return False

    # --- contatti --------------------------------------------------------------
    def _upsert_contatto(self, addr, nome):
        self.contatti[addr] = nome
        riga = self.righe_contatto.get(addr)
        if riga is None:
            riga = self._riga_contatto(addr, nome)
            self.righe_contatto[addr] = riga
            self.lista.add(riga)
            self.lista.show_all()
        else:
            riga.etichetta.set_text(nome)
        self.vuoto_contatti.set_visible(len(self.contatti) == 0)
        self._aggiorna_pallino(addr)

    def _riga_contatto(self, addr, nome):
        riga = Gtk.ListBoxRow()
        riga.get_style_context().add_class("contatto")
        riga.addr = addr
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        pallino = Gtk.Label(label="●")
        pallino.get_style_context().add_class("pallino-off")
        riga.pallino = pallino
        box.pack_start(pallino, False, False, 0)
        et = Gtk.Label(label=nome, xalign=0)
        et.get_style_context().add_class("nome")
        et.set_ellipsize(Pango.EllipsizeMode.END)
        riga.etichetta = et
        box.pack_start(et, True, True, 0)
        riga.add(box)
        return riga

    def _aggiorna_pallino(self, addr):
        riga = self.righe_contatto.get(addr)
        if not riga:
            return
        ctx = riga.pallino.get_style_context()
        if addr in self.online:
            ctx.remove_class("pallino-off")
            ctx.add_class("pallino-on")
        else:
            ctx.remove_class("pallino-on")
            ctx.add_class("pallino-off")

    def _scelto_contatto(self, _lista, riga):
        if riga is None:
            return
        self.attivo = riga.addr
        self.comp.set_sensitive(True)
        self._aggiorna_testa()
        # pulisci e ricarica la cronologia
        for figlio in self.conv.get_children():
            self.conv.remove(figlio)
        self._cmd("HIST %s" % self.attivo)
        self.ingresso.grab_focus()

    def _aggiorna_testa(self):
        if not self.attivo:
            return
        self.conv_nome.set_text(self.contatti.get(self.attivo, self.attivo[:10]))
        stato = _t("chat.online") if self.attivo in self.online else _t("chat.offline")
        self.conv_sotto.set_text("%s · %s" % (stato, self.attivo))

    def _contatto_sconosciuto(self, addr):
        # mostra una barra informativa con l'indirizzo e un pulsante "aggiungi"
        if addr in self.contatti:
            return
        self._dialogo_aggiungi(preimposta=addr, info=_t("chat.aggiungi_sconosciuto"))

    # --- messaggi --------------------------------------------------------------
    def _bolla(self, direzione, testo, ts):
        riga = Gtk.ListBoxRow()
        riga.set_selectable(False)
        contenitore = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        bolla = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        bolla.get_style_context().add_class("bolla")
        bolla.get_style_context().add_class("bolla-out" if direzione == "out" else "bolla-in")
        et = Gtk.Label(label=testo, xalign=0)
        et.set_line_wrap(True)
        et.set_line_wrap_mode(Pango.WrapMode.WORD_CHAR)
        et.set_max_width_chars(48)
        et.set_selectable(True)
        bolla.pack_start(et, False, False, 0)
        oraet = Gtk.Label(label=ora_hhmm(ts), xalign=1)
        oraet.get_style_context().add_class("ora")
        bolla.pack_start(oraet, False, False, 0)
        if direzione == "out":
            contenitore.pack_end(bolla, False, False, 0)
        else:
            contenitore.pack_start(bolla, False, False, 0)
        riga.add(contenitore)
        riga.show_all()
        self.conv.add(riga)
        GLib.idle_add(self._scorri_in_fondo)

    def _scorri_in_fondo(self):
        adj = self.scorri_conv.get_vadjustment()
        adj.set_value(adj.get_upper() - adj.get_page_size())
        return False

    def _invia(self):
        testo = self.ingresso.get_text().strip()
        if not testo or not self.attivo:
            return
        self._cmd("SEND %s %s" % (self.attivo, testo.encode().hex()))
        self.ingresso.set_text("")

    # --- dialoghi --------------------------------------------------------------
    def _dialogo_aggiungi(self, preimposta="", info=""):
        d = Gtk.Dialog(title=_t("chat.aggiungi"), transient_for=self, modal=True)
        d.add_button(_t("chat.annulla"), Gtk.ResponseType.CANCEL)
        d.add_button(_t("chat.ok"), Gtk.ResponseType.OK)
        box = d.get_content_area()
        box.set_spacing(8)
        box.set_border_width(12)
        if info:
            lab = Gtk.Label(label=info, xalign=0)
            lab.set_line_wrap(True)
            box.pack_start(lab, False, False, 0)
        e_addr = Gtk.Entry()
        e_addr.set_placeholder_text(_t("chat.addr"))
        e_addr.set_text(preimposta)
        box.pack_start(e_addr, False, False, 0)
        e_nome = Gtk.Entry()
        e_nome.set_placeholder_text(_t("chat.nome"))
        box.pack_start(e_nome, False, False, 0)
        d.show_all()
        if d.run() == Gtk.ResponseType.OK:
            addr = e_addr.get_text().strip()
            nome = e_nome.get_text().strip() or addr[:10]
            if addr:
                self._cmd("ADD %s %s" % (addr, nome.encode().hex()))
                # mostra subito il contatto (il motore confermerà con C)
                self._upsert_contatto(_con_suffisso(addr), nome)
        d.destroy()

    def _copia_id(self, _w):
        if not self.me:
            return
        Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).set_text(self.me, -1)
        self.btn_copia.set_label(_t("chat.copiato"))
        GLib.timeout_add(1200, lambda: (self.btn_copia.set_label(_t("chat.copia")), False)[1])


def _da_hex(h):
    try:
        return bytes.fromhex(h).decode("utf-8", errors="replace")
    except Exception:            # noqa: BLE001
        return ""


def _con_suffisso(addr):
    return addr if addr.lower().endswith(".nyct") else addr + ".nyct"


def main():
    Chat()
    Gtk.main()


if __name__ == "__main__":
    main()
