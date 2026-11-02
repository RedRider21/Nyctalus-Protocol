# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Tema GLOBALE di Nyctalus sul PC: uno solo per tutti i programmi (mini menu, chat,
browser, finestre delle chiamate). Si sceglie dal menu ⚙ del mini menu o dalla chat;
chi è aperto se ne accorge e cambia colori al volo.

Il tema scelto sta in ~/.config/nyctalus/tema (una parola: notte, ardesia, viola, chiaro).
Solo libreria standard + Gio (per accorgersi dei cambi).
"""
import colorsys
import os
import re
from pathlib import Path

# Tutti i colori dell'interfaccia. RGB = colore d'accento in "r,g,b" per i veli;
# CHBASE = fondo profondo delle chiamate; SCURO = tema scuro (per GTK e il browser).
TEMI = {
    "notte": {   # blu notte Nyctalus (predefinito)
        "BG": "#0a0e1a", "SIDEBAR": "#0c1120", "PANEL": "#0e1424", "TEXT": "#e6ecff",
        "MUTED": "#6b7797", "ACCENT": "#5aa9ff", "RGB": "90,169,255", "PILLTXT": "#cfe0ff",
        "SEND": "#2f6fe0", "SENDH": "#3b7cf0", "BUBIN": "#1b2740", "BUBINTXT": "#e6ecff",
        "OUT1": "#2f6fe0", "OUT2": "#3f82f2", "SHADOW": "0.30", "TRAMA": "0.05", "CHBASE": "#070b16",
    },
    "ardesia": {   # nero-ardesia, alla Telegram
        "BG": "#0e1621", "SIDEBAR": "#17212b", "PANEL": "#17212b", "TEXT": "#f5f5f5",
        "MUTED": "#7f91a4", "ACCENT": "#6ab3f3", "RGB": "106,179,243", "PILLTXT": "#d6e9fb",
        "SEND": "#3390ec", "SENDH": "#4aa0f0", "BUBIN": "#182533", "BUBINTXT": "#f5f5f5",
        "OUT1": "#2b5278", "OUT2": "#33618d", "SHADOW": "0.35", "TRAMA": "0.05", "CHBASE": "#0b131d",
    },
    "viola": {   # viola notturno
        "BG": "#120f1c", "SIDEBAR": "#1a1626", "PANEL": "#1a1626", "TEXT": "#f1edff",
        "MUTED": "#8a82a6", "ACCENT": "#9d85ff", "RGB": "157,133,255", "PILLTXT": "#e2dbff",
        "SEND": "#7a5cf0", "SENDH": "#8b70f5", "BUBIN": "#241f35", "BUBINTXT": "#f1edff",
        "OUT1": "#6b4fe0", "OUT2": "#8a6cf2", "SHADOW": "0.35", "TRAMA": "0.05", "CHBASE": "#0e0b18",
    },
    "chiaro": {   # chiaro, per il giorno
        "BG": "#eef1f6", "SIDEBAR": "#ffffff", "PANEL": "#ffffff", "TEXT": "#1a2233",
        "MUTED": "#7d889c", "ACCENT": "#2f6fe0", "RGB": "47,111,224", "PILLTXT": "#1f4fae",
        "SEND": "#2f6fe0", "SENDH": "#3b7cf0", "BUBIN": "#ffffff", "BUBINTXT": "#1a2233",
        "OUT1": "#2f6fe0", "OUT2": "#4a86f0", "SHADOW": "0.10", "TRAMA": "0.07", "CHBASE": "#16294f",
    },
}
PREDEFINITO = "notte"


# --- temi ricavati da UN colore: lo stesso calcolo c'è nella web-app (app.js) e nell'app
#     Android (MainActivity.deriva): tenerli uguali ---------------------------------------
def _hex(h, s, l):
    r, g, b = colorsys.hls_to_rgb(h, l, s)
    return "#%02x%02x%02x" % (round(r * 255), round(g * 255), round(b * 255))


def deriva(accento, scuro=True):
    """Tutta la tavolozza da un colore principale (#rrggbb) e dalla scelta chiaro/scuro:
    fondo, pannelli, testo, bolle e chiamate prendono la sua tinta, con contrasto leggibile."""
    accento = accento if re.fullmatch(r"#[0-9a-fA-F]{6}", accento or "") else "#5aa9ff"
    r, g, b = (int(accento[i:i + 2], 16) / 255 for i in (1, 3, 5))
    h, l, s = colorsys.rgb_to_hls(r, g, b)
    if scuro:
        t = {"BG": _hex(h, min(s, .45), .065), "SIDEBAR": _hex(h, min(s, .42), .085),
             "PANEL": _hex(h, min(s, .40), .10), "TEXT": _hex(h, .35, .93), "MUTED": _hex(h, .14, .52),
             "ACCENT": _hex(h, max(s, .55), max(l, .62)), "SEND": _hex(h, max(s, .6), .46),
             "SENDH": _hex(h, max(s, .6), .52), "BUBIN": _hex(h, .32, .17), "PILLTXT": _hex(h, .6, .88),
             "BORDO": _hex(h, .30, .17), "SHADOW": "0.30", "TRAMA": "0.05", "CHBASE": _hex(h, .45, .045)}
    else:
        t = {"BG": _hex(h, .25, .95), "SIDEBAR": "#ffffff", "PANEL": "#ffffff", "TEXT": _hex(h, .30, .14),
             "MUTED": _hex(h, .12, .50), "ACCENT": _hex(h, max(s, .6), .45), "SEND": _hex(h, max(s, .6), .45),
             "SENDH": _hex(h, max(s, .6), .51), "BUBIN": "#ffffff", "PILLTXT": _hex(h, .6, .33),
             "BORDO": _hex(h, .20, .88), "SHADOW": "0.10", "TRAMA": "0.07", "CHBASE": _hex(h, .45, .18)}
    t["BUBINTXT"] = t["TEXT"]
    t["OUT1"], t["OUT2"] = t["SEND"], (_hex(h, max(s, .6), .55) if not scuro else t["SENDH"])
    a = t["ACCENT"]
    t["RGB"] = "%d,%d,%d" % tuple(int(a[i:i + 2], 16) for i in (1, 3, 5))
    t["SCURO"] = scuro
    return t


# due temi in più, ricavati: verde tenue (scuro) e celeste (chiaro)
TEMI["menta"] = deriva("#3ecf9a", True)
TEMI["cielo"] = deriva("#2f9fe0", False)
for _n, _t in TEMI.items():
    _t.setdefault("SCURO", _n != "chiaro")
    _t.setdefault("BORDO", "#dde3ec" if _n == "chiaro" else "#1b2740")
NOMI = {"notte": "Notte", "ardesia": "Ardesia", "viola": "Viola", "chiaro": "Chiaro", "menta": "Menta",
        "cielo": "Cielo", "personalizzato": "Personalizzato"}
PERSONALIZZATO = "personalizzato"


def file_tema():
    base = os.environ.get("XDG_CONFIG_HOME") or str(Path.home() / ".config")
    return Path(base) / "nyctalus" / "tema"


def _leggi_file():
    try:
        return file_tema().read_text(encoding="utf-8").split()
    except OSError:
        return []


def leggi(ripiego=None):
    """Il tema globale; se non è mai stato scelto, `ripiego` (es. quello vecchio della chat)."""
    p = _leggi_file()
    if p and (p[0] in TEMI or p[0] == PERSONALIZZATO):
        return p[0]
    return ripiego if ripiego in TEMI else PREDEFINITO


def personalizzato():
    """(colore, scuro) del tema personalizzato: l'ultimo scelto, anche se ora è attivo un altro."""
    p = _leggi_file()
    try:
        f = file_tema().with_name("tema-personalizzato").read_text(encoding="utf-8").split()
        if len(f) >= 2 and re.fullmatch(r"#[0-9a-fA-F]{6}", f[0]):
            return f[0].lower(), f[1] != "chiaro"
    except OSError:
        pass
    if len(p) >= 3 and p[0] == PERSONALIZZATO:
        return p[1], p[2] != "chiaro"
    return "#3ecf9a", True


def scrivi_personalizzato(colore, scuro=True):
    """Sceglie il tema personalizzato (un colore + chiaro/scuro) per tutti i programmi."""
    if not re.fullmatch(r"#[0-9a-fA-F]{6}", colore or ""):
        return False
    try:
        file_tema().parent.mkdir(parents=True, exist_ok=True)
        file_tema().with_name("tema-personalizzato").write_text(
            "%s %s\n" % (colore.lower(), "scuro" if scuro else "chiaro"), encoding="utf-8")
    except OSError:
        return False
    return _scrivi_riga("%s %s %s" % (PERSONALIZZATO, colore.lower(), "scuro" if scuro else "chiaro"))


def scrivi(nome):
    """Sceglie il tema per tutti i programmi (scrittura atomica: chi osserva legge sempre intero)."""
    if nome == PERSONALIZZATO:
        return scrivi_personalizzato(*personalizzato())
    if nome not in TEMI:
        return False
    return _scrivi_riga(nome)


def _scrivi_riga(riga):
    f = file_tema()
    try:
        f.parent.mkdir(parents=True, exist_ok=True)
        tmp = f.with_suffix(".tmp")
        tmp.write_text(riga + "\n", encoding="utf-8")
        os.replace(tmp, f)
        return True
    except OSError:
        return False


def colori(nome=None):
    nome = nome or leggi()
    if nome == PERSONALIZZATO:
        return deriva(*personalizzato())
    return TEMI.get(nome, TEMI[PREDEFINITO])


def scuro(nome=None):
    return bool(colori(nome).get("SCURO", True))


def osserva(richiamo):
    """Chiama `richiamo(nome)` quando il tema globale cambia (anche da un altro programma).
    Restituisce l'osservatore: va tenuto in vita finché serve."""
    try:
        from gi.repository import Gio
    except Exception:            # noqa: BLE001
        return None
    f = file_tema()
    try:
        f.parent.mkdir(parents=True, exist_ok=True)
    except OSError:
        return None
    mon = Gio.File.new_for_path(str(f)).monitor_file(Gio.FileMonitorFlags.WATCH_MOVES, None)
    stato = {"ultimo": _leggi_file()}

    def cambiato(*_a):
        nuovo = _leggi_file()
        if nuovo and nuovo != stato["ultimo"]:       # anche un altro colore personalizzato
            stato["ultimo"] = nuovo
            richiamo(leggi())
    mon.connect("changed", cambiato)
    return mon


def scegli_colore(madre=None, titolo="Tema personalizzato"):
    """Finestra GTK per il tema personalizzato: colore (tavolozza o codice #rrggbb) e chiaro/
    scuro, con l'anteprima. Restituisce (colore, scuro) oppure None se si annulla."""
    import gi
    gi.require_version("Gtk", "3.0")
    from gi.repository import Gdk, Gtk
    colore, scuro_ora = personalizzato()
    d = Gtk.Dialog(title=titolo, transient_for=madre, modal=True)
    d.add_buttons("Annulla", Gtk.ResponseType.CANCEL, "Usa questo tema", Gtk.ResponseType.OK)
    d.set_default_response(Gtk.ResponseType.OK)
    box = d.get_content_area()
    box.set_spacing(10)
    box.set_border_width(14)
    sc = Gtk.ColorChooserWidget(show_editor=False)
    sc.set_use_alpha(False)
    rgba = Gdk.RGBA()
    rgba.parse(colore)
    sc.set_rgba(rgba)
    box.pack_start(sc, True, True, 0)
    riga = Gtk.Box(spacing=8)
    codice = Gtk.Entry(text=colore, width_chars=9, max_length=7)
    riga.pack_start(Gtk.Label(label="Codice colore"), False, False, 0)
    riga.pack_start(codice, False, False, 0)
    b_scuro = Gtk.RadioButton.new_with_label(None, "Scuro")
    b_chiaro = Gtk.RadioButton.new_with_label_from_widget(b_scuro, "Chiaro")
    (b_scuro if scuro_ora else b_chiaro).set_active(True)
    riga.pack_end(b_chiaro, False, False, 0)
    riga.pack_end(b_scuro, False, False, 0)
    box.pack_start(riga, False, False, 0)
    anteprima = Gtk.Label()
    box.pack_start(anteprima, False, False, 0)
    stato = {"colore": colore}

    def aggiorna(*_a):
        t = deriva(stato["colore"], b_scuro.get_active())
        anteprima.set_markup('<span background="%s" foreground="%s">  Nyctalus · anteprima  </span>  '
                             '<span background="%s" foreground="#ffffff">  messaggio  </span>  '
                             '<span foreground="%s"><b>accento</b></span>' % (t["BG"], t["TEXT"], t["SEND"], t["ACCENT"]))

    def da_tavolozza(*_a):
        c = sc.get_rgba()
        stato["colore"] = "#%02x%02x%02x" % (round(c.red * 255), round(c.green * 255), round(c.blue * 255))
        if codice.get_text().lower() != stato["colore"]:
            codice.set_text(stato["colore"])
        aggiorna()

    def da_codice(*_a):
        v = codice.get_text().strip()
        if re.fullmatch(r"#[0-9a-fA-F]{6}", v) and v.lower() != stato["colore"]:
            stato["colore"] = v.lower()
            r = Gdk.RGBA()
            r.parse(v)
            sc.set_rgba(r)
        aggiorna()

    sc.connect("notify::rgba", da_tavolozza)
    sc.connect("color-activated", lambda *_a: d.response(Gtk.ResponseType.OK))
    codice.connect("changed", da_codice)
    b_scuro.connect("toggled", aggiorna)
    aggiorna()
    d.show_all()
    esito = d.run()
    scelta = (stato["colore"], b_scuro.get_active())
    d.destroy()
    return scelta if esito == Gtk.ResponseType.OK else None

