# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Nyctalus · File: scambio di file grandi con i contatti, attraverso la rete Nyctalus.

Programma a sé (comando `nyctalus-file`, pulsante «File» del mini menu): si collega allo
stesso motore della chat (porta di controllo + segreto, da ambiente o dal file privato
`~/.nyctalus-chat/controllo`) e usa i comandi dei trasferimenti:
    → XSEND <addr> <percorso hex> [mime] · XOK|XNO|XPAUSA|XRIPRENDI|XANNULLA <id> · XLIST · XCONF
    ← XNEW · XSTATO · XPROG · XFINE  (e C <addr> <nome hex> per i contatti)
I file mandati e ricevuti compaiono anche nella conversazione della chat.
"""
import mimetypes
import os
import shutil
import socket
import subprocess
import threading
import time
from pathlib import Path
from urllib.parse import unquote, urlparse

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, GLib, Gtk, Pango  # noqa: E402

try:
    from nxs_i18n import t as _t_base
except Exception:                # noqa: BLE001
    _t_base = None
try:
    import nxs_tema
except Exception:                # noqa: BLE001
    nxs_tema = None

_ITA = {
    "file.titolo": "File",
    "file.sottotitolo": "Scambio di file attraverso Nyctalus",
    "file.invia": "Invia file…",
    "file.in_arrivo": "In arrivo",
    "file.in_uscita": "In uscita",
    "file.vuoto_in": "Nessun file in arrivo.\nQuando un contatto ti manda un file, lo trovi qui.",
    "file.vuoto_out": "Nessun file inviato.\nTocca «Invia file…» o trascina qui dei file.",
    "file.a": "a {nome}",
    "file.da": "da {nome}",
    "file.st.prepara": "Preparo il file…",
    "file.st.attesa_out": "Aspetto che {nome} sia in linea",
    "file.st.attesa_in": "Riprende quando {nome} è in linea",
    "file.st.chiede": "Vuole mandarti questo file",
    "file.st.chiede_out": "Aspetto che {nome} accetti",
    "file.st.corre": "{velocita}/s · mancano {resta}",
    "file.st.corre0": "In corso…",
    "file.st.pausa": "In pausa",
    "file.st.fatto_in": "Arrivato",
    "file.st.fatto_out": "Consegnato",
    "file.st.errore": "Problema: {dettaglio}",
    "file.st.rifiutato": "Rifiutato",
    "file.st.annullato": "Annullato",
    "file.accetta": "Accetta",
    "file.rifiuta": "Rifiuta",
    "file.pausa": "Pausa",
    "file.riprendi": "Riprendi",
    "file.annulla": "Annulla",
    "file.apri": "Apri",
    "file.cartella": "Mostra nella cartella",
    "file.scegli_contatto": "A chi mandi i file?",
    "file.cerca": "Cerca un contatto",
    "file.scegli_file": "Scegli i file da mandare",
    "file.nessun_contatto": "Non hai ancora contatti: aggiungili dalla chat.",
    "file.motore_assente": "Il motore di Nyctalus non risponde: riprovo…",
    "file.impostazioni": "Impostazioni",
    "file.soglia": "Accetta da solo i file dei contatti fino a (MB)",
    "file.canali": "Collegamenti in parallelo (più veloce, più carico sulla rete)",
    "file.conferma_annulla": "Annullare «{nome}»?",
    "file.conferma_annulla_corpo": "Il trasferimento si ferma per entrambi; quello che è già arrivato viene buttato.",
    "file.notifica_chiede": "{nome} vuole mandarti «{file}» ({dim})",
    "file.notifica_arrivato": "Arrivato da {nome}: «{file}»",
    "file.chiudi": "Chiudi",
    "file.secondi": "{n} s",
    "file.minuti": "{n} min",
    "file.ore": "{n} h {m} min",
}


def _t(key, **kw):
    if _t_base is not None:
        v = _t_base(key, **kw)
        if v != key:
            return v
    testo = _ITA.get(key, key)
    try:
        return testo.format(**kw) if kw else testo
    except (KeyError, IndexError, ValueError):
        return testo


def misura(n):
    """Byte in forma leggibile, con la virgola: 3,4 MB."""
    n = float(n)
    for u in ("B", "KB", "MB", "GB", "TB"):
        if n < 1024 or u == "TB":
            return ("%d %s" % (n, u)) if u == "B" else ("%.1f %s" % (n, u)).replace(".", ",")
        n /= 1024
    return ""


def durata(s):
    s = int(s)
    if s < 60:
        return _t("file.secondi", n=max(s, 1))
    if s < 3600:
        return _t("file.minuti", n=(s + 59) // 60)
    return _t("file.ore", n=s // 3600, m=(s % 3600) // 60)


def icona_mime(mime):
    for prefisso, icona in (("image/", "image-x-generic"), ("video/", "video-x-generic"), ("audio/", "audio-x-generic"),
                            ("text/", "text-x-generic"), ("application/pdf", "x-office-document")):
        if mime.startswith(prefisso):
            return icona
    if any(k in mime for k in ("zip", "tar", "compress", "7z", "rar")):
        return "package-x-generic"
    return "text-x-generic-template"


def collegamento():
    """(porta, segreto) del motore: dall'ambiente, oppure dal file privato scritto dall'avvio."""
    porta = os.environ.get("NYCTALUS_CHAT_PORT")
    segreto = os.environ.get("NYCTALUS_CHAT_TOKEN")
    if not (porta and segreto):
        try:
            p, s = (Path.home() / ".nyctalus-chat" / "controllo").read_text().split()[:2]
            porta, segreto = porta or p, segreto or s
        except (OSError, ValueError):
            pass
    return int(porta or 1091), segreto or ""


CSS_TMPL = """
.nyct-file { background-color: @BG@; }
.nyct-file headerbar { background-image: none; background-color: @PANEL@; border-bottom: 1px solid rgba(@RGB@,0.14);
                       box-shadow: none; color: @TEXT@; }
.nyct-file headerbar .title { font-weight: 700; color: @TEXT@; }
.nyct-file headerbar .subtitle { color: @MUTED@; }
.nyct-file headerbar button { color: @TEXT@; }
.nyct-file .invia { background-image: none; background-color: @SEND@; color: #ffffff; border: none; border-radius: 10px;
                    padding: 6px 14px; font-weight: 600; }
.nyct-file .invia:hover { background-color: @SENDH@; }
.nyct-file stackswitcher button { background-image: none; background-color: transparent; border: none; box-shadow: none;
                                  color: @MUTED@; padding: 8px 18px; border-radius: 0; border-bottom: 2px solid transparent; }
.nyct-file stackswitcher button:checked { color: @TEXT@; border-bottom-color: @ACCENT@; }
.nyct-file list, .nyct-file scrolledwindow, .nyct-file viewport { background-color: @BG@; }
.nyct-file row.trasf { background-color: @PANEL@; border-radius: 14px; margin: 6px 14px; padding: 12px 14px;
                       border: 1px solid rgba(@RGB@,0.10); }
.nyct-file row.trasf:hover { border-color: rgba(@RGB@,0.28); }
.nyct-file .nome { color: @TEXT@; font-weight: 600; font-size: 14px; }
.nyct-file .sotto, .nyct-file .stato { color: @MUTED@; font-size: 12.5px; }
.nyct-file .stato.chiede { color: @ACCENT@; font-weight: 600; }
.nyct-file .stato.errore { color: #ef6b6b; }
.nyct-file .stato.fatto { color: #3ec46d; }
.nyct-file progressbar trough { min-height: 6px; border-radius: 3px; background-color: rgba(@RGB@,0.14); border: none; }
.nyct-file progressbar progress { min-height: 6px; border-radius: 3px; background-color: @ACCENT@; border: none; }
.nyct-file progressbar.pausa progress { background-color: @MUTED@; }
.nyct-file progressbar.fatto progress { background-color: #3ec46d; }
.nyct-file .azione { background-image: none; background-color: rgba(@RGB@,0.12); color: @TEXT@; border: none;
                     border-radius: 9px; padding: 4px 12px; box-shadow: none; }
.nyct-file .azione:hover { background-color: rgba(@RGB@,0.22); }
.nyct-file .azione.primaria { background-color: @SEND@; color: #ffffff; }
.nyct-file .azione.pericolo { color: #ef6b6b; }
.nyct-file .icona-file { color: @ACCENT@; }
.nyct-file .vuoto { color: @MUTED@; font-size: 14px; }
.nyct-file .avviso { background-color: rgba(239,107,107,0.14); color: @TEXT@; padding: 8px 14px; }
"""

# colori se il tema globale non c'è (blu notte)
_NOTTE = {"BG": "#0a0e1a", "PANEL": "#0e1424", "TEXT": "#e6ecff", "MUTED": "#6b7797", "ACCENT": "#5aa9ff",
          "RGB": "90,169,255", "SEND": "#2f6fe0", "SENDH": "#3b7cf0"}


def _css(nome=None):
    c = dict(_NOTTE)
    if nxs_tema is not None:
        c.update({k: str(v) for k, v in nxs_tema.colori(nome).items()})
    css = CSS_TMPL
    for k, v in c.items():
        css = css.replace("@%s@" % k, v)
    return css.encode()


class Riga(Gtk.ListBoxRow):
    """Un trasferimento: icona, nome, contatto e misura, barra, stato, azioni."""

    def __init__(self, app, d):
        super().__init__()
        self.app, self.d = app, d
        self.get_style_context().add_class("trasf")
        self.set_activatable(False)
        g = Gtk.Grid(column_spacing=12, row_spacing=4)
        self.icona = Gtk.Image.new_from_icon_name(icona_mime(d["mime"]), Gtk.IconSize.DND)
        self.icona.set_pixel_size(34)
        self.icona.get_style_context().add_class("icona-file")
        self.icona.set_valign(Gtk.Align.CENTER)
        g.attach(self.icona, 0, 0, 1, 3)
        self.nome = Gtk.Label(label=d["nome"], xalign=0, ellipsize=Pango.EllipsizeMode.MIDDLE, hexpand=True)
        self.nome.get_style_context().add_class("nome")
        g.attach(self.nome, 1, 0, 1, 1)
        self.sotto = Gtk.Label(xalign=1)
        self.sotto.get_style_context().add_class("sotto")
        g.attach(self.sotto, 2, 0, 1, 1)
        self.barra = Gtk.ProgressBar(hexpand=True)
        g.attach(self.barra, 1, 1, 2, 1)
        self.stato = Gtk.Label(xalign=0, ellipsize=Pango.EllipsizeMode.END, hexpand=True)
        self.stato.get_style_context().add_class("stato")
        g.attach(self.stato, 1, 2, 1, 1)
        self.azioni = Gtk.Box(spacing=6, halign=Gtk.Align.END)
        g.attach(self.azioni, 2, 2, 1, 1)
        self.add(g)
        self.aggiorna()

    def _bottone(self, testo, cb, *classi):
        b = Gtk.Button(label=testo)
        b.get_style_context().add_class("azione")
        for c in classi:
            b.get_style_context().add_class(c)
        b.connect("clicked", lambda _b: cb())
        self.azioni.pack_start(b, False, False, 0)

    def aggiorna(self):
        d, app = self.d, self.app
        chi = app.nome_di(d["peer"])
        uscita = d["verso"] == "out"
        self.sotto.set_text("%s · %s" % (_t("file.a" if uscita else "file.da", nome=chi), misura(d["dim"])))
        frazione = (d["fatti"] / d["dim"]) if d["dim"] else (1.0 if d["stato"] == "fatto" else 0.0)
        if d["stato"] == "fatto":
            frazione = 1.0
        self.barra.set_fraction(min(max(frazione, 0.0), 1.0))
        bc = self.barra.get_style_context()
        for c in ("pausa", "fatto"):
            bc.remove_class(c)
        if d["stato"] == "fatto":
            bc.add_class("fatto")
        elif d["stato"] != "corre":
            bc.add_class("pausa")
        st = d["stato"]
        testo = {
            "prepara": _t("file.st.prepara"),
            "attesa": _t("file.st.attesa_out" if uscita else "file.st.attesa_in", nome=chi),
            "chiede": _t("file.st.chiede_out" if uscita else "file.st.chiede", nome=chi),
            "pausa": _t("file.st.pausa"),
            "fatto": _t("file.st.fatto_out" if uscita else "file.st.fatto_in"),
            "errore": _t("file.st.errore", dettaglio=d.get("dettaglio") or "?"),
            "rifiutato": _t("file.st.rifiutato"),
            "annullato": _t("file.st.annullato"),
        }.get(st)
        if st == "corre":
            bps = d.get("bps", 0)
            if bps > 0 and d["dim"]:
                testo = _t("file.st.corre", velocita=misura(bps), resta=durata((d["dim"] - d["fatti"]) / bps))
            else:
                testo = _t("file.st.corre0")
        # a metà: quanto è già passato, poi lo stato (es. «12,0 MB · 3,1 MB/s · mancano 5 s»)
        if st in ("corre", "pausa", "attesa", "errore") and 0 < d["fatti"] < d["dim"]:
            testo = "%s  ·  %s" % (misura(d["fatti"]), testo)
        self.stato.set_text(testo or st)
        sc = self.stato.get_style_context()
        for c in ("chiede", "errore", "fatto"):
            (sc.add_class if st == c else sc.remove_class)(c)
        for b in self.azioni.get_children():
            self.azioni.remove(b)
        i = d["id"]
        if st == "chiede" and not uscita:
            self._bottone(_t("file.rifiuta"), lambda: app.comando("XNO", i), "pericolo")
            self._bottone(_t("file.accetta"), lambda: app.comando("XOK", i), "primaria")
        elif st in ("corre", "attesa"):
            self._bottone(_t("file.pausa"), lambda: app.comando("XPAUSA", i))
            self._bottone(_t("file.annulla"), lambda: app.annulla(d), "pericolo")
        elif st in ("pausa", "errore"):
            self._bottone(_t("file.riprendi"), lambda: app.comando("XRIPRENDI", i), "primaria")
            self._bottone(_t("file.annulla"), lambda: app.annulla(d), "pericolo")
        elif st == "fatto" and d.get("percorso"):
            self._bottone(_t("file.cartella"), lambda: app.mostra_cartella(d["percorso"]))
            if not uscita:
                self._bottone(_t("file.apri"), lambda: app.apri(d["percorso"]), "primaria")
        self.azioni.show_all()


class FileApp(Gtk.Window):
    def __init__(self):
        super().__init__(title="Nyctalus · " + _t("file.titolo"))
        self.set_default_size(760, 560)
        self.set_icon_name("nyctalus")
        self.get_style_context().add_class("nyct-file")
        self.prov = Gtk.CssProvider()
        self.prov.load_from_data(_css())
        Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(), self.prov, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        self._oss_tema = nxs_tema.osserva(lambda n: self.prov.load_from_data(_css(n))) if nxs_tema is not None else None

        self.contatti = {}       # addr → nome
        self.trasf = {}          # id → dati
        self.righe = {}          # id → Riga
        self.sock = None
        self.porta, self.segreto = collegamento()
        self._conf = {}

        hb = Gtk.HeaderBar(show_close_button=True, title=_t("file.titolo"), subtitle=_t("file.sottotitolo"))
        invia = Gtk.Button(label=_t("file.invia"))
        invia.get_style_context().add_class("invia")
        invia.connect("clicked", lambda _b: self.invia())
        hb.pack_start(invia)
        imp = Gtk.MenuButton()
        imp.set_image(Gtk.Image.new_from_icon_name("emblem-system-symbolic", Gtk.IconSize.BUTTON))
        imp.set_tooltip_text(_t("file.impostazioni"))
        imp.set_popover(self._popover_impostazioni(imp))
        hb.pack_end(imp)
        self.set_titlebar(hb)

        box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL)
        self.avviso = Gtk.Label(label=_t("file.motore_assente"), xalign=0)
        self.avviso.get_style_context().add_class("avviso")
        self.avviso.set_no_show_all(True)
        box.pack_start(self.avviso, False, False, 0)
        self.pila = Gtk.Stack(transition_type=Gtk.StackTransitionType.CROSSFADE)
        sw = Gtk.StackSwitcher(stack=self.pila, halign=Gtk.Align.CENTER)
        box.pack_start(sw, False, False, 6)
        self.liste = {}
        for verso, titolo, vuoto in (("in", "file.in_arrivo", "file.vuoto_in"), ("out", "file.in_uscita", "file.vuoto_out")):
            lb = Gtk.ListBox(selection_mode=Gtk.SelectionMode.NONE)
            lb.set_sort_func(lambda a, b: (b.d["ts"] > a.d["ts"]) - (b.d["ts"] < a.d["ts"]))
            seg = Gtk.Label(label=_t(vuoto), justify=Gtk.Justification.CENTER)
            seg.get_style_context().add_class("vuoto")
            seg.show()
            lb.set_placeholder(seg)
            sc = Gtk.ScrolledWindow(hscrollbar_policy=Gtk.PolicyType.NEVER)
            sc.add(lb)
            self.pila.add_titled(sc, verso, _t(titolo))
            self.liste[verso] = lb
        box.pack_start(self.pila, True, True, 0)
        self.add(box)

        # trascina qui dei file: si chiede a chi mandarli
        self.drag_dest_set(Gtk.DestDefaults.ALL, [Gtk.TargetEntry.new("text/uri-list", 0, 0)], Gdk.DragAction.COPY)
        self.connect("drag-data-received", self._trascinati)
        self.connect("destroy", Gtk.main_quit)
        self.show_all()
        self._prova_collegamento()

    # --- motore -----------------------------------------------------------------------------
    def _prova_collegamento(self):
        if self._connetti():
            self.avviso.hide()
            return False
        self.avviso.show()
        GLib.timeout_add_seconds(3, self._prova_collegamento)
        return False

    def _connetti(self):
        try:
            self.sock = socket.create_connection(("127.0.0.1", self.porta), timeout=4)
            self.sock.settimeout(None)
        except OSError:
            self.sock = None
            return False
        threading.Thread(target=self._lettore, daemon=True).start()
        if self.segreto:
            self.cmd("AUTH " + self.segreto)
        for c in ("LIST", "SUB", "XLIST", "XCONF"):
            self.cmd(c)
        return True

    def cmd(self, riga):
        if not self.sock:
            return
        try:
            self.sock.sendall((riga + "\n").encode())
        except OSError:
            self.sock = None

    def _lettore(self):
        buf = b""
        s = self.sock
        while True:
            try:
                d = s.recv(1 << 16)
            except OSError:
                d = b""
            if not d:
                GLib.idle_add(self._caduto)
                return
            buf += d
            while b"\n" in buf:
                riga, buf = buf.split(b"\n", 1)
                GLib.idle_add(self.evento, riga.decode(errors="replace"))

    def _caduto(self):
        self.sock = None
        self._prova_collegamento()
        return False

    def evento(self, riga):
        p = riga.split(" ")
        t = p[0]
        if t == "C" and len(p) >= 2:
            try:
                self.contatti[p[1]] = bytes.fromhex(p[2]).decode() if len(p) >= 3 and p[2] else p[1][:10]
            except ValueError:
                self.contatti[p[1]] = p[1][:10]
            for r in self.righe.values():
                if r.d["peer"] == p[1]:
                    r.aggiorna()
        elif t == "XNEW" and len(p) >= 9:
            try:
                nome = bytes.fromhex(p[6]).decode(errors="replace")
            except ValueError:
                nome = "?"
            d = self.trasf.get(p[1], {})
            nuovo = not d
            d.update({"id": p[1], "verso": p[2], "peer": p[3], "dim": int(p[4]), "mime": p[5], "nome": nome,
                      "stato": p[7], "fatti": int(p[8]), "ts": d.get("ts", time.time())})
            self.trasf[p[1]] = d
            self._mostra(d)
            if nuovo and d["verso"] == "in" and d["stato"] == "chiede":
                self._notifica(_t("file.notifica_chiede", nome=self.nome_di(d["peer"]), file=nome, dim=misura(d["dim"])))
                self.pila.set_visible_child_name("in")
        elif t == "XSTATO" and len(p) >= 3 and p[1] in self.trasf:
            d = self.trasf[p[1]]
            d["stato"] = p[2]
            if len(p) >= 4:
                try:
                    d["dettaglio"] = bytes.fromhex(p[3]).decode(errors="replace")
                except ValueError:
                    pass
            if p[2] == "fatto":
                d["fatti"] = d["dim"]
            self._mostra(d)
        elif t == "XPROG" and len(p) >= 5 and p[1] in self.trasf:
            d = self.trasf[p[1]]
            d["fatti"], d["bps"] = int(p[2]), int(p[4])
            self._mostra(d)
        elif t == "XFINE" and len(p) >= 3 and p[1] in self.trasf:
            d = self.trasf[p[1]]
            try:
                d["percorso"] = bytes.fromhex(p[2]).decode()
            except ValueError:
                pass
            d["stato"], d["fatti"] = "fatto", d["dim"]
            self._mostra(d)
            if d["verso"] == "in":
                self._notifica(_t("file.notifica_arrivato", nome=self.nome_di(d["peer"]), file=d["nome"]))
        elif t == "XCONF" and len(p) >= 5:
            self._conf = {"soglia": int(p[2]), "canali": int(p[4])}
            self._applica_conf()
        return False

    def _mostra(self, d):
        r = self.righe.get(d["id"])
        if r is None:
            r = Riga(self, d)
            self.righe[d["id"]] = r
            self.liste[d["verso"]].add(r)
            r.show_all()
        else:
            r.d = d
            r.aggiorna()

    def nome_di(self, addr):
        return self.contatti.get(addr) or addr[:10]

    # --- azioni -------------------------------------------------------------------------------
    def comando(self, c, ident):
        self.cmd("%s %s" % (c, ident))

    def annulla(self, d):
        dlg = Gtk.MessageDialog(transient_for=self, modal=True, message_type=Gtk.MessageType.QUESTION,
                                buttons=Gtk.ButtonsType.NONE, text=_t("file.conferma_annulla", nome=d["nome"]))
        dlg.format_secondary_text(_t("file.conferma_annulla_corpo"))
        dlg.add_buttons(_t("file.chiudi"), Gtk.ResponseType.CANCEL, _t("file.annulla"), Gtk.ResponseType.OK)
        if dlg.run() == Gtk.ResponseType.OK:
            self.comando("XANNULLA", d["id"])
        dlg.destroy()

    def apri(self, percorso):
        if shutil.which("xdg-open"):
            subprocess.Popen(["xdg-open", percorso], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    def mostra_cartella(self, percorso):
        self.apri(str(Path(percorso).parent))

    def _notifica(self, testo):
        if self.is_active() or not shutil.which("notify-send"):
            return
        try:
            subprocess.Popen(["notify-send", "-a", "Nyctalus", "-i", "nyctalus", "--", "Nyctalus · File", testo],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except OSError:
            pass

    def _trascinati(self, _w, _ctx, _x, _y, dati, _info, _tempo):
        file = []
        for u in dati.get_uris():
            p = urlparse(u)
            if p.scheme == "file" and os.path.isfile(unquote(p.path)):
                file.append(unquote(p.path))
        if file:
            self.invia(file)

    def invia(self, file=None):
        addr = self._scegli_contatto()
        if not addr:
            return
        if not file:
            fc = Gtk.FileChooserDialog(title=_t("file.scegli_file"), transient_for=self, action=Gtk.FileChooserAction.OPEN)
            fc.add_buttons(_t("file.chiudi"), Gtk.ResponseType.CANCEL, _t("file.invia").rstrip("…"), Gtk.ResponseType.OK)
            fc.set_select_multiple(True)
            file = fc.get_filenames() if fc.run() == Gtk.ResponseType.OK else []
            fc.destroy()
        for f in file:
            mime = mimetypes.guess_type(f)[0] or "application/octet-stream"
            self.cmd("XSEND %s %s %s" % (addr, f.encode().hex(), mime))
        if file:
            self.pila.set_visible_child_name("out")

    def _scegli_contatto(self):
        if not self.contatti:
            dlg = Gtk.MessageDialog(transient_for=self, modal=True, message_type=Gtk.MessageType.INFO,
                                    buttons=Gtk.ButtonsType.OK, text=_t("file.nessun_contatto"))
            dlg.run()
            dlg.destroy()
            return None
        dlg = Gtk.Dialog(title=_t("file.scegli_contatto"), transient_for=self, modal=True)
        dlg.set_default_size(380, 420)
        dlg.add_button(_t("file.chiudi"), Gtk.ResponseType.CANCEL)
        area = dlg.get_content_area()
        area.set_spacing(8)
        area.set_border_width(12)
        cerca = Gtk.SearchEntry(placeholder_text=_t("file.cerca"))
        area.pack_start(cerca, False, False, 0)
        lb = Gtk.ListBox()
        for addr, nome in sorted(self.contatti.items(), key=lambda x: x[1].lower()):
            r = Gtk.ListBoxRow()
            r.addr, r.nome = addr, nome
            b = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2, margin=8)
            n = Gtk.Label(label=nome, xalign=0)
            n.get_style_context().add_class("nome")
            a = Gtk.Label(label=addr[:20] + "…", xalign=0)
            a.get_style_context().add_class("sotto")
            b.pack_start(n, False, False, 0)
            b.pack_start(a, False, False, 0)
            r.add(b)
            lb.add(r)
        lb.set_filter_func(lambda r: cerca.get_text().lower() in r.nome.lower())
        cerca.connect("search-changed", lambda _e: lb.invalidate_filter())
        scelto = {}
        lb.connect("row-activated", lambda _l, r: (scelto.update(a=r.addr), dlg.response(Gtk.ResponseType.OK)))
        sc = Gtk.ScrolledWindow(vexpand=True)
        sc.add(lb)
        area.pack_start(sc, True, True, 0)
        dlg.show_all()
        esito = dlg.run()
        dlg.destroy()
        return scelto.get("a") if esito == Gtk.ResponseType.OK else None

    def _popover_impostazioni(self, bottone):
        pop = Gtk.Popover(relative_to=bottone)
        g = Gtk.Grid(column_spacing=12, row_spacing=10, margin=14)
        self.s_soglia = Gtk.SpinButton.new_with_range(0, 100000, 10)
        self.s_canali = Gtk.SpinButton.new_with_range(1, 8, 1)
        for i, (testo, w) in enumerate(((_t("file.soglia"), self.s_soglia), (_t("file.canali"), self.s_canali))):
            g.attach(Gtk.Label(label=testo, xalign=0, wrap=True, max_width_chars=34), 0, i, 1, 1)
            g.attach(w, 1, i, 1, 1)
        self.s_soglia.connect("value-changed", lambda w: self._cambia_conf("soglia", int(w.get_value()) * 1024 * 1024))
        self.s_canali.connect("value-changed", lambda w: self._cambia_conf("canali", int(w.get_value())))
        g.show_all()
        pop.add(g)
        return pop

    def _applica_conf(self):
        self._in_applica = True
        self.s_soglia.set_value(self._conf.get("soglia", 0) // (1024 * 1024))
        self.s_canali.set_value(self._conf.get("canali", 3))
        self._in_applica = False

    def _cambia_conf(self, chiave, valore):
        if getattr(self, "_in_applica", False) or self._conf.get(chiave) == valore:
            return
        self._conf[chiave] = valore
        self.cmd("XCONF %s %d" % (chiave, valore))


def main():
    FileApp()
    Gtk.main()
