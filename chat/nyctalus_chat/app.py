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
from datetime import datetime
from pathlib import Path

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, GdkPixbuf, GLib, Gtk, Pango  # noqa: E402

try:
    from nxs_i18n import t as _t_base
except Exception:                # noqa: BLE001
    _t_base = None


def _t(key, **kw):
    """Traduzione con ripiego all'italiano: se nxs_i18n non ha la chiave
    (restituisce la chiave stessa) usiamo i testi di riserva _ITA."""
    if _t_base is not None:
        valore = _t_base(key, **kw)
        if valore != key:
            return valore
    return _ITA.get(key, key)


# Testi italiani di riserva, se nxs_i18n non è nel percorso.
_ITA = {
    "chat.titolo": "Nyctalus · Chat",
    "chat.app": "Nyctalus",
    "chat.sottotitolo": "Chat .nyct",
    "chat.contatti": "Contatti",
    "chat.cerca": "Cerca…",
    "chat.aggiungi": "Aggiungi contatto",
    "chat.il_mio_id": "Il tuo Contact ID",
    "chat.id_tip": "È il tuo indirizzo nella rete: condividilo per farti raggiungere.\nNon è segreto. La chiave privata resta in ~/.nyctalus-chat.",
    "chat.copia": "Copia",
    "chat.copiato": "Copiato negli appunti!",
    "chat.scrivi": "Scrivi un messaggio…",
    "chat.invia": "Invia",
    "chat.online": "in linea",
    "chat.offline": "non in linea",
    "chat.nessun_contatto": "Nessun contatto.\nPremi « + » e incolla un indirizzo .nyct.",
    "chat.benvenuto": "Le tue conversazioni, cifrate e senza metadati",
    "chat.scegli": "Scegli un contatto a sinistra, oppure aggiungine uno con « + ».",
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
LOGO = Path(__file__).resolve().parents[2] / "logo.svg"

# Colori per gli avatar (scelti in modo deterministico dall'indirizzo).
AVATAR_COLORI = ["#5aa9ff", "#7f6cff", "#38c6a4", "#e0794b", "#d45b8e",
                 "#c9a227", "#4bb3e0", "#8a86ff"]

CSS = b"""
.chat-root { background: #0a0e1a; }

/* le ListBox/ScrolledWindow non devono mostrare il bianco del tema */
scrolledwindow, scrolledwindow viewport, list, list > row {
    background-color: transparent;
    background-image: none;
}

/* --- colonna sinistra --- */
.sidebar { background: #0c1120; border-right: 1px solid rgba(90,169,255,0.12); }
.sidebar-testa { padding: 14px 16px 10px 16px; }
.app-nome { color: #e6ecff; font-weight: 800; font-size: 15px; }
.app-sotto { color: #5aa9ff; font-size: 11px; }
.cerca { margin: 0 12px 8px 12px; }
.cerca entry, entry.cerca { background: #0a0e1a; color: #e6ecff; border-radius: 10px;
        border: 1px solid rgba(90,169,255,0.16); padding: 6px 10px; }
.sezione { color: #6b7797; font-size: 11px; font-weight: 700; letter-spacing: .4px;
           padding: 4px 16px; }

row.contatto { padding: 7px 12px; border-radius: 12px; margin: 1px 8px; }
row.contatto:selected { background: rgba(90,169,255,0.18); box-shadow: inset 3px 0 0 #5aa9ff; }
row.contatto:hover { background: rgba(90,169,255,0.08); }
.nome { color: #e6ecff; font-weight: 600; }
.stato-riga { color: #6b7797; font-size: 11px; }

.avatar { color: #ffffff; font-weight: 700; border-radius: 22px;
          min-width: 38px; min-height: 38px; }
.avatar.grande { min-width: 44px; min-height: 44px; border-radius: 26px; font-size: 17px; }
.avatar.online { box-shadow: 0 0 0 2px #0c1120, 0 0 0 4px #37d08a; }
.av0{background:#5aa9ff}.av1{background:#7f6cff}.av2{background:#38c6a4}.av3{background:#e0794b}
.av4{background:#d45b8e}.av5{background:#c9a227}.av6{background:#4bb3e0}.av7{background:#8a86ff}

/* card del Contact ID */
.idcard { background: #0a0e1a; border-top: 1px solid rgba(90,169,255,0.12); padding: 12px 14px; }
.idcard .etich { color: #6b7797; font-size: 11px; font-weight: 700; }
.idcard .valore { color: #5aa9ff; font-family: monospace; font-size: 11px; }
.pill { background: rgba(90,169,255,0.14); color: #cfe0ff; border: none;
        border-radius: 9px; padding: 4px 10px; }
.pill:hover { background: rgba(90,169,255,0.24); }
.piu { background: #2f6fe0; color: #fff; border: none; border-radius: 10px;
       min-width: 34px; min-height: 30px; font-weight: 800; }
.piu:hover { background: #3b7cf0; }

/* --- colonna destra --- */
.conv-testa { background: #0e1424; border-bottom: 1px solid rgba(90,169,255,0.12); padding: 10px 16px; }
.conv-testa .nome { font-size: 15px; }
.conv-testa .sotto { color: #6b7797; font-size: 11px; }
.conv-testa .sotto.on { color: #37d08a; }
/* sfondo conversazione: velo di colore e profondita' (stile chat moderne) */
.conversazione { padding: 10px 6px;
    background-color: #0a0e1a;
    background-image:
        radial-gradient(circle at 20% 0%, rgba(90,169,255,0.08), rgba(0,0,0,0) 45%),
        radial-gradient(circle at 85% 100%, rgba(127,108,255,0.08), rgba(0,0,0,0) 45%); }

.bolla { padding: 9px 13px; border-radius: 17px; margin: 3px 12px;
         box-shadow: 0 1px 2px rgba(0,0,0,0.30); }
.bolla-in  { background-color: #1b2740; color: #e6ecff; border-radius: 17px 17px 17px 5px; }
.bolla-out { color: #ffffff; border-radius: 17px 17px 5px 17px;
             background-image: linear-gradient(135deg, #2f6fe0, #3f82f2); }
.bolla .testo { font-size: 13px; }
.bolla .ora { font-size: 10px; }
.bolla-in  .ora { color: rgba(230,236,255,0.40); }
.bolla-out .ora { color: rgba(255,255,255,0.55); }

/* benvenuto / stato vuoto */
.vuoto-wrap { background: #0a0e1a; }
.vuoto-bat { font-size: 54px; }
.vuoto-tit { color: #e6ecff; font-weight: 700; font-size: 17px; }
.vuoto-sub { color: #6b7797; font-size: 13px; }
.hint { color: #6b7797; font-size: 12px; }

/* compositore */
.compositore { background: #0e1424; border-top: 1px solid rgba(90,169,255,0.12); padding: 10px; }
.compositore entry { background: #0a0e1a; color: #e6ecff; border-radius: 20px;
                     border: 1px solid rgba(90,169,255,0.18); padding: 10px 16px; }
.compositore entry:focus { border-color: #5aa9ff; }
.send { background: #2f6fe0; color: #fff; border: none; border-radius: 21px;
        min-width: 42px; min-height: 42px; font-size: 16px; }
.send:hover { background: #3b7cf0; }
.send:disabled { background: #1b2740; color: #4a5676; }
"""


def ora_hhmm(ts):
    try:
        return datetime.fromtimestamp(int(ts)).strftime("%H:%M")
    except Exception:            # noqa: BLE001
        return ""


def iniziale(nome, addr):
    base = (nome or addr or "?").strip()
    return base[0].upper() if base else "?"


def indice_colore(addr):
    return sum(addr.encode()) % len(AVATAR_COLORI)


def fai_avatar(nome, addr, grande=False):
    av = Gtk.Label(label=iniziale(nome, addr))
    ctx = av.get_style_context()
    ctx.add_class("avatar")
    ctx.add_class("av%d" % indice_colore(addr))
    if grande:
        ctx.add_class("grande")
    return av


class Chat(Gtk.Window):
    def __init__(self):
        super().__init__(title=_t("chat.titolo"))
        self.set_default_size(880, 600)
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
        self.filtro = ""

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
        radice.pack_start(self._lato(), False, False, 0)

        sep = Gtk.Separator(orientation=Gtk.Orientation.VERTICAL)
        radice.pack_start(sep, False, False, 0)

        radice.pack_start(self._destra(), True, True, 0)

    def _lato(self):
        lato = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        lato.get_style_context().add_class("sidebar")
        lato.set_size_request(290, -1)

        # intestazione: logo + nome app + "+"
        testa = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        testa.get_style_context().add_class("sidebar-testa")
        try:
            px = GdkPixbuf.Pixbuf.new_from_file_at_scale(str(LOGO), 30, 30, True)
            testa.pack_start(Gtk.Image.new_from_pixbuf(px), False, False, 0)
        except Exception:        # noqa: BLE001
            pass
        titoli = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        n = Gtk.Label(label=_t("chat.app"), xalign=0); n.get_style_context().add_class("app-nome")
        s = Gtk.Label(label=_t("chat.sottotitolo"), xalign=0); s.get_style_context().add_class("app-sotto")
        titoli.pack_start(n, False, False, 0); titoli.pack_start(s, False, False, 0)
        testa.pack_start(titoli, True, True, 0)
        agg = Gtk.Button(label="+"); agg.get_style_context().add_class("piu")
        agg.set_tooltip_text(_t("chat.aggiungi")); agg.set_valign(Gtk.Align.CENTER)
        agg.connect("clicked", lambda _w: self._dialogo_aggiungi())
        testa.pack_end(agg, False, False, 0)
        lato.pack_start(testa, False, False, 0)

        # ricerca
        self.ricerca = Gtk.SearchEntry()
        self.ricerca.set_placeholder_text(_t("chat.cerca"))
        self.ricerca.get_style_context().add_class("cerca")
        self.ricerca.connect("search-changed", self._cerca_cambiata)
        lato.pack_start(self.ricerca, False, False, 0)

        et = Gtk.Label(label=_t("chat.contatti"), xalign=0)
        et.get_style_context().add_class("sezione")
        lato.pack_start(et, False, False, 0)

        scorri = Gtk.ScrolledWindow()
        scorri.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.lista = Gtk.ListBox()
        self.lista.set_selection_mode(Gtk.SelectionMode.SINGLE)
        self.lista.set_filter_func(self._filtra_riga)
        self.lista.connect("row-selected", self._scelto_contatto)
        scorri.add(self.lista)
        lato.pack_start(scorri, True, True, 0)

        self.vuoto_contatti = Gtk.Label(label=_t("chat.nessun_contatto"))
        self.vuoto_contatti.get_style_context().add_class("hint")
        self.vuoto_contatti.set_justify(Gtk.Justification.CENTER)
        self.vuoto_contatti.set_line_wrap(True)
        lato.pack_start(self.vuoto_contatti, False, False, 10)

        # card Contact ID
        idbox = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        idbox.get_style_context().add_class("idcard")
        e = Gtk.Label(label=_t("chat.il_mio_id"), xalign=0); e.get_style_context().add_class("etich")
        e.set_tooltip_text(_t("chat.id_tip"))
        idbox.pack_start(e, False, False, 0)
        idbox.set_tooltip_text(_t("chat.id_tip"))
        riga = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=6)
        self.mio_id = Gtk.Label(label="…", xalign=0)
        self.mio_id.get_style_context().add_class("valore")
        self.mio_id.set_ellipsize(Pango.EllipsizeMode.MIDDLE)
        self.mio_id.set_selectable(True)
        riga.pack_start(self.mio_id, True, True, 0)
        self.btn_copia = Gtk.Button(label=_t("chat.copia"))
        self.btn_copia.get_style_context().add_class("pill")
        self.btn_copia.connect("clicked", self._copia_id)
        riga.pack_end(self.btn_copia, False, False, 0)
        idbox.pack_start(riga, False, False, 0)
        lato.pack_start(idbox, False, False, 0)
        return lato

    def _destra(self):
        destra = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)

        # intestazione conversazione (avatar + nome + stato)
        self.testa_conv = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        self.testa_conv.get_style_context().add_class("conv-testa")
        self.testa_avatar_slot = Gtk.Box()
        self.testa_conv.pack_start(self.testa_avatar_slot, False, False, 0)
        col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.conv_nome = Gtk.Label(label="", xalign=0); self.conv_nome.get_style_context().add_class("nome")
        self.conv_sotto = Gtk.Label(label="", xalign=0); self.conv_sotto.get_style_context().add_class("sotto")
        self.conv_sotto.set_ellipsize(Pango.EllipsizeMode.MIDDLE)
        col.pack_start(self.conv_nome, False, False, 0)
        col.pack_start(self.conv_sotto, False, False, 0)
        self.testa_conv.pack_start(col, True, True, 0)
        destra.pack_start(self.testa_conv, False, False, 0)

        # pila: stato vuoto / conversazione
        self.pila = Gtk.Stack()
        self.pila.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        destra.pack_start(self.pila, True, True, 0)

        self.pila.add_named(self._pannello_vuoto(), "vuoto")

        self.scorri_conv = Gtk.ScrolledWindow()
        self.scorri_conv.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.conv = Gtk.ListBox()
        self.conv.set_selection_mode(Gtk.SelectionMode.NONE)
        self.conv.get_style_context().add_class("conversazione")
        self.scorri_conv.add(self.conv)
        self.pila.add_named(self.scorri_conv, "conv")

        # compositore
        self.comp = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self.comp.get_style_context().add_class("compositore")
        self.ingresso = Gtk.Entry()
        self.ingresso.set_placeholder_text(_t("chat.scrivi"))
        self.ingresso.connect("activate", lambda _w: self._invia())
        self.ingresso.connect("changed", self._aggiorna_send)
        self.comp.pack_start(self.ingresso, True, True, 0)
        self.btn_invia = Gtk.Button(label="➤")
        self.btn_invia.get_style_context().add_class("send")
        self.btn_invia.set_tooltip_text(_t("chat.invia"))
        self.btn_invia.connect("clicked", lambda _w: self._invia())
        self.comp.pack_end(self.btn_invia, False, False, 0)
        destra.pack_start(self.comp, False, False, 0)
        return destra

    def _pannello_vuoto(self):
        wrap = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        wrap.get_style_context().add_class("vuoto-wrap")
        wrap.set_valign(Gtk.Align.CENTER)
        wrap.set_halign(Gtk.Align.CENTER)
        bat = Gtk.Label(label="🦇"); bat.get_style_context().add_class("vuoto-bat")
        tit = Gtk.Label(label=_t("chat.benvenuto")); tit.get_style_context().add_class("vuoto-tit")
        tit.set_line_wrap(True); tit.set_justify(Gtk.Justification.CENTER); tit.set_max_width_chars(32)
        sub = Gtk.Label(label=_t("chat.scegli")); sub.get_style_context().add_class("vuoto-sub")
        sub.set_line_wrap(True); sub.set_justify(Gtk.Justification.CENTER); sub.set_max_width_chars(36)
        for w in (bat, tit, sub):
            w.set_margin_top(6); w.set_margin_bottom(6)
            wrap.pack_start(w, False, False, 0)
        return wrap

    def _mostra_vuoto(self):
        self.attivo = None
        self.conv_nome.set_text("")
        self.conv_sotto.set_text("")
        for f in self.testa_avatar_slot.get_children():
            self.testa_avatar_slot.remove(f)
        self.testa_conv.set_visible(False)
        self.comp.set_sensitive(False)
        if hasattr(self, "pila"):
            self.pila.set_visible_child_name("vuoto")

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
        box = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=10)
        av = fai_avatar(nome, addr)
        riga.avatar = av
        box.pack_start(av, False, False, 0)
        col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        et = Gtk.Label(label=nome, xalign=0); et.get_style_context().add_class("nome")
        et.set_ellipsize(Pango.EllipsizeMode.END)
        riga.etichetta = et
        st = Gtk.Label(label=_t("chat.offline"), xalign=0); st.get_style_context().add_class("stato-riga")
        riga.stato = st
        col.pack_start(et, False, False, 0)
        col.pack_start(st, False, False, 0)
        box.pack_start(col, True, True, 0)
        riga.add(box)
        return riga

    def _aggiorna_pallino(self, addr):
        riga = self.righe_contatto.get(addr)
        if not riga:
            return
        on = addr in self.online
        ctx = riga.avatar.get_style_context()
        ctx.add_class("online") if on else ctx.remove_class("online")
        riga.stato.set_text(_t("chat.online") if on else _t("chat.offline"))

    def _cerca_cambiata(self, entry):
        self.filtro = entry.get_text().strip().lower()
        self.lista.invalidate_filter()

    def _filtra_riga(self, riga):
        if not self.filtro:
            return True
        nome = self.contatti.get(riga.addr, "")
        return self.filtro in nome.lower() or self.filtro in riga.addr.lower()

    def _scelto_contatto(self, _lista, riga):
        if riga is None:
            return
        self.attivo = riga.addr
        self.comp.set_sensitive(True)
        self.testa_conv.set_visible(True)
        self._aggiorna_testa()
        for figlio in self.conv.get_children():
            self.conv.remove(figlio)
        self.pila.set_visible_child_name("conv")
        self._cmd("HIST %s" % self.attivo)
        self.ingresso.grab_focus()
        self._aggiorna_send()

    def _aggiorna_testa(self):
        if not self.attivo:
            return
        nome = self.contatti.get(self.attivo, self.attivo[:10])
        self.conv_nome.set_text(nome)
        on = self.attivo in self.online
        self.conv_sotto.set_text(_t("chat.online") if on else _t("chat.offline"))
        ctx = self.conv_sotto.get_style_context()
        ctx.add_class("on") if on else ctx.remove_class("on")
        for f in self.testa_avatar_slot.get_children():
            self.testa_avatar_slot.remove(f)
        av = fai_avatar(nome, self.attivo, grande=True)
        if on:
            av.get_style_context().add_class("online")
        self.testa_avatar_slot.add(av)
        self.testa_avatar_slot.show_all()

    def _contatto_sconosciuto(self, addr):
        if addr in self.contatti:
            return
        self._dialogo_aggiungi(preimposta=addr, info=_t("chat.aggiungi_sconosciuto"))

    # --- messaggi --------------------------------------------------------------
    def _bolla(self, direzione, testo, ts):
        riga = Gtk.ListBoxRow()
        riga.set_selectable(False)
        contenitore = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        bolla = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        bolla.get_style_context().add_class("bolla")
        bolla.get_style_context().add_class("bolla-out" if direzione == "out" else "bolla-in")
        et = Gtk.Label(label=testo, xalign=0); et.get_style_context().add_class("testo")
        et.set_line_wrap(True)
        et.set_line_wrap_mode(Pango.WrapMode.WORD_CHAR)
        et.set_max_width_chars(46)
        et.set_selectable(True)
        bolla.pack_start(et, False, False, 0)
        oraet = Gtk.Label(label=ora_hhmm(ts), xalign=1); oraet.get_style_context().add_class("ora")
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

    def _aggiorna_send(self, *_a):
        pieno = bool(self.ingresso.get_text().strip()) and self.attivo is not None
        self.btn_invia.set_sensitive(pieno)

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
        box.set_border_width(14)
        if info:
            lab = Gtk.Label(label=info, xalign=0)
            lab.set_line_wrap(True)
            lab.set_max_width_chars(44)
            box.pack_start(lab, False, False, 0)
        e_addr = Gtk.Entry()
        e_addr.set_placeholder_text(_t("chat.addr"))
        e_addr.set_text(preimposta)
        e_addr.set_width_chars(44)
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
                self._upsert_contatto(_con_suffisso(addr), nome)
        d.destroy()

    def _copia_id(self, _w):
        if not self.me:
            return
        Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).set_text(self.me, -1)
        self.btn_copia.set_label(_t("chat.copiato"))
        GLib.timeout_add(1400, lambda: (self.btn_copia.set_label(_t("chat.copia")), False)[1])


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
