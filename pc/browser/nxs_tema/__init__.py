# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Tema GLOBALE di Nyctalus sul PC: uno solo per tutti i programmi (mini menu, chat,
browser, finestre delle chiamate). Si sceglie dal menu ⚙ del mini menu o dalla chat;
chi è aperto se ne accorge e cambia colori al volo.

Il tema scelto sta in ~/.config/nyctalus/tema (una parola: notte, ardesia, viola, chiaro).
Solo libreria standard + Gio (per accorgersi dei cambi).
"""
import os
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
NOMI = {"notte": "Notte", "ardesia": "Ardesia", "viola": "Viola", "chiaro": "Chiaro"}
PREDEFINITO = "notte"


def file_tema():
    base = os.environ.get("XDG_CONFIG_HOME") or str(Path.home() / ".config")
    return Path(base) / "nyctalus" / "tema"


def leggi(ripiego=None):
    """Il tema globale; se non è mai stato scelto, `ripiego` (es. quello vecchio della chat)."""
    try:
        t = file_tema().read_text(encoding="utf-8").strip()
        if t in TEMI:
            return t
    except OSError:
        pass
    return ripiego if ripiego in TEMI else PREDEFINITO


def scrivi(nome):
    """Sceglie il tema per tutti i programmi (scrittura atomica: chi osserva legge sempre intero)."""
    if nome not in TEMI:
        return False
    f = file_tema()
    try:
        f.parent.mkdir(parents=True, exist_ok=True)
        tmp = f.with_suffix(".tmp")
        tmp.write_text(nome + "\n", encoding="utf-8")
        os.replace(tmp, f)
        return True
    except OSError:
        return False


def colori(nome=None):
    return TEMI.get(nome or leggi(), TEMI[PREDEFINITO])


def scuro(nome=None):
    return (nome or leggi()) != "chiaro"


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
    stato = {"ultimo": leggi()}

    def cambiato(*_a):
        nuovo = leggi()
        if nuovo != stato["ultimo"]:
            stato["ultimo"] = nuovo
            richiamo(nuovo)
    mon.connect("changed", cambiato)
    return mon
