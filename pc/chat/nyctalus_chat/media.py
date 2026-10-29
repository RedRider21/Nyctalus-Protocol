# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Allegati in stile Telegram: disegno delle bolle multimediali, miniature dei
video, lettore audio dentro la chat, fotocamera (foto e video) e finestra di
anteprima prima dell'invio.

Niente qui fa crittografia: produce solo file locali che la finestra passa al
motore con SENDFILE.
"""
import hashlib
import math
import os
import shutil
import subprocess
import threading
import time
from pathlib import Path

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, GdkPixbuf, GLib, Gtk, Pango  # noqa: E402

try:
    gi.require_version("Gst", "1.0")
    from gi.repository import Gst  # noqa: E402
    Gst.init(None)
except Exception:                # noqa: BLE001
    Gst = None

# Lato massimo (in pixel logici) di foto e miniature video nelle bolle.
LATO_FOTO = 320
CACHE = Path(GLib.get_user_cache_dir()) / "nyctalus-chat"

# Colori del cerchio per tipo di file (come le icone colorate di Telegram).
COLORI_TIPO = {
    "pdf": "#e05a4f", "archivio": "#e09a3e", "documento": "#4f86e0", "foglio": "#43a565",
    "presentazione": "#e07a3e", "audio": "#9a6ef0", "video": "#e05a8a", "immagine": "#3fa7c9",
    "altro": "#6b7797",
}


def famiglia(mime, nome=""):
    ext = os.path.splitext(nome)[1].lower().lstrip(".")
    if mime == "application/pdf" or ext == "pdf":
        return "pdf"
    if mime.startswith("audio/"):
        return "audio"
    if mime.startswith("video/"):
        return "video"
    if mime.startswith("image/"):
        return "immagine"
    if ext in ("zip", "7z", "rar", "tar", "gz", "xz", "bz2", "zst") or "zip" in mime or "compressed" in mime:
        return "archivio"
    if ext in ("xls", "xlsx", "ods", "csv") or "sheet" in mime:
        return "foglio"
    if ext in ("ppt", "pptx", "odp") or "presentation" in mime:
        return "presentazione"
    if ext in ("doc", "docx", "odt", "rtf", "txt", "md") or mime.startswith("text/") or "document" in mime:
        return "documento"
    return "altro"


def estensione(nome):
    ext = os.path.splitext(nome)[1].lstrip(".").upper()
    return ext[:4] if ext else "FILE"


def durata_leggibile(sec):
    if sec is None:
        return ""
    sec = int(round(sec))
    h, r = divmod(sec, 3600)
    m, s = divmod(r, 60)
    return "%d:%02d:%02d" % (h, m, s) if h else "%d:%02d" % (m, s)


def _rgb(colore):
    colore = colore.lstrip("#")
    return tuple(int(colore[i:i + 2], 16) / 255 for i in (0, 2, 4))


def _rettangolo_arrotondato(cr, x, y, w, h, r):
    r = min(r, w / 2, h / 2)
    cr.new_sub_path()
    cr.arc(x + w - r, y + r, r, -math.pi / 2, 0)
    cr.arc(x + w - r, y + h - r, r, 0, math.pi / 2)
    cr.arc(x + r, y + h - r, r, math.pi / 2, math.pi)
    cr.arc(x + r, y + r, r, math.pi, 3 * math.pi / 2)
    cr.close_path()


def _pillola(cr, testo, x, y, in_basso_a_destra=False, w_area=0, h_area=0):
    """Etichetta scura semitrasparente (ora, durata) disegnata sopra le foto."""
    cr.select_font_face("Sans", 0, 0)
    cr.set_font_size(11)
    est = cr.text_extents(testo)
    pw, ph = est.width + 14, 18
    if in_basso_a_destra:
        x, y = w_area - pw - 8, h_area - ph - 8
    cr.set_source_rgba(0, 0, 0, 0.45)
    _rettangolo_arrotondato(cr, x, y, pw, ph, 9)
    cr.fill()
    cr.set_source_rgba(1, 1, 1, 0.95)
    cr.move_to(x + 7 - est.x_bearing, y + ph / 2 - est.y_bearing - est.height / 2)
    cr.show_text(testo)


def dimensione_logica(percorso, lato=LATO_FOTO):
    """Dimensione con cui mostrare un'immagine: sta in lato×lato, mai ingrandita."""
    info = GdkPixbuf.Pixbuf.get_file_info(percorso)
    if not info or not info[0]:
        return None
    w, h = info[1], info[2]
    if w <= 0 or h <= 0:
        return None
    f = min(1.0, lato / w, lato / h)
    return max(60, int(w * f)), max(60, int(h * f))


class ImmagineArrotondata(Gtk.DrawingArea):
    """Foto o fotogramma di un video con angoli arrotondati; sopra può avere
    l'ora (in basso a destra), la durata (in alto a sinistra) e il tasto ▶."""

    def __init__(self, percorso=None, w=LATO_FOTO, h=200, raggio=14, ora="", durata="", play=False):
        super().__init__()
        self.pixbuf, self.raggio, self.ora, self.durata, self.play = None, raggio, ora, durata, play
        self.set_size_request(w, h)
        self.w, self.h = w, h
        if percorso:
            self.imposta(percorso)
        self.connect("draw", self._disegna)

    def imposta(self, percorso, w=None, h=None):
        if w and h:
            self.w, self.h = w, h
            self.set_size_request(w, h)
        try:   # caricata al doppio: nitida anche sugli schermi HiDPI
            self.pixbuf = GdkPixbuf.Pixbuf.new_from_file_at_scale(percorso, self.w * 2, self.h * 2, True)
            self.pixbuf = self.pixbuf.apply_embedded_orientation() or self.pixbuf
        except Exception:        # noqa: BLE001
            self.pixbuf = None
        self.queue_draw()

    def _disegna(self, _w, cr):
        w, h = self.get_allocated_width(), self.get_allocated_height()
        _rettangolo_arrotondato(cr, 0, 0, w, h, self.raggio)
        cr.clip()
        if self.pixbuf is not None:
            pw, ph = self.pixbuf.get_width(), self.pixbuf.get_height()
            s = max(w / pw, h / ph)                       # riempie (taglia l'eccesso)
            cr.save()
            cr.translate((w - pw * s) / 2, (h - ph * s) / 2)
            cr.scale(s, s)
            Gdk.cairo_set_source_pixbuf(cr, self.pixbuf, 0, 0)
            cr.paint()
            cr.restore()
        else:
            cr.set_source_rgb(0.12, 0.15, 0.22)
            cr.paint()
        if self.play:
            r = min(28, w / 5)
            cr.set_source_rgba(0, 0, 0, 0.45)
            cr.arc(w / 2, h / 2, r, 0, 2 * math.pi)
            cr.fill()
            cr.set_source_rgba(1, 1, 1, 0.95)
            cr.move_to(w / 2 - r * 0.32, h / 2 - r * 0.45)
            cr.line_to(w / 2 + r * 0.48, h / 2)
            cr.line_to(w / 2 - r * 0.32, h / 2 + r * 0.45)
            cr.close_path()
            cr.fill()
        if self.durata:
            _pillola(cr, self.durata, 8, 8)
        if self.ora:
            _pillola(cr, self.ora, 0, 0, True, w, h)
        return False


class CerchioFile(Gtk.DrawingArea):
    """Cerchio colorato con l'estensione (o ▶ ⏸ per l'audio); durante il
    trasferimento disegna attorno l'anello di avanzamento."""

    def __init__(self, colore, testo="", simbolo=None, diametro=46):
        super().__init__()
        self.colore, self.testo, self.simbolo, self.frazione = colore, testo, simbolo, None
        self.set_size_request(diametro, diametro)
        self.set_valign(Gtk.Align.CENTER)
        self.connect("draw", self._disegna)

    def imposta(self, frazione=None, simbolo=None):
        self.frazione = frazione
        if simbolo is not None:
            self.simbolo = simbolo
        self.queue_draw()

    def _disegna(self, _w, cr):
        w, h = self.get_allocated_width(), self.get_allocated_height()
        cx, cy, r = w / 2, h / 2, min(w, h) / 2 - 1
        cr.set_source_rgb(*_rgb(self.colore))
        cr.arc(cx, cy, r, 0, 2 * math.pi)
        cr.fill()
        cr.set_source_rgba(1, 1, 1, 1)
        if self.frazione is not None:                    # anello di avanzamento
            cr.set_line_width(3)
            cr.set_source_rgba(1, 1, 1, 0.30)
            cr.arc(cx, cy, r - 6, 0, 2 * math.pi)
            cr.stroke()
            cr.set_source_rgba(1, 1, 1, 1)
            cr.arc(cx, cy, r - 6, -math.pi / 2, -math.pi / 2 + 2 * math.pi * max(0.02, self.frazione))
            cr.stroke()
        elif self.simbolo == "play":
            cr.move_to(cx - r * 0.25, cy - r * 0.38)
            cr.line_to(cx + r * 0.40, cy)
            cr.line_to(cx - r * 0.25, cy + r * 0.38)
            cr.close_path()
            cr.fill()
        elif self.simbolo == "pausa":
            for dx in (-r * 0.22, r * 0.08):
                cr.rectangle(cx + dx, cy - r * 0.34, r * 0.16, r * 0.68)
            cr.fill()
        else:
            cr.select_font_face("Sans", 0, 1)
            cr.set_font_size(11 if len(self.testo) <= 3 else 9.5)
            est = cr.text_extents(self.testo)
            cr.move_to(cx - est.width / 2 - est.x_bearing, cy - est.height / 2 - est.y_bearing)
            cr.show_text(self.testo)
        return False


# --- miniature e durata dei video (ffmpeg/ffprobe, in sottofondo, con cache) ---------
def _ffprobe_durata(percorso):
    if not shutil.which("ffprobe"):
        return None
    try:
        out = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format=duration",
                              "-of", "default=nw=1:nk=1", percorso],
                             capture_output=True, text=True, timeout=10).stdout.strip()
        return float(out) if out else None
    except Exception:            # noqa: BLE001
        return None


def miniatura_video(percorso, pronto):
    """Estrae in un thread un fotogramma e la durata; chiama pronto(jpg, durata)
    nel thread di GTK. Risultati in cache (per percorso+data di modifica)."""
    def lavoro():
        jpg, durata = None, None
        try:
            st = os.stat(percorso)
            chiave = hashlib.sha1(("%s|%d|%d" % (percorso, st.st_size, int(st.st_mtime))).encode()).hexdigest()
            cartella = CACHE / "miniature"
            cartella.mkdir(parents=True, exist_ok=True)
            dest = cartella / (chiave + ".jpg")
            durata = _ffprobe_durata(percorso)
            if not dest.exists() and shutil.which("ffmpeg"):
                istante = "1" if (durata or 0) > 2 else "0"
                subprocess.run(["ffmpeg", "-v", "error", "-y", "-ss", istante, "-i", percorso,
                                "-frames:v", "1", "-vf", "scale=640:-2", str(dest)],
                               capture_output=True, timeout=20)
            if dest.exists():
                jpg = str(dest)
        except Exception:        # noqa: BLE001
            pass
        GLib.idle_add(lambda: (pronto(jpg, durata), False)[1])
    threading.Thread(target=lavoro, daemon=True).start()


def durata_in_sottofondo(percorso, pronto):
    def lavoro():
        d = _ffprobe_durata(percorso)
        GLib.idle_add(lambda: (pronto(d), False)[1])
    threading.Thread(target=lavoro, daemon=True).start()


# --- lettore audio dentro la chat ------------------------------------------------
class LettoreAudio:
    """Un solo brano alla volta, come in Telegram: avviarne uno ferma l'altro."""

    def __init__(self):
        self.pb = Gst.ElementFactory.make("playbin", "lettore") if Gst else None
        self.attuale = None          # (percorso, aggiorna(frazione, pos, durata, suona))
        self.timer = 0
        if self.pb is not None:
            bus = self.pb.get_bus()
            bus.add_signal_watch()
            bus.connect("message::eos", lambda *_a: self.ferma())
            bus.connect("message::error", lambda *_a: self.ferma())

    def disponibile(self):
        return self.pb is not None

    def alterna(self, percorso, aggiorna):
        if self.pb is None:
            return False
        if self.attuale and self.attuale[0] == percorso:
            _ok, stato, _p = self.pb.get_state(0)
            if stato == Gst.State.PLAYING:
                self.pb.set_state(Gst.State.PAUSED)
                aggiorna(None, None, None, False)
            else:
                self.pb.set_state(Gst.State.PLAYING)
                aggiorna(None, None, None, True)
            return True
        self.ferma()
        self.attuale = (percorso, aggiorna)
        self.pb.set_property("uri", GLib.filename_to_uri(percorso, None))
        self.pb.set_state(Gst.State.PLAYING)
        aggiorna(0.0, 0, None, True)
        self.timer = GLib.timeout_add(250, self._tick)
        return True

    def _tick(self):
        if not self.attuale:
            return False
        ok1, pos = self.pb.query_position(Gst.Format.TIME)
        ok2, dur = self.pb.query_duration(Gst.Format.TIME)
        if ok1 and ok2 and dur > 0:
            self.attuale[1](pos / dur, pos / Gst.SECOND, dur / Gst.SECOND, None)
        return True

    def ferma(self):
        if self.timer:
            GLib.source_remove(self.timer)
            self.timer = 0
        if self.pb is not None:
            self.pb.set_state(Gst.State.NULL)
        if self.attuale:
            self.attuale[1](None, None, None, False)
            self.attuale = None


# --- fotocamera ------------------------------------------------------------------
def sorgente_video():
    # NYCTALUS_CAMERA_SRC permette di provare senza webcam (es. "videotestsrc is-live=true")
    return os.environ.get("NYCTALUS_CAMERA_SRC", "v4l2src device=%s" % os.environ.get("NYCTALUS_CAMERA_DEV", "/dev/video0"))


def sorgente_audio():
    return os.environ.get("NYCTALUS_MIC_SRC", "autoaudiosrc")


def fotocamera_disponibile():
    if Gst is None or Gst.ElementFactory.find("gtksink") is None:
        return False
    if "NYCTALUS_CAMERA_SRC" in os.environ:
        return True
    return os.path.exists(os.environ.get("NYCTALUS_CAMERA_DEV", "/dev/video0"))


class Fotocamera(Gtk.Dialog):
    """Anteprima dal vivo; in modalità «foto» scatta un'immagine, in modalità
    «video» registra (con l'audio del microfono se c'è). run() → percorso o None."""

    def __init__(self, genitore, modalita, testi):
        super().__init__(title=testi["titolo_" + modalita], transient_for=genitore, modal=True)
        self.modalita, self.t, self.risultato = modalita, testi, None
        self.pipeline, self.vista, self.registra_su, self.inizio = None, None, None, 0
        self.set_default_size(720, 520)
        self.get_style_context().add_class("fotocamera")
        area = self.get_content_area()
        self.cornice = Gtk.Box()
        self.cornice.set_vexpand(True)
        area.pack_start(self.cornice, True, True, 0)
        barra = Gtk.Box(spacing=12); barra.set_border_width(12)
        self.stato = Gtk.Label(xalign=0); self.stato.get_style_context().add_class("cam-stato")
        barra.pack_start(self.stato, True, True, 0)
        annulla = Gtk.Button(label=testi["annulla"]); annulla.connect("clicked", lambda *_a: self.response(Gtk.ResponseType.CANCEL))
        barra.pack_end(annulla, False, False, 0)
        self.scatto = Gtk.Button(label=testi["scatta"] if modalita == "foto" else testi["registra"])
        self.scatto.get_style_context().add_class("cam-scatto")
        self.scatto.connect("clicked", self._premuto)
        barra.pack_end(self.scatto, False, False, 0)
        area.pack_end(barra, False, False, 0)
        self.connect("response", lambda *_a: self._chiudi())
        self._avvia(registrazione=False)
        self.show_all()

    # costruisce la pipeline: anteprima specchiata + (foto) cattura fotogramma / (video) codifica su file
    def _avvia(self, registrazione):
        self._ferma_pipeline()
        anteprima = "t. ! queue ! videoflip method=horizontal-flip ! videoconvert ! gtksink name=vista sync=false"
        if self.modalita == "foto":
            ramo = "t. ! queue leaky=downstream max-size-buffers=1 ! videoconvert ! gdkpixbufsink name=presa post-messages=false"
        elif registrazione:
            ramo = ("t. ! queue ! videoconvert ! x264enc tune=zerolatency speed-preset=veryfast bitrate=2500 key-int-max=60 "
                    "! h264parse ! queue ! mp4mux name=mux faststart=true ! filesink location=\"%s\"" % self.registra_su)
            if Gst.ElementFactory.find("avenc_aac"):
                ramo += " %s ! queue ! audioconvert ! audioresample ! avenc_aac ! aacparse ! queue ! mux." % sorgente_audio()
        else:
            ramo = ""
        descr = "%s ! videoconvert ! videoscale ! video/x-raw,width=[320,1280] ! tee name=t %s %s" % (sorgente_video(), anteprima, ramo)
        try:
            self.pipeline = Gst.parse_launch(descr)
        except Exception:        # noqa: BLE001
            # alcune camere rifiutano il vincolo di larghezza: si riprova senza
            self.pipeline = Gst.parse_launch(descr.replace("! videoscale ! video/x-raw,width=[320,1280] ", ""))
        sink = self.pipeline.get_by_name("vista")
        for f in self.cornice.get_children():
            self.cornice.remove(f)
        self.vista = sink.props.widget
        self.vista.set_hexpand(True); self.vista.set_vexpand(True)
        self.cornice.pack_start(self.vista, True, True, 0)
        self.cornice.show_all()
        bus = self.pipeline.get_bus()
        bus.add_signal_watch()
        bus.connect("message::error", self._errore)
        if self.pipeline.set_state(Gst.State.PLAYING) == Gst.StateChangeReturn.FAILURE:
            self.stato.set_text(self.t["errore"])
            self.scatto.set_sensitive(False)

    def _errore(self, _bus, msg):
        err, _dbg = msg.parse_error()
        self.stato.set_text("%s (%s)" % (self.t["errore"], err.message))
        self.scatto.set_sensitive(False)

    def _premuto(self, _b):
        if self.modalita == "foto":
            px = self.pipeline.get_by_name("presa").props.last_pixbuf
            if px is None:
                self.stato.set_text(self.t["attendi"])
                return
            dest = _file_temporaneo("foto", ".jpg")
            px.savev(dest, "jpeg", ["quality"], ["92"])
            self.risultato = dest
            self.response(Gtk.ResponseType.OK)
        elif self.registra_su is None:        # inizia a registrare
            self.registra_su = _file_temporaneo("video", ".mp4")
            self._avvia(registrazione=True)
            self.inizio = time.time()
            self.scatto.set_label(self.t["stop"])
            self.scatto.get_style_context().add_class("registrando")
            GLib.timeout_add(500, self._timer)
        else:                                 # ferma: EOS e attesa che mp4mux chiuda il file
            self.scatto.set_sensitive(False)
            self.pipeline.send_event(Gst.Event.new_eos())
            self.pipeline.get_bus().timed_pop_filtered(5 * Gst.SECOND, Gst.MessageType.EOS | Gst.MessageType.ERROR)
            self._ferma_pipeline()
            if os.path.exists(self.registra_su) and os.path.getsize(self.registra_su) > 0:
                self.risultato = self.registra_su
                self.response(Gtk.ResponseType.OK)

    def _timer(self):
        if self.registra_su is None or self.pipeline is None:
            return False
        self.stato.set_markup('<span foreground="#ef5b5b">●</span> %s' % durata_leggibile(time.time() - self.inizio))
        return True

    def _ferma_pipeline(self):
        if self.pipeline is not None:
            self.pipeline.set_state(Gst.State.NULL)
            self.pipeline = None

    def _chiudi(self):
        self._ferma_pipeline()


def _file_temporaneo(prefisso, ext):
    cartella = CACHE / "fotocamera"
    cartella.mkdir(parents=True, exist_ok=True)
    return str(cartella / ("%s-%s%s" % (prefisso, time.strftime("%Y%m%d-%H%M%S"), ext)))


# --- anteprima prima dell'invio ---------------------------------------------------
class AnteprimaInvio(Gtk.Dialog):
    """Come «Invia foto» di Telegram: miniature, didascalia, foto senza metadati.
    run() == OK → self.didascalia(), self.senza_exif()."""

    def __init__(self, genitore, voci, testi, exif_predefinito, dim_leggibile):
        # voci: [(percorso, mime)]
        super().__init__(title=testi["titolo"] % len(voci) if len(voci) > 1 else testi["titolo1"],
                         transient_for=genitore, modal=True)
        self.get_style_context().add_class("anteprima-invio")
        self.set_default_size(440, -1)
        area = self.get_content_area()
        area.set_spacing(10); area.set_border_width(14)
        griglia = Gtk.FlowBox(); griglia.set_selection_mode(Gtk.SelectionMode.NONE)
        griglia.set_max_children_per_line(3); griglia.set_column_spacing(6); griglia.set_row_spacing(6)
        file_box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=8)
        immagini = [v for v in voci if v[1].startswith(("image/", "video/"))]
        for percorso, mime in voci:
            nome = os.path.basename(percorso)
            if mime.startswith("image/") and mime != "image/svg+xml" and dimensione_logica(percorso):
                lato = 400 if len(immagini) == 1 else 128
                w, h = dimensione_logica(percorso, lato)
                if len(immagini) > 1:
                    w = h = lato
                griglia.add(ImmagineArrotondata(percorso, w, h, raggio=10))
            elif mime.startswith("video/"):
                lato = 400 if len(immagini) == 1 else 128
                img = ImmagineArrotondata(None, lato, int(lato * 9 / 16) if len(immagini) == 1 else lato, raggio=10, play=True)
                miniatura_video(percorso, lambda jpg, d, img=img: (jpg and img.imposta(jpg), setattr(img, "durata", durata_leggibile(d)), img.queue_draw()))
                griglia.add(img)
            else:
                riga = Gtk.Box(spacing=12)
                fam = famiglia(mime, nome)
                riga.pack_start(CerchioFile(COLORI_TIPO[fam], estensione(nome)), False, False, 0)
                col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
                n = Gtk.Label(label=nome, xalign=0); n.set_ellipsize(Pango.EllipsizeMode.MIDDLE); n.set_max_width_chars(34)
                n.get_style_context().add_class("nome-file")
                d = Gtk.Label(label=dim_leggibile(os.path.getsize(percorso)), xalign=0); d.get_style_context().add_class("dim-file")
                col.pack_start(n, False, False, 0); col.pack_start(d, False, False, 0)
                riga.pack_start(col, True, True, 0)
                file_box.pack_start(riga, False, False, 0)
        if immagini:
            area.pack_start(griglia, False, False, 0)
        area.pack_start(file_box, False, False, 0)
        self.voce_didascalia = Gtk.Entry()
        self.voce_didascalia.set_placeholder_text(testi["didascalia"])
        self.voce_didascalia.set_activates_default(True)
        area.pack_start(self.voce_didascalia, False, False, 0)
        self.exif = Gtk.CheckButton(label=testi["exif"])
        self.exif.set_active(exif_predefinito)
        if any(m in ("image/jpeg", "image/png") for _p, m in voci):
            area.pack_start(self.exif, False, False, 0)
        self.add_button(testi["annulla"], Gtk.ResponseType.CANCEL)
        invia = self.add_button(testi["invia"], Gtk.ResponseType.OK)
        invia.get_style_context().add_class("suggested-action")
        self.set_default_response(Gtk.ResponseType.OK)
        self.show_all()
        self.voce_didascalia.grab_focus()

    def didascalia(self):
        return self.voce_didascalia.get_text().strip()

    def senza_exif(self):
        return self.exif.get_active()
