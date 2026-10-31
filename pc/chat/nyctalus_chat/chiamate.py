# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Chiamate vocali nella chat del PC (stile Telegram).

Il motore porta segnali e voce dentro la sessione cifrata con il contatto, attraverso la rete
Nyctalus (niente collegamento diretto: nessuno vede l'IP dell'altro):
  comandi  CALL addr · CALLOK|CALLNO|CALLEND addr id · AUD addr seq hex
  eventi   CALLID addr id · CALLIN addr id · CALLOK|CALLNO|CALLEND addr id · AUD addr seq hex
Qui: il pulsante 📞, la finestra «chiamata in arrivo», la barra della chiamata (durata, 🎤 muto,
riaggancia) e la voce (media.AudioChiamata: microfono → Opus 20 ms, altoparlante).
"""
import shutil
import subprocess
import time

import gi
gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk  # noqa: E402

from nyctalus_chat import media  # noqa: E402

SQUILLO_MAX = 45          # secondi: poi «nessuna risposta» / invito lasciato cadere


def durata(sec):
    sec = int(sec)
    return "%d:%02d" % (sec // 60, sec % 60) if sec < 3600 else "%d:%02d:%02d" % (sec // 3600, sec % 3600 // 60, sec % 60)


class Chiamate:
    def __init__(self, chat, t):
        self.chat, self._t = chat, t
        self.c = None             # chiamata in corso: dict(addr, id, stato, audio, inizio, uscente)
        self.finestra_arrivo = None
        self.timer = 0

        # pulsante 📞 nell'intestazione della conversazione
        self.pulsante = Gtk.Button(label="📞")
        self.pulsante.get_style_context().add_class("puntini")
        self.pulsante.set_tooltip_text(t("chat.ch.chiama"))
        self.pulsante.set_valign(Gtk.Align.CENTER)
        self.pulsante.connect("clicked", lambda _b: self.chat.attivo and self.chiama(self.chat.attivo))

        # barra della chiamata (sotto l'intestazione)
        self.barra = Gtk.Revealer()
        box = Gtk.Box(spacing=10)
        box.set_border_width(8)
        box.get_style_context().add_class("barra-chiamata")
        self.etichetta = Gtk.Label(xalign=0)
        self.qualita = Gtk.Label(xalign=1)
        self.qualita.get_style_context().add_class("stato-riga")
        self.uscita = None                  # None = altoparlante predefinito del sistema
        self.volume = 1.0
        audio = Gtk.MenuButton(label="🔊 " + t("chat.ch.audio"))
        audio.set_tooltip_text(t("chat.ch.audio_nota"))
        audio.set_popover(self._popover_audio(audio))
        video = Gtk.Button(label="🎥")
        video.set_sensitive(False)
        video.set_tooltip_text(t("chat.ch.video_presto"))
        self.muto = Gtk.ToggleButton(label="🎤 " + t("chat.ch.muto"))
        self.muto.connect("toggled", lambda b: self.c and self.c.get("audio") and self.c["audio"].muto(b.get_active()))
        fine = Gtk.Button(label="✆ " + t("chat.ch.riaggancia"))
        fine.get_style_context().add_class("destructive-action")
        fine.connect("clicked", lambda _b: self.riaggancia())
        box.pack_start(self.etichetta, True, True, 0)
        box.pack_end(fine, False, False, 0)
        box.pack_end(self.muto, False, False, 0)
        box.pack_end(audio, False, False, 0)
        box.pack_end(video, False, False, 0)
        box.pack_end(self.qualita, False, False, 0)
        self.barra.add(box)

    def _popover_audio(self, rif):
        """🔊: volume di chi parla e uscita (vivavoce/altoparlante o cuffie), anche durante la chiamata."""
        pop = Gtk.Popover.new(rif)
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        box.set_border_width(10)
        box.pack_start(Gtk.Label(label=self._t("chat.ch.volume"), xalign=0), False, False, 0)
        scala = Gtk.Scale.new_with_range(Gtk.Orientation.HORIZONTAL, 0, 200, 5)
        scala.set_value(100)
        scala.set_size_request(220, -1)
        scala.connect("value-changed", lambda w: self._volume(w.get_value() / 100))
        box.pack_start(scala, False, False, 0)
        box.pack_start(Gtk.Label(label=self._t("chat.ch.uscita"), xalign=0), False, False, 0)
        self.box_uscite = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        box.pack_start(self.box_uscite, False, False, 0)
        box.show_all()
        pop.add(box)
        pop.connect("show", lambda _p: self._riempi_uscite())
        return pop

    def _riempi_uscite(self):
        for f in self.box_uscite.get_children():
            self.box_uscite.remove(f)
        primo = Gtk.RadioButton.new_with_label(None, self._t("chat.ch.predefinita"))
        primo.set_active(self.uscita is None)
        primo.connect("toggled", lambda b: b.get_active() and self._scegli_uscita(None))
        self.box_uscite.pack_start(primo, False, False, 0)
        for nome, descr in media.uscite_audio():
            b = Gtk.RadioButton.new_with_label_from_widget(primo, descr)
            b.set_active(self.uscita == nome)
            b.connect("toggled", lambda w, n=nome: w.get_active() and self._scegli_uscita(n))
            self.box_uscite.pack_start(b, False, False, 0)
        self.box_uscite.show_all()

    def _scegli_uscita(self, nome):
        self.uscita = nome
        if self.c and self.c.get("audio"):
            self.c["audio"].cambia_uscita(nome)

    def _volume(self, v):
        self.volume = v
        if self.c and self.c.get("audio"):
            self.c["audio"].imposta_volume(v)

    # --- comandi dell'utente -------------------------------------------------------------
    def chiama(self, addr):
        if self.c:
            return self._barra_testo(self._t("chat.ch.gia"))
        if not media.audio_chiamate_disponibile():
            return self.chat._avviso_inline("⚠ " + self._t("chat.ch.senza_audio"))
        self.c = {"addr": addr, "id": None, "stato": "chiamo", "audio": None, "inizio": time.time(), "uscente": True}
        self.chat._cmd("CALL %s" % addr)
        self._mostra()
        self.timer = GLib.timeout_add_seconds(1, self._tick)

    def riaggancia(self, motivo=None):
        c, self.c = self.c, None
        if not c:
            return
        if c.get("id"):
            self.chat._cmd("CALLEND %s %s" % (c["addr"], c["id"]))
        if c.get("audio"):
            c["audio"].ferma()
        self._chiudi_ui()
        testo = motivo or (self._t("chat.ch.finita") + (" · " + durata(time.time() - c["inizio"]) if c["stato"] == "attiva" else ""))
        if c["addr"] == self.chat.attivo:
            self.chat._avviso_inline("📞 " + testo)

    def _accetta(self, addr, cid):
        self._chiudi_arrivo()
        self.c = {"addr": addr, "id": cid, "stato": "attiva", "audio": None, "inizio": time.time(), "uscente": False}
        self.chat._cmd("CALLOK %s %s" % (addr, cid))
        self._avvia_audio()
        self.chat.scrivi_a(addr, self.chat.contatti.get(addr, addr[:10]))
        self._mostra()
        self.timer = GLib.timeout_add_seconds(1, self._tick)

    def _rifiuta(self, addr, cid):
        self._chiudi_arrivo()
        self.chat._cmd("CALLNO %s %s" % (addr, cid))

    # --- eventi del motore -------------------------------------------------------------
    def evento(self, tipo, p):
        """True se l'evento era di una chiamata (gestito qui)."""
        if tipo == "AUD" and len(p) >= 4:
            if self.c and self.c["stato"] == "attiva" and p[1] == self.c["addr"] and self.c.get("audio"):
                try:
                    self.c["audio"].ricevi(int(p[2]), bytes.fromhex(p[3]))
                except ValueError:
                    pass
            return True
        if tipo not in ("CALLID", "CALLIN", "CALLOK", "CALLNO", "CALLEND") or len(p) < 3:
            return False
        addr, cid = p[1], p[2]
        if tipo == "CALLID" and self.c and self.c["addr"] == addr and not self.c["id"]:
            self.c["id"] = cid
            self._barra_testo()
        elif tipo == "CALLIN":
            if self.c or self.finestra_arrivo:
                self.chat._cmd("CALLNO %s %s" % (addr, cid))          # occupato
            else:
                self._arrivo(addr, cid)
        elif tipo == "CALLOK" and self.c and self.c["id"] == cid and self.c["stato"] == "chiamo":
            self.c.update(stato="attiva", inizio=time.time())
            self._avvia_audio()
            self._barra_testo()
        elif tipo == "CALLNO" and self.c and self.c["id"] == cid:
            self.c["id"] = None
            self.riaggancia(self._t("chat.ch.rifiutata"))
        elif tipo == "CALLEND":
            if self.c and self.c["id"] == cid:
                self.c["id"] = None                    # l'ha chiusa l'altro: niente CALLEND di ritorno
                self.riaggancia()
            elif self.finestra_arrivo and getattr(self.finestra_arrivo, "cid", None) == cid:
                self._chiudi_arrivo()
                self.chat._avviso_inline("📞 " + self._t("chat.ch.persa"))
        return True

    def contatto_offline(self, addr):
        if self.c and self.c["addr"] == addr:
            self.riaggancia(self._t("chat.ch.caduta"))

    # --- interni ----------------------------------------------------------------------
    def _avvia_audio(self):
        addr = self.c["addr"]
        try:
            self.c["audio"] = media.AudioChiamata(lambda seq, dati: self.chat._cmd("AUD %s %d %s" % (addr, seq, dati.hex())),
                                                  self.uscita, self.volume)
            self.c["contati"] = (0, 0, time.time())
            self.c["audio"].avvia()
            self.c["audio"].muto(self.muto.get_active())
        except Exception as e:   # noqa: BLE001
            self.riaggancia("⚠ %s (%s)" % (self._t("chat.ch.senza_audio"), e))

    def _tick(self):
        if not self.c:
            self.timer = 0
            return False
        if self.c["stato"] == "chiamo" and time.time() - self.c["inizio"] > SQUILLO_MAX:
            self.timer = 0
            self.riaggancia(self._t("chat.ch.nessuna_risposta"))
            return False
        self._barra_testo()
        self._qualita()
        return True

    def _qualita(self):
        """📶 dai pacchetti dell'ultimo secondo: arrivati, persi o in ritardo."""
        a = self.c.get("audio") if self.c else None
        if not a or self.c["stato"] != "attiva":
            self.qualita.set_text("")
            return
        ric0, per0, t0 = self.c.get("contati", (0, 0, time.time()))
        ric, per = a.ricevuti, a.persi + a.scartati
        self.c["contati"] = (ric, per, time.time())
        dr, dp = ric - ric0, per - per0
        if dr == 0:
            testo = "📵 " + self._t("chat.ch.q_nessuna")
        else:
            perdita = dp / (dr + dp)
            testo = "📶 " + self._t("chat.ch.q_buona" if perdita < 0.03 else "chat.ch.q_media" if perdita < 0.12 else "chat.ch.q_scarsa")
        self.qualita.set_text(testo)

    def _barra_testo(self, extra=None):
        if not self.c:
            return
        nome = self.chat.contatti.get(self.c["addr"], self.c["addr"][:10])
        if self.c["stato"] == "chiamo":
            t = "📞 %s %s…" % (self._t("chat.ch.chiamo"), nome)
        else:
            t = "📞 %s · %s" % (nome, durata(time.time() - self.c["inizio"]))
        self.etichetta.set_text(t + (" — " + extra if extra else ""))

    def _mostra(self):
        self.muto.set_active(False)
        self._barra_testo()
        self.barra.show_all()
        self.barra.set_reveal_child(True)

    def _chiudi_ui(self):
        self.barra.set_reveal_child(False)
        if self.timer:
            GLib.source_remove(self.timer)
            self.timer = 0

    def _arrivo(self, addr, cid):
        nome = self.chat.contatti.get(addr, addr[:10])
        w = Gtk.Window(title=self._t("chat.ch.in_arrivo"))
        w.cid = cid
        w.set_transient_for(self.chat)
        w.set_keep_above(True)
        w.set_default_size(320, 140)
        w.get_style_context().add_class("finestra-rubrica")
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_border_width(18)
        lab = Gtk.Label()
        lab.set_markup("<big>📞</big>\n<b>%s</b>\n%s" % (GLib.markup_escape_text(nome), GLib.markup_escape_text(self._t("chat.ch.ti_chiama"))))
        lab.set_justify(Gtk.Justification.CENTER)
        riga = Gtk.Box(spacing=10, homogeneous=True)
        no = Gtk.Button(label=self._t("chat.ch.rifiuta"))
        no.get_style_context().add_class("destructive-action")
        si = Gtk.Button(label=self._t("chat.ch.rispondi"))
        si.get_style_context().add_class("suggested-action")
        no.connect("clicked", lambda _b: self._rifiuta(addr, cid))
        si.connect("clicked", lambda _b: self._accetta(addr, cid))
        riga.pack_start(no, True, True, 0)
        riga.pack_start(si, True, True, 0)
        box.pack_start(lab, True, True, 0)
        box.pack_start(riga, False, False, 0)
        w.add(box)
        w.connect("delete-event", lambda *_a: (self._rifiuta(addr, cid), True)[1])
        self.finestra_arrivo = w
        w.show_all()
        w.present()
        GLib.timeout_add_seconds(SQUILLO_MAX, lambda: (self.finestra_arrivo is w and self._rifiuta(addr, cid), False)[1])
        if shutil.which("notify-send") and not self.chat.is_active():
            subprocess.Popen(["notify-send", "-a", "Nyctalus", "-u", "critical", "-i", "nyctalus", "--",
                              "📞 " + nome, self._t("chat.ch.ti_chiama")],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def _chiudi_arrivo(self):
        if self.finestra_arrivo:
            w, self.finestra_arrivo = self.finestra_arrivo, None
            w.destroy()
