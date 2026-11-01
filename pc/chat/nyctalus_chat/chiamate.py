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
MUTA_MAX = 15             # secondi senza voce dall'altro: la chiamata si chiude


def durata(sec):
    sec = int(sec)
    return "%d:%02d" % (sec // 60, sec % 60) if sec < 3600 else "%d:%02d:%02d" % (sec // 3600, sec % 3600 // 60, sec % 60)


class Chiamate:
    def __init__(self, chat, t):
        self.chat, self._t = chat, t
        self.c = None             # chiamata in corso: dict(addr, id, stato, audio, inizio, uscente)
        self.finestra_arrivo = None
        self.timer = 0
        self.suono = None             # squillo (in arrivo) o tono d'attesa (in uscita)

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
        # 📹 passa al video / 📞 torna alla sola voce (la nostra telecamera)
        video = self.tasto_video = Gtk.Button(label="📹")
        video.set_tooltip_text(t("chat.ch.passa_video"))
        video.connect("clicked", lambda _b: self.c and self.c["stato"] == "attiva" and self.video(not self.c.get("video_mio")))
        self.finestra_video = None
        self.muto = Gtk.ToggleButton(label="🎤 " + t("chat.ch.muto"))
        self.muto.connect("toggled", lambda b: (self.c and self.c.get("audio") and self.c["audio"].muto(b.get_active()),
                                                self._aggiorna_finestra()))
        fine = Gtk.Button(label="✆ " + t("chat.ch.riaggancia"))
        fine.get_style_context().add_class("destructive-action")
        fine.connect("clicked", lambda _b: self.riaggancia())
        apri = Gtk.Button(label="⤢ " + t("chat.ch.apri"))
        apri.set_tooltip_text(t("chat.ch.apri_nota"))
        apri.connect("clicked", lambda _b: self.mostra_finestra())
        box.pack_start(self.etichetta, True, True, 0)
        box.pack_start(apri, False, False, 0)
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
    def chiama(self, addr, video=False):
        if self.c:
            return self._barra_testo(self._t("chat.ch.gia"))
        if not media.audio_chiamate_disponibile():
            return self.chat._avviso_inline("⚠ " + self._t("chat.ch.senza_audio"))
        self.c = {"addr": addr, "id": None, "stato": "chiamo", "audio": None, "inizio": time.time(), "uscente": True,
                  "vuole_video": bool(video) and media.video_chiamate_disponibile()}
        self.chat._cmd("CALL %s%s" % (addr, " v" if self.c["vuole_video"] else ""))
        self._suona("uscita")
        self._mostra()
        self.timer = GLib.timeout_add_seconds(1, self._tick)

    def _suona(self, quale):
        self._zitto()
        try:
            self.suono = media.Suoneria(quale)
        except Exception:   # noqa: BLE001
            self.suono = None

    def _zitto(self):
        if self.suono:
            self.suono.ferma()
            self.suono = None

    def riaggancia(self, motivo=None):
        c, self.c = self.c, None
        self._zitto()
        if not c:
            return
        if c.get("id"):
            self.chat._cmd("CALLEND %s %s" % (c["addr"], c["id"]))
        if c.get("audio"):
            c["audio"].ferma()
        if c.get("video"):
            c["video"].ferma()
        self._chiudi_video()
        self._chiudi_ui()
        testo = motivo if motivo is not None else (self._t("chat.ch.finita") + (" · " + durata(time.time() - c["inizio"]) if c["stato"] == "attiva" else ""))
        if testo and c["addr"] == self.chat.attivo:            # "" = in silenzio
            self.chat._avviso_inline("📞 " + testo)

    def _accetta(self, addr, cid, video=False):
        self._chiudi_arrivo()
        self._zitto()
        self.c = {"addr": addr, "id": cid, "stato": "attiva", "audio": None, "inizio": time.time(), "uscente": False}
        self.chat._cmd("CALLOK %s %s" % (addr, cid))
        self._avvia_audio()
        if video:
            self.video(True)
        self.chat.scrivi_a(addr, self.chat.contatti.get(addr, addr[:10]))
        self._mostra()
        self.timer = GLib.timeout_add_seconds(1, self._tick)

    def _rifiuta(self, addr, cid):
        self._chiudi_arrivo()
        self._zitto()
        self.chat._cmd("CALLNO %s %s" % (addr, cid))

    # --- eventi del motore -------------------------------------------------------------
    def evento(self, tipo, p):
        """True se l'evento era di una chiamata (gestito qui)."""
        if tipo in ("VID", "VIDKEY", "VIDSTATO"):
            if self.c and self.c["stato"] == "attiva" and len(p) >= 2 and p[1] == self.c["addr"]:
                v = self._video_obj()
                if tipo == "VID" and len(p) >= 5 and v:
                    try:
                        v.ricevi(int(p[2]), p[3] == "1", bytes.fromhex(p[4]))
                    except ValueError:
                        pass
                elif tipo == "VIDKEY" and v:
                    v.fotogramma_completo()
                elif tipo == "VIDSTATO" and len(p) >= 3:
                    self.c["video_suo"] = p[2] == "1"
                    self._aggiorna_video()
            return True
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
            if self.c and self.c["addr"] == addr and self.c.get("id") != cid:
                self.c["id"] = None                    # ci richiama: la chiamata vecchia era morta
                self.riaggancia("")
            if self.c or self.finestra_arrivo:
                self.chat._cmd("CALLNO %s %s" % (addr, cid))          # occupato
            else:
                self._arrivo(addr, cid, len(p) >= 4 and p[3] == "v")
        elif tipo == "CALLOK" and self.c and self.c["id"] == cid and self.c["stato"] == "chiamo":
            self._zitto()
            self.c.update(stato="attiva", inizio=time.time())
            self._avvia_audio()
            if self.c.get("vuole_video"):
                self.video(True)
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
                self._zitto()
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
        self.c["muta"] = self.c.get("muta", 0) + 1 if dr == 0 else 0
        if self.c["muta"] >= MUTA_MAX:
            return self.riaggancia(self._t("chat.ch.muta"))
        if dr == 0:
            testo = "📵 " + self._t("chat.ch.q_nessuna")
        else:
            perdita = dp / (dr + dp)
            testo = "📶 " + self._t("chat.ch.q_buona" if perdita < 0.03 else "chat.ch.q_media" if perdita < 0.12 else "chat.ch.q_scarsa")
        self.qualita.set_text(testo)
        self._aggiorna_finestra()

    def _barra_testo(self, extra=None):
        if not self.c:
            return
        nome = self.chat.contatti.get(self.c["addr"], self.c["addr"][:10])
        if self.c["stato"] == "chiamo":
            t = "📞 %s %s…" % (self._t("chat.ch.chiamo"), nome)
        else:
            t = "📞 %s · %s" % (nome, durata(time.time() - self.c["inizio"]))
        self.etichetta.set_text(t + (" — " + extra if extra else ""))
        self._aggiorna_finestra()

    def _mostra(self):
        self.muto.set_active(False)
        self._barra_testo()
        self.barra.show_all()
        self.barra.set_reveal_child(True)
        self.mostra_finestra()

    def _chiudi_ui(self):
        self.barra.set_reveal_child(False)
        if self.timer:
            GLib.source_remove(self.timer)
            self.timer = 0

    def _arrivo(self, addr, cid, video=False):
        nome = self.chat.contatti.get(addr, addr[:10])
        w = Gtk.Window(title=self._t("chat.ch.video_in_arrivo" if video else "chat.ch.in_arrivo"))
        w.cid = cid
        w.set_transient_for(self.chat)
        w.set_keep_above(True)
        w.set_default_size(320, 140)
        w.get_style_context().add_class("finestra-rubrica")
        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=12)
        box.set_border_width(18)
        lab = Gtk.Label()
        lab.set_markup("<big>%s</big>\n<b>%s</b>\n%s" % ("📹" if video else "📞", GLib.markup_escape_text(nome),
                       GLib.markup_escape_text(self._t("chat.ch.ti_videochiama" if video else "chat.ch.ti_chiama"))))
        lab.set_justify(Gtk.Justification.CENTER)
        riga = Gtk.Box(spacing=10, homogeneous=True)
        no = Gtk.Button(label=self._t("chat.ch.rifiuta"))
        no.get_style_context().add_class("destructive-action")
        si = Gtk.Button(label=("📹 " if video else "📞 ") + self._t("chat.ch.rispondi"))
        si.get_style_context().add_class("suggested-action")
        no.connect("clicked", lambda _b: self._rifiuta(addr, cid))
        si.connect("clicked", lambda _b: self._accetta(addr, cid, video))
        riga.pack_start(no, True, True, 0)
        if video:                                  # come Telegram: si può rispondere anche solo con la voce
            voce = Gtk.Button(label="📞 " + self._t("chat.ch.solo_voce"))
            voce.connect("clicked", lambda _b: self._accetta(addr, cid, False))
            riga.pack_start(voce, True, True, 0)
        riga.pack_start(si, True, True, 0)
        box.pack_start(lab, True, True, 0)
        box.pack_start(riga, False, False, 0)
        w.add(box)
        w.connect("delete-event", lambda *_a: (self._rifiuta(addr, cid), True)[1])
        self.finestra_arrivo = w
        self._suona("arrivo")
        w.show_all()
        w.present()
        GLib.timeout_add_seconds(SQUILLO_MAX, lambda: (self.finestra_arrivo is w and self._rifiuta(addr, cid), False)[1])
        if shutil.which("notify-send") and not self.chat.is_active():
            subprocess.Popen(["notify-send", "-a", "Nyctalus", "-u", "critical", "-i", "nyctalus", "--",
                              "📞 " + nome, self._t("chat.ch.ti_chiama")],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    # --- video ---------------------------------------------------------------------------
    def _video_obj(self):
        if not self.c:
            return None
        if not self.c.get("video") and media.video_chiamate_disponibile():
            addr = self.c["addr"]
            v = media.VideoChiamata(
                lambda seq, completo, dati: self.chat._cmd("VID %s %d %d %s" % (addr, seq, 1 if completo else 0, dati.hex())),
                lambda: self.chat._cmd("VIDKEY %s" % addr))
            self.c["video"] = v
        return self.c.get("video")

    def video(self, accesa):
        """📹 accende la nostra telecamera (videochiamata) · 📞 la spegne (solo voce)."""
        if not self.c:
            return
        v = self._video_obj()
        if not v:
            return self.chat._avviso_inline("⚠ " + self._t("chat.ch.senza_video"))
        try:
            v.accendi() if accesa else v.spegni()
        except Exception as e:   # noqa: BLE001
            accesa = False
            self.chat._avviso_inline("⚠ %s (%s)" % (self._t("chat.ch.senza_webcam"), e))
        self.c["video_mio"] = accesa
        self.chat._cmd("VIDSTATO %s %d" % (self.c["addr"], 1 if accesa else 0))
        self._aggiorna_video()

    # --- finestra della chiamata (come Telegram Web) -----------------------------------
    # icone vettoriali di sistema (prendono il colore del testo); l'emoji solo se mancano
    ICONE = {"video": ("camera-video-symbolic", "📹"), "voce": ("call-start-symbolic", "📞"),
             "audio": ("audio-volume-high-symbolic", "🔊"), "mic": ("microphone-sensitivity-high-symbolic", "🎤"),
             "muto": ("microphone-sensitivity-muted-symbolic", "🎤"), "fine": ("call-stop-symbolic", "✆")}

    def _icona_tonda(self, nome, classe=""):
        """Cerchio con l'icona; .imposta(nome) la cambia."""
        box = Gtk.Box()
        box.get_style_context().add_class("tondo")
        for c in classe.split():
            box.get_style_context().add_class(c)
        img, lab = Gtk.Image(), Gtk.Label()
        img.set_pixel_size(24)
        for x in (img, lab):
            x.set_halign(Gtk.Align.CENTER); x.set_valign(Gtk.Align.CENTER)
            box.pack_start(x, True, True, 0)
        tema = Gtk.IconTheme.get_default()

        def imposta(n):
            icona, emoji = self.ICONE[n]
            if tema.has_icon(icona):
                img.set_from_icon_name(icona, Gtk.IconSize.LARGE_TOOLBAR); img.set_pixel_size(24)
                img.show(); lab.hide()
            else:
                lab.set_text(emoji); lab.show(); img.hide()
        box.imposta = imposta
        img.set_no_show_all(True); lab.set_no_show_all(True)
        imposta(nome)
        return box

    def _tasto_tondo(self, icona, testo, cb, classe=""):
        b = Gtk.Button()
        b.get_style_context().add_class("tondo-btn")
        col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=7)
        ic = self._icona_tonda(icona, classe)
        lab = Gtk.Label(label=testo)
        lab.get_style_context().add_class("tondo-testo")
        col.pack_start(ic, False, False, 0)
        col.pack_start(lab, False, False, 0)
        b.add(col)
        b.icona, b.testo = ic, lab
        b.connect("clicked", lambda _b: cb())
        return b

    def _finestra(self):
        """Una sola finestra per voce e video: avatar grande con la voce; il video dell'altro a
        tutto sfondo e la propria anteprima piccola in alto a destra con la telecamera."""
        if self.finestra_video or not self.c:
            return self.finestra_video
        c = self.c
        nome = self.chat.contatti.get(c["addr"], c["addr"][:10])
        w = Gtk.Window(title="📞 " + nome)
        w.set_default_size(420, 600)
        w.get_style_context().add_class("finestra-chiamata")
        sovr = Gtk.Overlay()
        self._schermo = Gtk.Stack()
        self._schermo.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        voce = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        voce.set_valign(Gtk.Align.CENTER)
        from nyctalus_chat.app import fai_avatar
        av = fai_avatar(nome, c["addr"], True)
        av.get_style_context().add_class("avatar-chiamata")
        av.set_size_request(124, 124)
        av.set_halign(Gtk.Align.CENTER)
        voce.pack_start(av, False, False, 0)
        self._schermo.add_named(voce, "voce")
        sovr.add(self._schermo)
        # in alto: nome, stato/durata, qualità della linea
        testa = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        testa.set_valign(Gtk.Align.START)
        testa.set_margin_top(18)
        self._w_nome = Gtk.Label(label=nome)
        self._w_nome.get_style_context().add_class("chiamata-nome")
        self._w_stato = Gtk.Label()
        self._w_stato.get_style_context().add_class("chiamata-stato")
        self._w_qualita = Gtk.Label()
        self._w_qualita.get_style_context().add_class("chiamata-stato")
        for x in (self._w_nome, self._w_stato, self._w_qualita):
            testa.pack_start(x, False, False, 0)
        sovr.add_overlay(testa)
        # in alto a destra: la propria anteprima (con la telecamera accesa)
        self._angolo = Gtk.Box()
        self._angolo.get_style_context().add_class("anteprima-video")
        self._angolo.set_halign(Gtk.Align.END)
        self._angolo.set_valign(Gtk.Align.START)
        self._angolo.set_margin_end(14)
        self._angolo.set_margin_top(14)
        self._angolo.set_hexpand(False)
        self._angolo.set_vexpand(False)
        self._angolo.set_no_show_all(True)
        sovr.add_overlay(self._angolo)
        # in basso: i tasti tondi
        tasti = Gtk.Box(spacing=18)
        tasti.set_halign(Gtk.Align.CENTER)
        tasti.set_valign(Gtk.Align.END)
        tasti.set_margin_bottom(26)
        self._t_video = self._tasto_tondo("video", self._t("chat.ch.video"), lambda: self.c and self.c["stato"] == "attiva"
                                          and self.video(not self.c.get("video_mio")))
        audio = Gtk.MenuButton()
        audio.get_style_context().add_class("tondo-btn")
        col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=6)
        ic = self._icona_tonda("audio")
        lab = Gtk.Label(label=self._t("chat.ch.audio")); lab.get_style_context().add_class("tondo-testo")
        col.pack_start(ic, False, False, 0); col.pack_start(lab, False, False, 0)
        audio.add(col)
        audio.set_popover(self._popover_audio(audio))
        self._t_muto = self._tasto_tondo("mic", self._t("chat.ch.muto"), lambda: self.muto.set_active(not self.muto.get_active()))
        fine = self._tasto_tondo("fine", self._t("chat.ch.termina"), lambda: self.riaggancia(), "rosso")
        for b in (self._t_video, audio, self._t_muto, fine):
            tasti.pack_start(b, False, False, 0)
        sovr.add_overlay(tasti)
        w.add(sovr)
        # chiudere la finestra = ridurla: la chiamata continua (barra verde nella chat)
        w.connect("delete-event", lambda *_a: (w.hide(), True)[1])
        self.finestra_video = w
        w.show_all()
        self._angolo.hide()
        return w

    def mostra_finestra(self):
        w = self._finestra()
        if w:
            w.show()
            w.present()

    def _aggiorna_finestra(self):
        if not self.finestra_video or not self.c:
            return
        c = self.c
        if c["stato"] == "chiamo":
            self._w_stato.set_text(self._t("chat.ch.chiamo") + "…")
        else:
            self._w_stato.set_text(durata(time.time() - c["inizio"]))
        self._w_qualita.set_text(self.qualita.get_text())
        muto = self.muto.get_active()
        (self._t_muto.icona.get_style_context().add_class if muto else self._t_muto.icona.get_style_context().remove_class)("attivo")
        self._t_muto.icona.imposta("muto" if muto else "mic")

    def _aggiorna_video(self):
        c = self.c
        if not c:
            return
        mio, suo = bool(c.get("video_mio")), bool(c.get("video_suo"))
        self.tasto_video.set_label("📞" if mio else "📹")
        self.tasto_video.set_tooltip_text(self._t("chat.ch.solo_voce" if mio else "chat.ch.passa_video"))
        w = self._finestra()
        if not w:
            return
        v = c.get("video")
        # in voce: «📹 Video»; in video: «📞 Solo voce» (come chiesto: si passa nei due sensi)
        self._t_video.icona.imposta("voce" if mio else "video")
        self._t_video.testo.set_text(self._t("chat.ch.solo_voce" if mio else "chat.ch.video"))
        if v and self._schermo.get_child_by_name("video") is None:
            self._schermo.add_named(v.schermo, "video")
        if v:
            v.schermo.show()                       # il widget di GStreamer non si mostra da solo
        self._schermo.set_visible_child_name("video" if (suo and v) else "voce")
        if v and suo:
            v.avvia()                              # ora che lo schermo è nella finestra
        for f in self._angolo.get_children():
            self._angolo.remove(f)
        if mio and v and v.anteprima:
            if v.anteprima.get_parent():
                v.anteprima.get_parent().remove(v.anteprima)
            self._angolo.pack_start(v.anteprima, True, True, 0)
            v.anteprima.show()
            self._angolo.show()
            v.trasmetti()                          # ora che l'anteprima è nella finestra
        else:
            self._angolo.hide()
        if mio or suo:
            w.resize(640, 600) if w.get_size()[0] < 600 else None
            w.present()

    def _chiudi_video(self):
        if self.finestra_video:
            w, self.finestra_video = self.finestra_video, None
            if self.c and self.c.get("video"):
                for f in (self.c["video"].schermo, self.c["video"].anteprima):
                    if f is not None and f.get_parent():
                        f.get_parent().remove(f)
            w.destroy()

    def _chiudi_arrivo(self):
        if self.finestra_arrivo:
            w, self.finestra_arrivo = self.finestra_arrivo, None
            w.destroy()
