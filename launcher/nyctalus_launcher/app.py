# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Mini-launcher da desktop di Nyctalus.

Un piccolo pannello senza bordi, **trascinabile** e collocabile ovunque sul
desktop, con **bottoni quadrati** disposti in orizzontale o in verticale (a
scelta, dal menu Impostazioni). Lancia i componenti di Nyctalus: il browser,
la chat e il gestore file (gli ultimi due arriveranno). Posizione e
orientamento vengono ricordati (`~/.nyctalus-launcher/config.json`).
"""
import json
import os
import subprocess
import sys
from pathlib import Path

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, GdkPixbuf, GLib, Gtk  # noqa: E402

# Tasti rapidi GLOBALI (funzionano anche quando il launcher non è a fuoco):
# serve la libreria Keybinder. Se manca, ripieghiamo su acceleratori locali
# (attivi solo con la finestra del launcher a fuoco).
try:
    gi.require_version("Keybinder", "3.0")
    from gi.repository import Keybinder  # noqa: E402
    Keybinder.init()
    HA_KEYBINDER = True
except Exception:                # noqa: BLE001
    Keybinder = None
    HA_KEYBINDER = False

# Traduzioni: riusa quelle del browser (nxs_i18n); fallback alla chiave.
try:
    from nxs_i18n import t as _t
except Exception:                # noqa: BLE001
    def _t(key, **kw):
        return key

RADICE = Path(__file__).resolve().parents[2]      # cartella del progetto / pacchetto
CARTELLA_BROWSER = RADICE / "browser"
LOGO = RADICE / "logo.svg"
PAGES = "https://redrider21.github.io/Nyctalus-Protocol/"
CONF_DIR = Path.home() / ".nyctalus-launcher"
CONF = CONF_DIR / "config.json"

# Dimensione dei bottoni (lato del quadrato, in px). Regolabile dalle
# impostazioni: "Più grande"/"Più piccola" cambiano a passi di PASSO_DIM.
DIM_MIN, DIM_MAX, DIM_DEF, PASSO_DIM = 36, 104, 50, 8

# Look piatto e pulito: pannello scuro arrotondato, bottoni senza bordo con solo
# un leggero evidenziatore al passaggio.
CSS = b"""
.nyct-launcher { background: #0e1221; border-radius: 16px;
                 border: 1px solid rgba(90,169,255,0.18); }
.nyct-launcher button { border: none; background: transparent; box-shadow: none;
                        border-radius: 12px; padding: 7px; margin: 0;
                        min-width: 0; min-height: 0;
                        color: #8a9bb8;                 /* icone tenui, leggibili ma non accese */
                        -gtk-icon-shadow: none;
                        transition: background 120ms ease, color 120ms ease; }
.nyct-launcher button image { color: #8a9bb8; }
.nyct-launcher button:hover { background: rgba(90,169,255,0.18); color: #dfe8ff; }
.nyct-launcher button:hover image { color: #dfe8ff; }
.nyct-launcher button:active { background: rgba(90,169,255,0.32); }
/* il bottone-logo ha un alone tenue: il suo disco scuro e' quasi come il pannello */
.nyct-launcher button.logo { background: rgba(99,208,224,0.12); }
.nyct-launcher button.logo:hover { background: rgba(99,208,224,0.22); }
.nyct-launcher separator { background: rgba(255,255,255,0.08);
                           min-width: 1px; min-height: 1px; margin: 4px; }
"""


class Launcher(Gtk.Window):
    def __init__(self):
        super().__init__(type=Gtk.WindowType.TOPLEVEL)
        self.set_title("Nyctalus")
        self.set_decorated(False)          # niente barra del titolo
        self.set_resizable(False)
        self.set_skip_taskbar_hint(True)
        self.set_skip_pager_hint(True)
        try:
            self.set_icon_name("nyctalus")
        except Exception:            # noqa: BLE001
            pass

        self.cfg = self._carica_cfg()
        self._salva_pendente = 0

        prov = Gtk.CssProvider()
        prov.load_from_data(CSS)
        Gtk.StyleContext.add_provider_for_screen(
            Gdk.Screen.get_default(), prov, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        self.get_style_context().add_class("nyct-launcher")

        self.set_keep_above(bool(self.cfg.get("sopra", True)))
        self._accel = Gtk.AccelGroup()       # ripiego locale (finestra a fuoco)
        self.add_accel_group(self._accel)
        self._keybinder_attivi = []          # combinazioni globali registrate
        self._costruisci()
        self._applica_posizione()

        self.add_events(Gdk.EventMask.BUTTON_PRESS_MASK)
        self.connect("button-press-event", self._premuto)
        self.connect("configure-event", self._spostato)
        self.connect("destroy", self._su_chiusura)
        self.show_all()
        self._applica_scorciatoie()

    # --- azioni associabili a un tasto rapido ---------------------------------
    def _azioni(self):
        # (chiave di config, etichetta, callback)
        return [
            ("pages", _t("lau.info"), self._apri_pages),
            ("browser", _t("lau.browser"), self._apri_browser),
            ("chat", _t("lau.chat"), self._apri_chat),
            ("file", _t("lau.file"), self._apri_file),
            ("mostra", _t("lau.mostra_nascondi"), self._toggle_visibile),
        ]

    # --- costruzione -----------------------------------------------------------
    def _costruisci(self):
        figlio = self.get_child()
        if figlio is not None:
            self.remove(figlio)
        verticale = self.cfg.get("orientamento") == "verticale"
        # dimensione dei bottoni, limitata a un intervallo ragionevole
        self.dim = max(DIM_MIN, min(DIM_MAX, int(self.cfg.get("dimensione", DIM_DEF))))
        orient = Gtk.Orientation.VERTICAL if verticale else Gtk.Orientation.HORIZONTAL
        self.box = Gtk.Box(orientation=orient, spacing=4)
        self.box.set_border_width(6)
        self.add(self.box)
        # il logo (a sinistra/in alto) apre il sito/guida su GitHub Pages
        self._bottone_logo(_t("lau.info"), self._apri_pages)
        self._bottone("web-browser-symbolic", _t("lau.browser"), self._apri_browser)
        self._bottone("user-available-symbolic", _t("lau.chat"), self._apri_chat, attivo=False)
        self._bottone("folder-download-symbolic", _t("lau.file"), self._apri_file, attivo=False)
        sep = Gtk.Separator(
            orientation=Gtk.Orientation.VERTICAL if not verticale else Gtk.Orientation.HORIZONTAL)
        self.box.pack_start(sep, False, False, 0)
        self._bottone("open-menu-symbolic", _t("lau.settings"), self._menu_impostazioni)
        self.box.show_all()

    def _nuovo_bottone(self, img, tip, cb):
        b = Gtk.Button()
        b.set_image(img)
        b.set_always_show_image(True)
        b.set_size_request(self.dim, self.dim)   # quadrato, dimensione regolabile
        b.set_relief(Gtk.ReliefStyle.NONE)
        b.set_can_focus(False)
        b.set_tooltip_text(tip)
        b.connect("clicked", lambda _w: cb())
        self.box.pack_start(b, False, False, 0)
        return b

    def _bottone(self, icona, tip, cb, attivo=True):
        img = Gtk.Image.new_from_icon_name(icona, Gtk.IconSize.DND)
        img.set_pixel_size(int(self.dim * 0.56))   # l'icona scala col bottone
        if not attivo:
            tip = "%s — %s" % (tip, _t("lau.in_arrivo"))
        return self._nuovo_bottone(img, tip, cb)

    def _bottone_logo(self, tip, cb):
        lato = int(self.dim * 0.74)
        try:
            px = GdkPixbuf.Pixbuf.new_from_file_at_scale(str(LOGO), lato, lato, True)
            img = Gtk.Image.new_from_pixbuf(px)
        except Exception:            # noqa: BLE001
            img = Gtk.Image.new_from_icon_name("help-about-symbolic", Gtk.IconSize.DND)
            img.set_pixel_size(lato)
        b = self._nuovo_bottone(img, tip, cb)
        b.get_style_context().add_class("logo")
        return b

    # --- tasti rapidi ---------------------------------------------------------
    def _scorciatoie_cfg(self):
        sc = self.cfg.get("scorciatoie")
        return dict(sc) if isinstance(sc, dict) else {}

    def _applica_scorciatoie(self):
        """Lega ogni combinazione configurata alla sua azione. Combinazioni
        vuote = azione senza tasto rapido."""
        # pulizia dei binding precedenti
        if HA_KEYBINDER:
            for accel in self._keybinder_attivi:
                try:
                    Keybinder.unbind(accel)
                except Exception:        # noqa: BLE001
                    pass
        self._keybinder_attivi = []
        try:
            self.remove_accel_group(self._accel)
        except Exception:                # noqa: BLE001
            pass
        self._accel = Gtk.AccelGroup()
        self.add_accel_group(self._accel)

        sc = self._scorciatoie_cfg()
        for chiave, _etichetta, cb in self._azioni():
            accel = (sc.get(chiave) or "").strip()
            if not accel:
                continue
            tasto, mods = Gtk.accelerator_parse(accel)
            if not tasto:
                continue
            if HA_KEYBINDER:
                # Keybinder vuole la stringa dell'acceleratore (es. "<Super>b")
                if Keybinder.bind(accel, lambda _a, _k, c=cb: c()):
                    self._keybinder_attivi.append(accel)
            else:
                self._accel.connect(tasto, mods, Gtk.AccelFlags.VISIBLE,
                                    lambda *_a, c=cb: (c(), True)[1])

    def _toggle_visibile(self):
        if self.get_visible() and self.is_active():
            self.hide()
        else:
            self.show_all()
            self.present()

    def _su_chiusura(self, *_a):
        if HA_KEYBINDER:
            for accel in self._keybinder_attivi:
                try:
                    Keybinder.unbind(accel)
                except Exception:        # noqa: BLE001
                    pass
        Gtk.main_quit()

    def _dialogo_scorciatoie(self):
        d = Gtk.Dialog(title=_t("lau.tasti_rapidi"), transient_for=self, modal=True)
        d.add_button(_t("lau.chiudi"), Gtk.ResponseType.CLOSE)
        box = d.get_content_area()
        box.set_spacing(8)
        box.set_border_width(14)
        intro = Gtk.Label(xalign=0, label=_t("lau.tasti_info")
                          if HA_KEYBINDER else _t("lau.tasti_info_locale"))
        intro.set_line_wrap(True)
        intro.set_max_width_chars(44)
        box.pack_start(intro, False, False, 0)

        griglia = Gtk.Grid(column_spacing=10, row_spacing=8)
        box.pack_start(griglia, False, False, 6)
        sc = self._scorciatoie_cfg()
        for r, (chiave, etichetta, _cb) in enumerate(self._azioni()):
            griglia.attach(Gtk.Label(label=etichetta, xalign=0), 0, r, 1, 1)
            btn = Gtk.Button(label=sc.get(chiave) or _t("lau.nessun_tasto"))
            btn.set_size_request(180, -1)
            btn.connect("clicked", self._cattura_tasto, chiave, btn)
            griglia.attach(btn, 1, r, 1, 1)
            pulisci = Gtk.Button(label="✕")
            pulisci.set_tooltip_text(_t("lau.togli_tasto"))
            pulisci.connect("clicked", self._pulisci_tasto, chiave, btn)
            griglia.attach(pulisci, 2, r, 1, 1)
        d.show_all()
        d.run()
        d.destroy()

    def _pulisci_tasto(self, _w, chiave, btn):
        sc = self._scorciatoie_cfg()
        sc.pop(chiave, None)
        self.cfg["scorciatoie"] = sc
        self._salva_cfg()
        btn.set_label(_t("lau.nessun_tasto"))
        self._applica_scorciatoie()

    def _cattura_tasto(self, _w, chiave, btn):
        """Finestrella che cattura la prossima combinazione premuta."""
        cat = Gtk.Dialog(title=_t("lau.premi_combinazione"), transient_for=self, modal=True)
        lab = Gtk.Label(label=_t("lau.premi_combinazione_corpo"))
        lab.set_margin_top(18)
        lab.set_margin_bottom(18)
        lab.set_margin_start(24)
        lab.set_margin_end(24)
        cat.get_content_area().pack_start(lab, True, True, 0)
        cat.add_events(Gdk.EventMask.KEY_PRESS_MASK)

        def premuto(_d, ev):
            # ignora i soli modificatori
            if ev.keyval in (Gdk.KEY_Control_L, Gdk.KEY_Control_R, Gdk.KEY_Alt_L,
                             Gdk.KEY_Alt_R, Gdk.KEY_Shift_L, Gdk.KEY_Shift_R,
                             Gdk.KEY_Super_L, Gdk.KEY_Super_R, Gdk.KEY_Meta_L,
                             Gdk.KEY_Meta_R, Gdk.KEY_Hyper_L, Gdk.KEY_Hyper_R):
                return True
            if ev.keyval == Gdk.KEY_Escape:
                cat.response(Gtk.ResponseType.CANCEL)
                return True
            mods = ev.state & Gtk.accelerator_get_default_mod_mask()
            if Gtk.accelerator_valid(ev.keyval, mods):
                accel = Gtk.accelerator_name(ev.keyval, mods)
                sc = self._scorciatoie_cfg()
                sc[chiave] = accel
                self.cfg["scorciatoie"] = sc
                self._salva_cfg()
                btn.set_label(accel)
                self._applica_scorciatoie()
                cat.response(Gtk.ResponseType.OK)
            return True

        cat.connect("key-press-event", premuto)
        cat.show_all()
        cat.run()
        cat.destroy()

    def _apri_pages(self):
        env = dict(os.environ)
        env["PYTHONPATH"] = str(CARTELLA_BROWSER) + os.pathsep + env.get("PYTHONPATH", "")
        self._lancia([sys.executable, "-m", "nyctalus_browser", PAGES], env=env)

    # --- azioni ----------------------------------------------------------------
    def _lancia(self, argomenti, env=None):
        try:
            subprocess.Popen(argomenti, env=env, start_new_session=True,
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except Exception as e:       # noqa: BLE001
            self._avviso("Errore", str(e))

    def _apri_browser(self):
        env = dict(os.environ)
        env["PYTHONPATH"] = str(CARTELLA_BROWSER) + os.pathsep + env.get("PYTHONPATH", "")
        self._lancia([sys.executable, "-m", "nyctalus_browser"], env=env)

    def _apri_chat(self):
        self._avviso(_t("lau.chat"), _t("lau.in_arrivo_corpo"))

    def _apri_file(self):
        self._avviso(_t("lau.file"), _t("lau.in_arrivo_corpo"))

    def _menu_impostazioni(self):
        menu = Gtk.Menu()
        verticale = self.cfg.get("orientamento") == "verticale"

        def voce(testo, cb):
            mi = Gtk.MenuItem(label=testo)
            mi.connect("activate", lambda _w: cb())
            menu.append(mi)

        voce(_t("lau.orizzontale") if verticale else _t("lau.verticale"), self._cambia_orientamento)
        menu.append(Gtk.SeparatorMenuItem())
        grande = Gtk.MenuItem(label=_t("lau.piu_grande"))
        grande.connect("activate", lambda _w: self._ridimensiona(+PASSO_DIM))
        grande.set_sensitive(self.dim < DIM_MAX)
        menu.append(grande)
        piccola = Gtk.MenuItem(label=_t("lau.piu_piccola"))
        piccola.connect("activate", lambda _w: self._ridimensiona(-PASSO_DIM))
        piccola.set_sensitive(self.dim > DIM_MIN)
        menu.append(piccola)
        normale = Gtk.MenuItem(label=_t("lau.normale"))
        normale.connect("activate", lambda _w: self._dimensione_normale())
        normale.set_sensitive(self.dim != DIM_DEF)
        menu.append(normale)
        menu.append(Gtk.SeparatorMenuItem())
        sopra = Gtk.CheckMenuItem(label=_t("lau.sopra"))
        sopra.set_active(bool(self.cfg.get("sopra", True)))
        sopra.connect("toggled", self._cambia_sopra)
        menu.append(sopra)
        menu.append(Gtk.SeparatorMenuItem())
        voce(_t("lau.tasti_rapidi") + "…", self._dialogo_scorciatoie)
        menu.append(Gtk.SeparatorMenuItem())
        voce(_t("lau.esci"), self.destroy)
        menu.show_all()
        menu.popup_at_pointer(None)

    def _cambia_orientamento(self):
        self.cfg["orientamento"] = "orizzontale" if self.cfg.get("orientamento") == "verticale" else "verticale"
        self._salva_cfg()
        self._costruisci()
        self.resize(1, 1)            # si ridimensiona al contenuto

    def _ridimensiona(self, delta):
        self._imposta_dimensione(int(self.cfg.get("dimensione", DIM_DEF)) + delta)

    def _dimensione_normale(self):
        self._imposta_dimensione(DIM_DEF)

    def _imposta_dimensione(self, valore):
        nuova = max(DIM_MIN, min(DIM_MAX, valore))
        if nuova == self.cfg.get("dimensione", DIM_DEF):
            return
        self.cfg["dimensione"] = nuova
        self._salva_cfg()
        self._costruisci()
        self.resize(1, 1)            # si ridimensiona al contenuto

    def _cambia_sopra(self, item):
        self.cfg["sopra"] = bool(item.get_active())
        self.set_keep_above(self.cfg["sopra"])
        self._salva_cfg()

    # --- spostamento e posizione ----------------------------------------------
    def _premuto(self, _widget, event):
        if event.button == 1:        # trascina il pannello col tasto sinistro
            self.begin_move_drag(event.button, int(event.x_root), int(event.y_root), event.time)
            return True
        if event.button == 3:        # tasto destro: menu
            self._menu_impostazioni()
            return True
        return False

    def _spostato(self, *_):
        if self._salva_pendente:
            GLib.source_remove(self._salva_pendente)
        self._salva_pendente = GLib.timeout_add(500, self._salva_posizione)

    def _salva_posizione(self):
        self._salva_pendente = 0
        x, y = self.get_position()
        self.cfg["x"], self.cfg["y"] = x, y
        self._salva_cfg()
        return False

    def _applica_posizione(self):
        x, y = self.cfg.get("x"), self.cfg.get("y")
        if isinstance(x, int) and isinstance(y, int):
            self.move(x, y)

    # --- utilità ---------------------------------------------------------------
    def _avviso(self, titolo, corpo):
        d = Gtk.MessageDialog(transient_for=self, modal=True,
                              message_type=Gtk.MessageType.INFO,
                              buttons=Gtk.ButtonsType.OK, text=titolo)
        d.format_secondary_text(corpo)
        d.run()
        d.destroy()

    def _carica_cfg(self):
        try:
            return json.loads(CONF.read_text(encoding="utf-8"))
        except Exception:            # noqa: BLE001
            return {"orientamento": "orizzontale", "sopra": True}

    def _salva_cfg(self):
        try:
            CONF_DIR.mkdir(exist_ok=True)
            CONF.write_text(json.dumps(self.cfg, indent=2), encoding="utf-8")
        except Exception:            # noqa: BLE001
            pass


def main():
    Launcher()
    Gtk.main()


if __name__ == "__main__":
    main()
