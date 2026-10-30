# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Finestra della RUBRICA della rete (PC): iscriversi con un nome, vedere chi usa la rete
(in linea / ultimo accesso), aggiungere e scrivere con un clic.

Parla con la rubrica attraverso il motore della chat (messaggi JSON verso il suo indirizzo
.nyct): la chat cifra e autentica, quindi la rubrica sa con certezza chi chiede cosa."""
import gi
gi.require_version("Gtk", "3.0")
from gi.repository import Gtk, Pango  # noqa: E402


class FinestraRubrica(Gtk.Window):
    def __init__(self, chat, t, avatar, ultimo_accesso):
        super().__init__(title=t("chat.rub.titolo"))
        self.chat, self._t, self._avatar, self._ultimo = chat, t, avatar, ultimo_accesso
        self.set_transient_for(chat)
        self.set_default_size(420, 560)
        self.get_style_context().add_class("chat-root")
        self.get_style_context().add_class("finestra-rubrica")
        self.dati = None
        scatola = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=0)
        testa = Gtk.Box(spacing=8)
        testa.set_border_width(12)
        tit = Gtk.Label(xalign=0)
        tit.set_markup("<b>👥 %s</b>" % t("chat.rub.titolo"))
        tit.get_style_context().add_class("app-nome")
        aggiorna = Gtk.Button(label="⟳")
        aggiorna.get_style_context().add_class("pill")
        aggiorna.set_tooltip_text(t("chat.rub.aggiorna"))
        aggiorna.connect("clicked", lambda _b: self.chiedi())
        testa.pack_start(tit, True, True, 0)
        testa.pack_end(aggiorna, False, False, 0)
        scatola.pack_start(testa, False, False, 0)
        self.io = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        self.io.set_border_width(12)
        self.io.get_style_context().add_class("idcard")
        scatola.pack_start(self.io, False, False, 0)
        self.elenco = Gtk.ListBox()
        self.elenco.set_selection_mode(Gtk.SelectionMode.NONE)
        self.elenco.get_style_context().add_class("lista")
        scorri = Gtk.ScrolledWindow()
        scorri.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        scorri.add(self.elenco)
        scatola.pack_start(scorri, True, True, 0)
        self.add(scatola)
        self._messaggio(t("chat.rub.attendi"))
        self.show_all()
        self.chiedi()

    def chiedi(self):
        self.chat.chiedi_rubrica({"op": "elenco"})

    def _messaggio(self, testo):
        for f in self.io.get_children():
            self.io.remove(f)
        lab = Gtk.Label(label=testo, xalign=0)
        lab.set_line_wrap(True)
        self.io.pack_start(lab, False, False, 0)
        self.io.show_all()

    def mostra(self, dati, mio_nome):
        """Risposta «elenco» della rubrica."""
        self.dati = dati
        t = self._t
        for f in self.io.get_children():
            self.io.remove(f)
        if dati.get("iscritto"):
            self.io.pack_start(Gtk.Label(label=t("chat.rub.sei_iscritto", nome=mio_nome or "…"), xalign=0), False, False, 0)
            riga = Gtk.Box(spacing=8)
            cambia = Gtk.Button(label="✏️ " + t("chat.rub.cambia_nome"))
            cambia.connect("clicked", lambda _b: self._chiedi_nome(mio_nome or ""))
            via = Gtk.Button(label=t("chat.rub.cancellati"))
            via.get_style_context().add_class("destructive-action")
            via.connect("clicked", lambda _b: self.chat.chiedi_rubrica({"op": "cancella"}))
            riga.pack_start(cambia, True, True, 0)
            riga.pack_start(via, True, True, 0)
            self.io.pack_start(riga, False, False, 0)
        else:
            nota = Gtk.Label(label=t("chat.rub.nota"), xalign=0)
            nota.set_line_wrap(True)
            nota.get_style_context().add_class("stato-riga")
            self.io.pack_start(nota, False, False, 0)
            iscr = Gtk.Button(label=t("chat.rub.iscriviti"))
            iscr.get_style_context().add_class("suggested-action")
            iscr.connect("clicked", lambda _b: self._chiedi_nome(""))
            self.io.pack_start(iscr, False, False, 0)
        self.io.show_all()
        for f in self.elenco.get_children():
            self.elenco.remove(f)
        altri = [u for u in dati.get("utenti", []) if u.get("a") != self.chat.me]
        if not altri:
            vuoto = Gtk.Label(label=t("chat.rub.vuoto"))
            vuoto.set_margin_top(18)
            vuoto.get_style_context().add_class("stato-riga")
            self.elenco.add(vuoto)
        for u in altri:
            self.elenco.add(self._riga(u))
        self.elenco.show_all()

    def _riga(self, u):
        t = self._t
        riga = Gtk.ListBoxRow()
        box = Gtk.Box(spacing=12)
        box.set_border_width(8)
        av = self._avatar(u["n"], u["a"])
        if u.get("on"):
            av.get_style_context().add_class("online")
        box.pack_start(av, False, False, 0)
        col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        n = Gtk.Label(label=u["n"], xalign=0)
        n.get_style_context().add_class("nome")
        n.set_ellipsize(Pango.EllipsizeMode.END)
        s = Gtk.Label(label=t("chat.online") if u.get("on") else self._ultimo(u.get("v")), xalign=0)
        s.get_style_context().add_class("stato-riga")
        col.pack_start(n, False, False, 0)
        col.pack_start(s, False, False, 0)
        box.pack_start(col, True, True, 0)
        gia = u["a"] in self.chat.contatti
        scrivi = Gtk.Button(label=("💬 " + t("chat.rub.scrivi")) if gia else ("＋ " + t("chat.rub.aggiungi")))
        scrivi.set_valign(1)
        scrivi.connect("clicked", lambda _b: self._scrivi(u))
        box.pack_end(scrivi, False, False, 0)
        riga.add(box)
        return riga

    def _scrivi(self, u):
        self.chat.scrivi_a(u["a"], u["n"])
        self.chat.present()

    def _chiedi_nome(self, attuale):
        t = self._t
        d = Gtk.Dialog(title=t("chat.rub.nome_titolo"), transient_for=self, modal=True)
        d.add_button(t("chat.annulla") if t("chat.annulla") != "chat.annulla" else "Annulla", Gtk.ResponseType.CANCEL)
        d.add_button(t("chat.rub.salva"), Gtk.ResponseType.OK)
        d.set_default_response(Gtk.ResponseType.OK)
        e = Gtk.Entry()
        e.set_max_length(40)
        e.set_text(attuale)
        e.set_activates_default(True)
        area = d.get_content_area()
        area.set_border_width(12)
        area.set_spacing(8)
        area.pack_start(Gtk.Label(label=t("chat.rub.nome_nota"), xalign=0), False, False, 0)
        area.pack_start(e, False, False, 0)
        d.show_all()
        if d.run() == Gtk.ResponseType.OK and e.get_text().strip():
            self.chat.chiedi_rubrica({"op": "iscrivi", "nome": e.get_text().strip()})
        d.destroy()
