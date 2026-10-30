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
import json
import os
import shutil
import socket
import subprocess
import sys
import threading
from datetime import datetime
from pathlib import Path

import gi

gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, GdkPixbuf, Gio, GLib, Gtk, Pango  # noqa: E402

from nyctalus_chat import media  # noqa: E402

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
    "chat.gal.titolo": "Allega",
    "chat.gal.galleria": "Galleria",
    "chat.gal.fotocamera": "Fotocamera",
    "chat.gal.scheda_file": "File",
    "chat.gal.scheda_altre": "Altre foto",
    "chat.gal.scheda_foto": "Foto",
    "chat.gal.scheda_video": "Video",
    "chat.gal.didascalia": "Aggiungi una didascalia…",
    "chat.gal.exif": "Senza metadati",
    "chat.gal.invia": "Invia",
    "chat.gal.invia_n": "Invia (%d)",
    "chat.gal.selezionati": "%d selezionati",
    "chat.gal.vuoto": "Nessuna foto o video recente nelle cartelle Immagini, Video e Download. Usa «Altre foto» o «File».",
    "chat.allega.foto_cam": "Scatta una foto",
    "chat.allega.video_cam": "Registra un video",
    "chat.allega.media": "Foto o video",
    "chat.allega.file": "File",
    "chat.filtro.media": "Foto e video",
    "chat.cam.titolo_foto": "Scatta una foto",
    "chat.cam.titolo_video": "Registra un video",
    "chat.cam.scatta": "📷  Scatta",
    "chat.cam.registra": "●  Registra",
    "chat.cam.stop": "■  Ferma e usa",
    "chat.cam.annulla": "Annulla",
    "chat.cam.errore": "Fotocamera non disponibile",
    "chat.cam.attendi": "Un attimo, la fotocamera si sta avviando…",
    "chat.ant.titolo": "Invia %d file",
    "chat.ant.titolo1": "Invia",
    "chat.ant.didascalia": "Aggiungi una didascalia…",
    "chat.ant.exif": "Togli i metadati dalle foto (posizione GPS, fotocamera)",
    "chat.ant.invia": "Invia",
    "chat.ant.annulla": "Annulla",
    "chat.menu.altro": "Altre azioni",
    "chat.menu.apri": "Apri la chat",
    "chat.menu.letto": "Segna come letto",
    "chat.menu.non_letto": "Segna come non letto",
    "chat.menu.fissa": "Fissa in alto",
    "chat.menu.sblocca": "Togli dai fissati",
    "chat.menu.rinomina": "Rinomina…",
    "chat.menu.copia_id": "Copia il Contact ID",
    "chat.menu.cancella_cron": "Cancella cronologia",
    "chat.menu.elimina_chat": "Elimina la chat",
    "chat.menu.salva_come": "Salva con nome…",
    "chat.menu.inoltra": "Inoltra",
    "chat.menu.inoltra_a": "Inoltra a…",
    "chat.menu.copia_testo": "Copia testo",
    "chat.menu.elimina_msg": "Elimina per me",
    "chat.menu.salva": "Salva",
    "chat.menu.cancella": "Cancella",
    "chat.menu.elimina": "Elimina",
    "chat.conferma.cancella_cron": "Tutti i messaggi con %s verranno cancellati da questo computer. I file ricevuti restano nella cartella degli allegati.",
    "chat.conferma.elimina_chat": "%s verrà tolto dai contatti e la cronologia verrà cancellata da questo computer.",
    "chat.conferma.elimina_msg": "Il messaggio verrà cancellato solo da questo computer, non da quello dell'altra persona.",
    "chat.tu": "Tu:",
    "chat.foto": "Foto",
    "chat.allega": "Allega file, foto, video…",
    "chat.apri": "Apri",
    "chat.mostra_cartella": "Mostra nella cartella",
    "chat.file_mancante": "file non più disponibile",
    "chat.invio_in_corso": "Invio",
    "chat.ricezione": "Ricezione",
    "chat.senza_exif": "Foto senza metadati (posizione GPS, fotocamera)",
    "chat.filtro.tutti": "Tutti i file",
    "chat.filtro.immagini": "Immagini",
    "chat.filtro.video": "Video",
    "chat.filtro.audio": "Audio",
    "chat.filtro.documenti": "Documenti",
    "chat.filtro.archivi": "Archivi",
    "chat.aspetto": "Aspetto: tema e sfondo",
    "chat.tema": "Tema",
    "chat.sfondo": "Sfondo",
    "chat.tema.notte": "Notte (blu)",
    "chat.tema.ardesia": "Ardesia (stile Telegram)",
    "chat.tema.viola": "Viola notturno",
    "chat.tema.chiaro": "Chiaro",
    "chat.sfondo.pipistrelli": "Trama a pipistrelli",
    "chat.sfondo.velo": "Velo di colore",
    "chat.sfondo.tinta": "Tinta unita",
    "chat.aiuto": "Manuale (apre nel browser)",
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
RADICE = Path(__file__).resolve().parents[2]      # radice del pacchetto, o pc/ nel progetto
LOGO = next((b / "logo.svg" for b in (RADICE, RADICE.parent) if (b / "logo.svg").exists()), RADICE / "logo.svg")
# Manuale online (Pages). Il pulsante « ? » lo apre nel browser Nyctalus.
MANUALE_URL = os.environ.get(
    "NYCTALUS_MANUALE_URL", "https://redrider21.github.io/Nyctalus-Protocol/#chat")

# Colori per gli avatar (scelti in modo deterministico dall'indirizzo).
AVATAR_COLORI = ["#5aa9ff", "#7f6cff", "#38c6a4", "#e0794b", "#d45b8e",
                 "#c9a227", "#4bb3e0", "#8a86ff"]

CSS_TMPL = """
.chat-root { background: @BG@; }

/* le ListBox/ScrolledWindow non devono mostrare il bianco del tema */
scrolledwindow, scrolledwindow viewport, list, list > row {
    background-color: transparent;
    background-image: none;
}

/* --- colonna sinistra --- */
.sidebar { background: @SIDEBAR@; border-right: 1px solid rgba(@RGB@,0.14); }
.sidebar-testa { padding: 14px 16px 10px 16px; }
.app-nome { color: @TEXT@; font-weight: 800; font-size: 15px; }
.app-sotto { color: @ACCENT@; font-size: 11px; }
.cerca { margin: 0 12px 8px 12px; }
.cerca entry, entry.cerca { background: @BG@; color: @TEXT@; border-radius: 10px;
        border: 1px solid rgba(@RGB@,0.18); padding: 6px 10px; }
.sezione { color: @MUTED@; font-size: 11px; font-weight: 700; letter-spacing: .4px;
           padding: 4px 16px; }

row.contatto { padding: 7px 12px; border-radius: 12px; margin: 1px 8px; }
row.contatto:selected { background: rgba(@RGB@,0.20); box-shadow: inset 3px 0 0 @ACCENT@; }
row.contatto:hover { background: rgba(@RGB@,0.09); }
.nome { color: @TEXT@; font-weight: 600; }
.stato-riga { color: @MUTED@; font-size: 11px; }

.avatar { color: #ffffff; font-weight: 700; border-radius: 22px;
          min-width: 38px; min-height: 38px; }
.avatar.grande { min-width: 44px; min-height: 44px; border-radius: 26px; font-size: 17px; }
.avatar.online { box-shadow: 0 0 0 2px @SIDEBAR@, 0 0 0 4px #37d08a; }
.av0{background:#5aa9ff}.av1{background:#7f6cff}.av2{background:#38c6a4}.av3{background:#e0794b}
.av4{background:#d45b8e}.av5{background:#c9a227}.av6{background:#4bb3e0}.av7{background:#8a86ff}

/* card del Contact ID */
.idcard { background: @BG@; border-top: 1px solid rgba(@RGB@,0.14); padding: 12px 14px; }
.idcard .etich { color: @MUTED@; font-size: 11px; font-weight: 700; }
.idcard .valore { color: @ACCENT@; font-family: monospace; font-size: 11px; }
.pill { background: rgba(@RGB@,0.15); color: @PILLTXT@; border: none;
        border-radius: 9px; padding: 4px 10px; }
.pill:hover { background: rgba(@RGB@,0.26); }
.piu { background: @SEND@; color: #fff; border: none; border-radius: 9px;
       padding: 4px 10px; min-width: 0; min-height: 0; font-weight: 800; }
.piu:hover { background: @SENDH@; }

/* --- colonna destra --- */
.conv-testa { background: @PANEL@; border-bottom: 1px solid rgba(@RGB@,0.14); padding: 10px 16px; }
.conv-testa .nome { font-size: 15px; }
.conv-testa .sotto { color: @MUTED@; font-size: 11px; }
.conv-testa .sotto.on { color: #37d08a; }
/* sfondo chat: scelto dall'utente, su tutta l'area */
.chat-bg {
    background-color: @BG@;
    background-image: @CHATIMG@;
    background-repeat: @CHATREP@; }
.conversazione { padding: 10px 6px; }

.bolla { padding: 9px 13px; border-radius: 17px; margin: 3px 12px;
         box-shadow: 0 1px 2px rgba(0,0,0,@SHADOW@); }
.bolla-in  { background-color: @BUBIN@; color: @BUBINTXT@; border-radius: 17px 17px 17px 5px; }
.bolla-out { color: #ffffff; border-radius: 17px 17px 5px 17px;
             background-image: linear-gradient(135deg, @OUT1@, @OUT2@); }
.bolla .testo { font-size: 13px; }
.bolla .ora { font-size: 10px; }
.bolla-in  .ora { color: @MUTED@; }
.bolla-out .ora { color: rgba(255,255,255,0.60); }

/* benvenuto / stato vuoto */
.vuoto-wrap { background: transparent; }
.vuoto-bat { font-size: 54px; }
.vuoto-tit { color: @TEXT@; font-weight: 700; font-size: 17px; }
.vuoto-sub { color: @MUTED@; font-size: 13px; }
.hint { color: @MUTED@; font-size: 12px; }

/* compositore */
.compositore { background: @PANEL@; border-top: 1px solid rgba(@RGB@,0.14); padding: 10px; }
.compositore entry { background: @BG@; color: @TEXT@; border-radius: 20px;
                     border: 1px solid rgba(@RGB@,0.20); padding: 10px 16px; }
.compositore entry:focus { border-color: @ACCENT@; }
.send { background: @SEND@; color: #fff; border: none; border-radius: 21px;
        min-width: 42px; min-height: 42px; font-size: 16px; }
.send:hover { background: @SENDH@; }
.send:disabled { background: rgba(@RGB@,0.15); color: @MUTED@; }

/* menu a comparsa intonati al tema (tasto destro, ⋮, ⚙) */
menu, .menu, .context-menu {
    background-color: @PANEL@; color: @TEXT@;
    border: 1px solid rgba(@RGB@,0.18); border-radius: 10px; padding: 6px 0; }
menu menuitem { padding: 7px 18px 7px 12px; color: @TEXT@; }
menu menuitem:hover { background-color: rgba(@RGB@,0.18); }
menu menuitem image { color: @MUTED@; }
menu menuitem:hover image { color: @TEXT@; }
menu menuitem:disabled label { color: @MUTED@; font-size: 11px; font-weight: 700; }
menu menuitem.pericolosa label, menu menuitem.pericolosa image { color: #ef5b5b; }
menu separator { background-color: rgba(@RGB@,0.14); margin: 5px 0; min-height: 1px; }
menu check, menu radio { color: @ACCENT@; }

/* elenco stile Telegram: ora, contatore, spillo; tre puntini */
.ora-riga { color: @MUTED@; font-size: 11px; }
.badge { background: @SEND@; color: #ffffff; border-radius: 10px; font-size: 11px;
         font-weight: 700; padding: 0 6px; min-width: 10px; }
.spillo { font-size: 10px; }
.puntini { background: transparent; border: none; box-shadow: none; color: @MUTED@;
           font-size: 20px; padding: 2px 10px; }
.puntini:hover { color: @TEXT@; background: rgba(@RGB@,0.12); }

/* finestre di dialogo intonate al tema (aggiungi, rinomina, conferme, invio, fotocamera) */
dialog, messagedialog, dialog > box, messagedialog > box { background-color: @PANEL@; color: @TEXT@; }
dialog label, messagedialog label { color: @TEXT@; }
messagedialog .dialog-vbox label:not(:first-child) { color: @MUTED@; }
dialog entry, messagedialog entry { background-color: @BG@; color: @TEXT@; border: 1px solid rgba(@RGB@,0.25);
                                    border-radius: 10px; padding: 8px 12px; box-shadow: none; }
dialog entry:focus { border-color: @ACCENT@; }
dialog button, messagedialog button { background-color: rgba(@RGB@,0.14); background-image: none; color: @TEXT@;
                                      border: none; border-radius: 9px; padding: 6px 16px; box-shadow: none; text-shadow: none; }
dialog button:hover, messagedialog button:hover { background-color: rgba(@RGB@,0.26); }
dialog button.suggested-action { background-color: @SEND@; color: #ffffff; }
dialog button.suggested-action:hover { background-color: @SENDH@; }
dialog button.destructive-action, messagedialog button.destructive-action { background-color: #d64545; color: #ffffff; }
dialog checkbutton { color: @TEXT@; }
dialog check { background-color: @BG@; border: 1px solid rgba(@RGB@,0.35); border-radius: 4px; }
dialog check:checked { background-color: @SEND@; color: #ffffff; }
dialog list, dialog list row { background-color: transparent; color: @TEXT@; }
dialog list row:selected { background-color: rgba(@RGB@,0.22); }
.fotocamera { background-color: #000000; }

/* selettore allegati come Telegram */
.selettore-allegati { background-color: @PANEL@; }
.gal-titolo { font-weight: 800; font-size: 16px; color: @TEXT@; }
.gal-conta { color: @ACCENT@; font-weight: 700; }
.gal-cam { background-image: linear-gradient(135deg, #1f3b66, #2a2050); color: #ffffff; border: none;
           border-radius: 2px; font-size: 13px; }
.gal-schede button { border-radius: 18px; }
.gal-vuoto { color: @MUTED@; padding: 20px; }

/* allegati in stile Telegram */
.bolla.foto { padding: 0; background-color: transparent; background-image: none; box-shadow: none; }
.bolla.allegato { padding: 9px 14px 7px 10px; }
.cam-stato { color: @MUTED@; }
.cam-scatto { background: @SEND@; color: #ffffff; border: none; border-radius: 18px; padding: 6px 18px; font-weight: 700; }
.cam-scatto.registrando { background: #e04848; }
.anteprima-invio entry { border-radius: 10px; padding: 8px 12px; }

/* allegati */
.graffetta { background: transparent; border: none; box-shadow: none; color: @MUTED@;
             font-size: 18px; padding: 4px 6px; min-width: 0; }
.graffetta:hover { color: @ACCENT@; }
popover.pannello-emoji, popover.pannello-emoji > contents, .pannello-emoji scrolledwindow, .pannello-emoji flowbox,
.pannello-emoji flowboxchild { background-color: @PANEL@; color: @TEXT@; }
popover.pannello-emoji { border: 1px solid rgba(@RGB@,0.18); border-radius: 12px; }
.pannello-emoji button { color: @TEXT@; }
.pannello-emoji .schede-emoji { border-bottom: 1px solid rgba(@RGB@,0.14); }
.pannello-emoji .schede-emoji button { font-size: 17px; padding: 4px 8px; border-radius: 0; opacity: 0.5;
    border-bottom: 2px solid transparent; }
.pannello-emoji .schede-emoji button.sel { opacity: 1; border-bottom-color: @ACCENT@; }
.pannello-emoji .titolo-emoji { font-size: 11px; font-weight: bold; color: @MUTED@; margin: 8px 6px 2px; }
.pannello-emoji button.emoji { font-size: 22px; padding: 2px; min-width: 38px; min-height: 36px; border-radius: 10px; }
.pannello-emoji button.emoji:hover { background: alpha(@ACCENT@, 0.16); }
.anteprima { border-radius: 12px; }
.icona-file { font-size: 28px; padding: 2px 4px; }
.nome-file { font-weight: 600; }
.dim-file { font-size: 11px; opacity: 0.75; }
progressbar.barra trough { min-height: 5px; border-radius: 3px; background: rgba(@RGB@,0.20); }
progressbar.barra progress { min-height: 5px; border-radius: 3px; background: @ACCENT@; }
.avviso { color: #f0b45a; font-size: 12px; padding: 4px 12px; }

/* barre di scorrimento sottili e intonate al tema; niente tratteggi di GTK */
scrollbar, scrollbar trough { background: transparent; border: none; }
scrollbar slider { background: rgba(@RGB@,0.35); min-width: 6px; min-height: 30px;
                   border-radius: 3px; border: none; }
scrollbar slider:hover { background: rgba(@RGB@,0.55); }
scrolledwindow undershoot, scrolledwindow overshoot { background: none; border: none; box-shadow: none; }
"""

# Temi: tutti i colori dell'interfaccia. RGB = colore d'accento in "r,g,b" per i veli.
TEMI = {
    "notte": {   # blu notte Nyctalus (predefinito)
        "BG": "#0a0e1a", "SIDEBAR": "#0c1120", "PANEL": "#0e1424", "TEXT": "#e6ecff",
        "MUTED": "#6b7797", "ACCENT": "#5aa9ff", "RGB": "90,169,255", "PILLTXT": "#cfe0ff",
        "SEND": "#2f6fe0", "SENDH": "#3b7cf0", "BUBIN": "#1b2740", "BUBINTXT": "#e6ecff",
        "OUT1": "#2f6fe0", "OUT2": "#3f82f2", "SHADOW": "0.30", "TRAMA": "0.05",
    },
    "ardesia": {   # nero-ardesia, alla Telegram
        "BG": "#0e1621", "SIDEBAR": "#17212b", "PANEL": "#17212b", "TEXT": "#f5f5f5",
        "MUTED": "#7f91a4", "ACCENT": "#6ab3f3", "RGB": "106,179,243", "PILLTXT": "#d6e9fb",
        "SEND": "#3390ec", "SENDH": "#4aa0f0", "BUBIN": "#182533", "BUBINTXT": "#f5f5f5",
        "OUT1": "#2b5278", "OUT2": "#33618d", "SHADOW": "0.35", "TRAMA": "0.05",
    },
    "viola": {   # viola notturno (come il Telegram degli screenshot)
        "BG": "#120f1c", "SIDEBAR": "#1a1626", "PANEL": "#1a1626", "TEXT": "#f1edff",
        "MUTED": "#8a82a6", "ACCENT": "#9d85ff", "RGB": "157,133,255", "PILLTXT": "#e2dbff",
        "SEND": "#7a5cf0", "SENDH": "#8b70f5", "BUBIN": "#241f35", "BUBINTXT": "#f1edff",
        "OUT1": "#6b4fe0", "OUT2": "#8a6cf2", "SHADOW": "0.35", "TRAMA": "0.05",
    },
    "chiaro": {   # chiaro, per il giorno
        "BG": "#eef1f6", "SIDEBAR": "#ffffff", "PANEL": "#ffffff", "TEXT": "#1a2233",
        "MUTED": "#7d889c", "ACCENT": "#2f6fe0", "RGB": "47,111,224", "PILLTXT": "#1f4fae",
        "SEND": "#2f6fe0", "SENDH": "#3b7cf0", "BUBIN": "#ffffff", "BUBINTXT": "#1a2233",
        "OUT1": "#2f6fe0", "OUT2": "#4a86f0", "SHADOW": "0.10", "TRAMA": "0.07",
    },
}
TEMA_DEF = "notte"

SFONDI = ("pipistrelli", "velo", "tinta")
SFONDO_DEF = "pipistrelli"
SFONDO = Path(__file__).resolve().parent / "assets" / "sfondo.svg"


def _url_trama(tema):
    """La trama a pipistrelli, ricolorata per il tema (scuro o chiaro)."""
    try:
        svg = SFONDO.read_text(encoding="utf-8")
    except Exception:            # noqa: BLE001
        return ""
    chiaro = tema.get("BG", "").lower() in ("#eef1f6",)
    colore = "#3a4a6e" if chiaro else "#9fb4e6"
    svg = svg.replace("#9fb4e6", colore).replace('fill-opacity="0.05"', 'fill-opacity="%s"' % tema["TRAMA"])
    dest = Path(GLib.get_user_cache_dir()) / "nyctalus-chat"
    try:
        dest.mkdir(parents=True, exist_ok=True)
        f = dest / ("trama-%s.svg" % colore.strip("#"))
        f.write_text(svg, encoding="utf-8")
        return f.as_uri()
    except Exception:            # noqa: BLE001
        return SFONDO.as_uri()


def _css(nome_tema=TEMA_DEF, nome_sfondo=SFONDO_DEF):
    """CSS completo per il tema e lo sfondo scelti."""
    tema = TEMI.get(nome_tema, TEMI[TEMA_DEF])
    rgb = tema["RGB"]
    velo = ("radial-gradient(circle at 20%% 0%%, rgba(%s,0.10), rgba(0,0,0,0) 45%%), "
            "radial-gradient(circle at 85%% 100%%, rgba(%s,0.08), rgba(0,0,0,0) 45%%)") % (rgb, rgb)
    if nome_sfondo == "pipistrelli":
        img = "url('%s'), %s" % (_url_trama(tema), velo)
        rep = "repeat, no-repeat, no-repeat"
    elif nome_sfondo == "velo":
        img, rep = velo, "no-repeat, no-repeat"
    else:
        img, rep = "none", "no-repeat"
    css = CSS_TMPL.replace("@CHATIMG@", img).replace("@CHATREP@", rep)
    for k, v in tema.items():
        css = css.replace("@%s@" % k, v)
    return css


# --- allegati: tipi, icone, apertura sicura -----------------------------------
TIPI_ANTEPRIMA = ("image/jpeg", "image/png", "image/gif", "image/webp", "image/bmp")
FILTRI = (
    ("tutti", ("*",)),
    ("immagini", ("image/*",)),
    ("video", ("video/*",)),
    ("audio", ("audio/*",)),
    ("documenti", ("application/pdf", "text/*", "*.odt", "*.ods", "*.odp", "*.doc", "*.docx",
                   "*.xls", "*.xlsx", "*.ppt", "*.pptx", "*.rtf", "*.md", "*.csv")),
    ("archivi", ("*.zip", "*.7z", "*.tar", "*.gz", "*.xz", "*.rar")),
)


def mime_di(percorso):
    tipo, _incerto = Gio.content_type_guess(percorso, None)
    return Gio.content_type_get_mime_type(tipo) or "application/octet-stream"


def apribile(mime):
    """Tipi che si possono aprire direttamente senza rischio di eseguire
    codice: media, PDF, testo semplice. Il resto va solo mostrato in cartella."""
    return (mime.startswith(("image/", "video/", "audio/")) and mime != "image/svg+xml") \
        or mime in ("application/pdf", "text/plain")


def icona_tipo(mime):
    if mime.startswith("image/"):
        return "🖼"
    if mime.startswith("video/"):
        return "🎬"
    if mime.startswith("audio/"):
        return "🎵"
    if mime == "application/pdf" or mime.startswith("text/") or "document" in mime or "sheet" in mime:
        return "📄"
    if any(k in mime for k in ("zip", "compressed", "tar", "x-7z", "rar")):
        return "🗜"
    return "📦"


def dimensione_leggibile(n):
    try:
        n = float(n)
    except Exception:            # noqa: BLE001
        return ""
    for unita in ("B", "KB", "MB", "GB"):
        if n < 1024 or unita == "GB":
            return ("%d %s" % (n, unita)) if unita == "B" else ("%.1f %s" % (n, unita))
        n /= 1024


def senza_metadati(percorso, mime):
    """Per le foto JPEG/PNG: ricodifica (ruotata correttamente) senza i
    metadati EXIF — niente posizione GPS né modello del telefono. Se qualcosa
    va storto si invia l'originale."""
    if mime not in ("image/jpeg", "image/png"):
        return percorso
    try:
        px = GdkPixbuf.Pixbuf.new_from_file(percorso)
        px = px.apply_embedded_orientation() or px
        cartella = Path(GLib.get_user_cache_dir()) / "nyctalus-chat" / "invio"
        cartella.mkdir(parents=True, exist_ok=True)
        dest = cartella / os.path.basename(percorso)
        if mime == "image/jpeg":
            px.savev(str(dest), "jpeg", ["quality"], ["92"])
        else:
            px.savev(str(dest), "png", [], [])
        return str(dest)
    except Exception:            # noqa: BLE001
        return percorso


def ultimo_accesso(ts):
    """«ultimo accesso oggi alle 18:05» (come Telegram), calcolato dal nostro motore."""
    if not ts:
        return _t("chat.offline")
    import datetime as dt
    d = dt.datetime.fromtimestamp(ts)
    oggi = dt.date.today()
    ora = d.strftime("%H:%M")
    if d.date() == oggi:
        return _t("chat.visto_oggi", ora=ora)
    if d.date() == oggi - dt.timedelta(days=1):
        return _t("chat.visto_ieri", ora=ora)
    return _t("chat.visto_data", data=d.strftime("%d/%m"), ora=ora)


def errore_leggibile(dettaglio):
    """L'errore del motore detto in modo comprensibile, col dettaglio tecnico in coda."""
    d = dettaglio.strip()
    if "SOCKS" in d or "servizio" in d or "service" in d:
        return ("Messaggio non consegnato: il contatto non è raggiungibile adesso (non è in linea, "
                "oppure non è nella stessa rete Nyctalus). Riprova quando risulta in linea. [%s]" % d)
    if "rete" in d.lower() or "network" in d.lower():
        return "Messaggio non consegnato: la rete Nyctalus non è pronta. [%s]" % d
    return "Operazione non riuscita: %s" % d


def riga_dell_evento(elenco, ev):
    """La riga dell'elenco su cui si è cliccato.

    Si parte dal widget che ha ricevuto il clic e si risale fino alla sua riga. NON si usa
    la coordinata ev.y: per le foto, i video e i cerchi dei file (aree di disegno con una
    finestra propria) è relativa a quell'area e non all'elenco, e indicava un'altra riga —
    con «Elimina per me» si cancellava il messaggio sbagliato."""
    w = Gtk.get_event_widget(ev)
    while w is not None and not isinstance(w, Gtk.ListBoxRow):
        w = w.get_parent()
    if w is not None and w.get_parent() is elenco:
        return w
    if ev.window == elenco.get_window():          # clic sull'elenco stesso: la coordinata è sua
        return elenco.get_row_at_y(int(ev.y))
    return None


def anteprima_da_riga(grezza):
    """(ts, testo breve) da una riga di cronologia: testo o allegato."""
    p = grezza.split(" ")
    try:
        if p[0] in ("in", "out") and len(p) >= 3:
            testo = _da_hex(p[2]).replace("\n", " ")
            return int(p[1]), (_t("chat.tu") + " " if p[0] == "out" else "") + testo
        if p[0] in ("fin", "fout") and len(p) >= 6:
            mime, nome = p[4], _da_hex(p[5])
            etichetta = _t("chat.foto") if mime in TIPI_ANTEPRIMA else nome
            return int(p[1]), (_t("chat.tu") + " " if p[0] == "fout" else "") + icona_tipo(mime) + " " + etichetta
    except Exception:            # noqa: BLE001
        pass
    return None, ""


def ora_o_data(ts):
    """Come Telegram: l'ora se è oggi, altrimenti il giorno."""
    try:
        d = datetime.fromtimestamp(int(ts))
    except Exception:            # noqa: BLE001
        return ""
    oggi = datetime.now()
    if d.date() == oggi.date():
        return d.strftime("%H:%M")
    if (oggi.date() - d.date()).days < 7:
        return d.strftime("%a")
    return d.strftime("%d/%m/%y")


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
        try:
            self.set_icon_name("nyctalus")
        except Exception:        # noqa: BLE001
            pass

        self.me = ""
        self.contatti = {}       # addr -> nome
        self.online = set()
        self.visti = {}          # ultimo accesso visto dal nostro motore (evento V)
        self.attivo = None
        self.righe_contatto = {} # addr -> Gtk.ListBoxRow
        self.sock = None
        self.filtro = ""
        self.trasferimenti = {}  # id → (riga, barra, etichetta) delle bolle in corso
        self.ultimo = {}         # addr → (ts, testo d'anteprima) per l'elenco
        self.lettore = media.LettoreAudio()

        # aspetto (tema + sfondo) ricordato fra un avvio e l'altro
        self.ui = self._carica_ui()
        self.prov = Gtk.CssProvider()
        Gtk.StyleContext.add_provider_for_screen(
            Gdk.Screen.get_default(), self.prov, Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION)
        self._applica_aspetto()
        self.get_style_context().add_class("chat-root")
        # dimensione/stato della finestra come all'ultima chiusura
        self.set_default_size(int(self.ui.get("larghezza", 880)), int(self.ui.get("altezza", 600)))
        if isinstance(self.ui.get("x"), int) and isinstance(self.ui.get("y"), int):
            self.move(self.ui["x"], self.ui["y"])     # dove l'avevi lasciata
        if self.ui.get("massimizzata"):
            self.maximize()
        self._salva_dim_pendente = 0
        self.connect("configure-event", self._finestra_cambiata)
        self.connect("window-state-event", self._stato_finestra)

        self._costruisci()
        self.connect("delete-event", lambda *_a: (self._salva_dimensioni(), False)[1])
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
        lato.set_size_request(258, -1)

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
        aiuto = Gtk.Button(label="?"); aiuto.get_style_context().add_class("pill")
        aiuto.set_tooltip_text(_t("chat.aiuto")); aiuto.set_valign(Gtk.Align.CENTER)
        aiuto.connect("clicked", self._apri_manuale)
        testa.pack_end(aiuto, False, False, 0)
        aspetto = Gtk.Button(label="⚙"); aspetto.get_style_context().add_class("pill")
        aspetto.set_tooltip_text(_t("chat.aspetto")); aspetto.set_valign(Gtk.Align.CENTER)
        aspetto.connect("clicked", self._menu_aspetto)
        testa.pack_end(aspetto, False, False, 0)
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
        self.lista.set_sort_func(self._ordina_righe)
        self.lista.connect("row-selected", self._scelto_contatto)
        self.lista.connect("button-press-event", self._click_lista)
        scorri.add(self.lista)
        lato.pack_start(scorri, True, True, 0)

        self.vuoto_contatti = Gtk.Label(label=_t("chat.nessun_contatto"))
        self.vuoto_contatti.get_style_context().add_class("hint")
        self.vuoto_contatti.set_justify(Gtk.Justification.CENTER)
        self.vuoto_contatti.set_line_wrap(True)
        self.vuoto_contatti.set_max_width_chars(24)
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
        self.mio_id.set_max_width_chars(16)   # non allarga la colonna
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
        puntini = Gtk.Button(label="⋮"); puntini.get_style_context().add_class("puntini")
        puntini.set_tooltip_text(_t("chat.menu.altro")); puntini.set_valign(Gtk.Align.CENTER)
        puntini.connect("clicked", lambda b: self.attivo and self._menu_contatto(self.attivo).popup_at_widget(
            b, Gdk.Gravity.SOUTH_EAST, Gdk.Gravity.NORTH_EAST, None))
        self.testa_conv.pack_end(puntini, False, False, 0)
        destra.pack_start(self.testa_conv, False, False, 0)

        # pila: stato vuoto / conversazione
        self.pila = Gtk.Stack()
        self.pila.set_transition_type(Gtk.StackTransitionType.CROSSFADE)
        self.pila.get_style_context().add_class("chat-bg")   # trama su tutta l'area
        destra.pack_start(self.pila, True, True, 0)

        self.pila.add_named(self._pannello_vuoto(), "vuoto")

        self.scorri_conv = Gtk.ScrolledWindow()
        self.scorri_conv.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        self.conv = Gtk.ListBox()
        self.conv.set_selection_mode(Gtk.SelectionMode.NONE)
        self.conv.connect("button-press-event", self._click_conversazione)
        self.conv.get_style_context().add_class("conversazione")
        self.scorri_conv.add(self.conv)
        self.pila.add_named(self.scorri_conv, "conv")
        # trascina qui i file per inviarli (come in Telegram)
        self.scorri_conv.drag_dest_set(Gtk.DestDefaults.ALL, [], Gdk.DragAction.COPY)
        self.scorri_conv.drag_dest_add_uri_targets()
        self.scorri_conv.connect("drag-data-received", self._file_trascinati)

        # compositore
        self.comp = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=8)
        self.comp.get_style_context().add_class("compositore")
        graffetta = Gtk.Button(label="📎")
        graffetta.get_style_context().add_class("graffetta")
        graffetta.set_tooltip_text(_t("chat.allega"))
        graffetta.connect("clicked", self._menu_graffetta)
        self.comp.pack_start(graffetta, False, False, 0)
        faccina = Gtk.Button(label="😊")
        faccina.get_style_context().add_class("graffetta")
        faccina.set_tooltip_text(_t("chat.emoji"))
        faccina.connect("clicked", self._pannello_emoji)
        self.comp.pack_start(faccina, False, False, 0)
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
        self.comp.set_visible(False)      # compositore solo a chat aperta (stile Telegram)
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
        segreto = os.environ.get("NYCTALUS_CHAT_TOKEN", "")
        if segreto:                       # porta di controllo protetta: ci presentiamo
            self._cmd("AUTH " + segreto)
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
        elif tipo == "L" and len(parti) >= 4:
            # L <addr> <ultima riga di cronologia>
            self._aggiorna_anteprima(parti[1], " ".join(parti[2:]))
        elif tipo == "ST" and len(parti) >= 3:
            addr, stato = parti[1], parti[2]
            if stato == "1":
                self.online.add(addr)
            else:
                self.online.discard(addr)
            self._aggiorna_pallino(addr)
            if addr == self.attivo:
                self._aggiorna_testa()
        elif tipo == "V" and len(parti) >= 3:
            self.visti[parti[1]] = int(parti[2])
            if parti[1] == self.attivo:
                self._aggiorna_testa()
        elif tipo == "MSG" and len(parti) >= 4:
            addr, ts, hx = parti[1], parti[2], parti[3]
            grezza = "in %s %s" % (ts, hx)
            self._aggiorna_anteprima(addr, grezza, nuovo=True)
            if addr == self.attivo:
                self._bolla("in", _da_hex(hx), ts, grezza)
        elif tipo == "OUT" and len(parti) >= 4:
            addr, ts, hx = parti[1], parti[2], parti[3]
            grezza = "out %s %s" % (ts, hx)
            self._aggiorna_anteprima(addr, grezza)
            if addr == self.attivo:
                self._bolla("out", _da_hex(hx), ts, grezza)
        elif tipo == "P" and len(parti) >= 8 and parti[1] in ("fin", "fout"):
            # P fin|fout <ts> <id> <dim> <mime> <nome hex> <percorso hex>
            self._bolla_allegato("out" if parti[1] == "fout" else "in", parti[2], parti[3],
                                 parti[4], parti[5], _da_hex(parti[6]), _da_hex(parti[7]), riga[2:])
        elif tipo == "P" and len(parti) >= 4:
            direzione, ts, hx = parti[1], parti[2], parti[3]
            self._bolla("out" if direzione == "out" else "in", _da_hex(hx), ts, riga[2:])
        elif tipo == "FILE" and len(parti) >= 9:
            # FILE <addr> <in|out> <ts> <id> <dim> <mime> <nome hex> <percorso hex>
            grezza = " ".join(["fout" if parti[2] == "out" else "fin"] + parti[3:9])
            self._aggiorna_anteprima(parti[1], grezza, nuovo=(parti[2] == "in"))
            if parti[1] == self.attivo:
                self._bolla_allegato(parti[2], parti[3], parti[4], parti[5], parti[6],
                                     _da_hex(parti[7]), _da_hex(parti[8]), grezza)
        elif tipo == "PROG" and len(parti) >= 6:
            # PROG <addr> <in|out> <id> <fatti> <totale>
            if parti[1] == self.attivo:
                mime = parti[6] if len(parti) >= 8 else ""
                nome = _da_hex(parti[7]) if len(parti) >= 8 else ""
                self._avanzamento(parti[2], parti[3], int(parti[4]), int(parti[5]), mime, nome)
        elif tipo == "FERR" and len(parti) >= 4:
            # FERR <addr> <id|-> <messaggio hex>
            if parti[1] == self.attivo:
                self._togli_trasferimento(parti[2])
                self._avviso_inline("⚠ " + _da_hex(parti[3]))
        elif tipo == "NEW" and len(parti) >= 2:
            self._contatto_sconosciuto(parti[1])
        elif tipo == "ERR":
            # un invio non riuscito (prima veniva ignorato: «non succede niente»)
            self._avviso_inline("⚠ " + errore_leggibile(riga[4:]))
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
        col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=1)
        alto = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        et = Gtk.Label(label=nome, xalign=0); et.get_style_context().add_class("nome")
        et.set_ellipsize(Pango.EllipsizeMode.END)
        et.set_max_width_chars(14)
        riga.etichetta = et
        spillo = Gtk.Label(label="📌"); spillo.get_style_context().add_class("spillo")
        spillo.set_no_show_all(True)
        riga.spillo = spillo
        ora = Gtk.Label(label="", xalign=1); ora.get_style_context().add_class("ora-riga")
        riga.ora = ora
        alto.pack_start(et, True, True, 0)
        alto.pack_start(spillo, False, False, 0)
        alto.pack_end(ora, False, False, 0)
        basso = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=4)
        st = Gtk.Label(label=_t("chat.offline"), xalign=0); st.get_style_context().add_class("stato-riga")
        st.set_ellipsize(Pango.EllipsizeMode.END)
        st.set_max_width_chars(18)
        riga.stato = st
        badge = Gtk.Label(label=""); badge.get_style_context().add_class("badge")
        badge.set_no_show_all(True)
        riga.badge = badge
        basso.pack_start(st, True, True, 0)
        basso.pack_end(badge, False, False, 0)
        col.pack_start(alto, False, False, 0)
        col.pack_start(basso, False, False, 0)
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
        if addr not in self.ultimo:   # senza messaggi: mostra lo stato
            riga.stato.set_text(_t("chat.online") if on else _t("chat.offline"))
        self._aggiorna_badge(addr)

    def _aggiorna_anteprima(self, addr, grezza, nuovo=False):
        """Ultimo messaggio + ora nella riga del contatto; se arriva un
        messaggio nuovo da una chat non aperta, sale il contatore dei non letti."""
        ts, testo = anteprima_da_riga(grezza)
        if ts is None:
            return
        self.ultimo[addr] = (ts, testo)
        riga = self.righe_contatto.get(addr)
        if riga is not None:
            riga.stato.set_text(testo)
            riga.ora.set_text(ora_o_data(ts))
        if nuovo and addr != self.attivo:
            nl = self.ui["non_letti"]
            nl[addr] = int(nl.get(addr, 0)) + 1
            self._salva_ui()
        self._aggiorna_badge(addr)
        self.lista.invalidate_sort()

    def _aggiorna_badge(self, addr):
        riga = self.righe_contatto.get(addr)
        if riga is None:
            return
        n = int(self.ui["non_letti"].get(addr, 0))
        riga.badge.set_text(str(n) if n < 100 else "99+")
        riga.badge.set_visible(n > 0)
        riga.spillo.set_visible(addr in self.ui["fissati"])

    def _ordina_righe(self, r1, r2):
        """Prima i fissati, poi le chat con l'attività più recente."""
        f1, f2 = r1.addr in self.ui["fissati"], r2.addr in self.ui["fissati"]
        if f1 != f2:
            return -1 if f1 else 1
        t1, t2 = self.ultimo.get(r1.addr, (0, ""))[0], self.ultimo.get(r2.addr, (0, ""))[0]
        return (t2 > t1) - (t2 < t1)

    # --- menu contestuali ------------------------------------------------------
    def _voce(self, menu, testo, cb, pericolosa=False, icona=None):
        v = Gtk.MenuItem()
        riga = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        img = Gtk.Image()
        if icona and Gtk.IconTheme.get_default().has_icon(icona):
            img.set_from_icon_name(icona, Gtk.IconSize.MENU)
        img.set_size_request(16, 16)
        riga.pack_start(img, False, False, 0)
        et = Gtk.Label(label=testo, xalign=0)
        riga.pack_start(et, True, True, 0)
        if pericolosa:
            v.get_style_context().add_class("pericolosa")
        v.add(riga)
        v.connect("activate", lambda *_a: cb())
        menu.append(v)

    def _nuovo_menu(self):
        m = Gtk.Menu()
        m.get_style_context().add_class("nyct-menu")
        # finestrina del menu con trasparenza: niente angoli chiari attorno ai bordi arrotondati
        top = m.get_toplevel()
        visual = top.get_screen().get_rgba_visual()
        if visual is not None and top.get_screen().is_composited():
            top.set_visual(visual)
            top.set_app_paintable(True)
        return m

    def _menu_contatto(self, addr):
        menu = self._nuovo_menu()
        nl = int(self.ui["non_letti"].get(addr, 0))
        fiss = addr in self.ui["fissati"]
        self._voce(menu, _t("chat.menu.apri"), lambda: self._apri_contatto(addr), icona="document-open-symbolic")
        self._voce(menu, _t("chat.menu.letto") if nl else _t("chat.menu.non_letto"),
                   lambda: self._segna_letto(addr, bool(nl)), icona="mail-read-symbolic" if nl else "mail-unread-symbolic")
        self._voce(menu, _t("chat.menu.sblocca") if fiss else _t("chat.menu.fissa"),
                   lambda: self._fissa(addr, not fiss), icona="view-pin-symbolic")
        self._voce(menu, _t("chat.menu.rinomina"), lambda: self._dialogo_rinomina(addr), icona="document-edit-symbolic")
        self._voce(menu, _t("chat.menu.copia_id"), lambda: self._copia_testo(addr), icona="edit-copy-symbolic")
        menu.append(Gtk.SeparatorMenuItem())
        self._voce(menu, _t("chat.menu.cancella_cron"), lambda: self._cancella_cronologia(addr), True, "edit-clear-all-symbolic")
        self._voce(menu, _t("chat.menu.elimina_chat"), lambda: self._elimina_chat(addr), True, "user-trash-symbolic")
        menu.show_all()
        return menu

    def _click_lista(self, _w, ev):
        if ev.type != Gdk.EventType.BUTTON_PRESS or ev.button != 3:
            return False
        riga = riga_dell_evento(self.lista, ev)
        if riga is None:
            return False
        self._menu_contatto(riga.addr).popup_at_pointer(ev)
        return True

    def _click_conversazione(self, _w, ev):
        if ev.type != Gdk.EventType.BUTTON_PRESS or ev.button != 3:
            return False
        riga = riga_dell_evento(self.conv, ev)
        if riga is None or not getattr(riga, "grezza", None):
            return False
        self._menu_messaggio(riga).popup_at_pointer(ev)
        return True

    def _menu_messaggio(self, riga):
        menu = self._nuovo_menu()
        allegato = getattr(riga, "allegato", None)
        if allegato:
            percorso, mime, _nome = allegato
            if os.path.isfile(percorso):
                if apribile(mime):
                    self._voce(menu, _t("chat.apri"), lambda: self._apri_allegato(percorso, mime), icona="document-open-symbolic")
                self._voce(menu, _t("chat.mostra_cartella"), lambda: self._mostra_cartella(percorso), icona="folder-open-symbolic")
                self._voce(menu, _t("chat.menu.salva_come"), lambda: self._salva_come(percorso), icona="document-save-as-symbolic")
                self._voce(menu, _t("chat.menu.inoltra"), lambda: self._inoltra(file=percorso, mime=mime), icona="mail-forward-symbolic")
        else:
            testo = getattr(riga, "testo", "")
            self._voce(menu, _t("chat.menu.copia_testo"), lambda: self._copia_testo(testo), icona="edit-copy-symbolic")
            self._voce(menu, _t("chat.menu.inoltra"), lambda: self._inoltra(testo=testo), icona="mail-forward-symbolic")
        menu.append(Gtk.SeparatorMenuItem())
        self._voce(menu, _t("chat.menu.elimina_msg"), lambda: self._elimina_messaggio(riga), True, "user-trash-symbolic")
        menu.show_all()
        return menu

    # --- azioni dei menu -------------------------------------------------------
    def _apri_contatto(self, addr):
        riga = self.righe_contatto.get(addr)
        if riga is not None:
            self.lista.select_row(riga)

    def _segna_letto(self, addr, letto):
        if letto:
            self.ui["non_letti"].pop(addr, None)
        else:
            self.ui["non_letti"][addr] = max(1, int(self.ui["non_letti"].get(addr, 0)))
        self._salva_ui()
        self._aggiorna_badge(addr)

    def _fissa(self, addr, si):
        f = self.ui["fissati"]
        if si and addr not in f:
            f.append(addr)
        elif not si and addr in f:
            f.remove(addr)
        self._salva_ui()
        self._aggiorna_badge(addr)
        self.lista.invalidate_sort()

    def _copia_testo(self, testo):
        Gtk.Clipboard.get(Gdk.SELECTION_CLIPBOARD).set_text(testo, -1)

    def _conferma(self, titolo, corpo, pulsante):
        d = Gtk.MessageDialog(transient_for=self, modal=True, message_type=Gtk.MessageType.WARNING,
                              buttons=Gtk.ButtonsType.NONE, text=titolo)
        d.format_secondary_text(corpo)
        d.add_button(_t("chat.annulla"), Gtk.ResponseType.CANCEL)
        b = d.add_button(pulsante, Gtk.ResponseType.OK)
        b.get_style_context().add_class("destructive-action")
        ok = d.run() == Gtk.ResponseType.OK
        d.destroy()
        return ok

    def _dialogo_rinomina(self, addr):
        d = Gtk.Dialog(title=_t("chat.menu.rinomina"), transient_for=self, modal=True)
        d.add_button(_t("chat.annulla"), Gtk.ResponseType.CANCEL)
        d.add_button(_t("chat.menu.salva"), Gtk.ResponseType.OK)
        d.set_default_response(Gtk.ResponseType.OK)
        box = d.get_content_area(); box.set_border_width(14); box.set_spacing(8)
        e = Gtk.Entry(); e.set_text(self.contatti.get(addr, "")); e.set_activates_default(True)
        e.set_width_chars(30)
        box.pack_start(e, False, False, 0)
        d.show_all()
        if d.run() == Gtk.ResponseType.OK and e.get_text().strip():
            nome = e.get_text().strip()
            self._cmd("ADD %s %s" % (addr, nome.encode().hex()))
            self._upsert_contatto(addr, nome)
            riga = self.righe_contatto.get(addr)
            if riga is not None:   # avatar con la nuova iniziale
                for f in riga.get_child().get_children()[:1]:
                    riga.get_child().remove(f)
                riga.avatar = fai_avatar(nome, addr)
                riga.get_child().pack_start(riga.avatar, False, False, 0)
                riga.get_child().reorder_child(riga.avatar, 0)
                riga.show_all()
                self._aggiorna_pallino(addr)
            if addr == self.attivo:
                self._aggiorna_testa()
        d.destroy()

    def _cancella_cronologia(self, addr):
        nome = self.contatti.get(addr, addr[:10])
        if not self._conferma(_t("chat.menu.cancella_cron") + "?",
                              _t("chat.conferma.cancella_cron") % nome, _t("chat.menu.cancella")):
            return
        self._cmd("CLEAR %s" % addr)
        self.ultimo.pop(addr, None)
        self.ui["non_letti"].pop(addr, None); self._salva_ui()
        self._aggiorna_pallino(addr)
        riga = self.righe_contatto.get(addr)
        if riga is not None:
            riga.ora.set_text("")
        if addr == self.attivo:
            for figlio in self.conv.get_children():
                self.conv.remove(figlio)

    def _elimina_chat(self, addr):
        nome = self.contatti.get(addr, addr[:10])
        if not self._conferma(_t("chat.menu.elimina_chat") + "?",
                              _t("chat.conferma.elimina_chat") % nome, _t("chat.menu.elimina")):
            return
        self._cmd("DEL %s" % addr)
        riga = self.righe_contatto.pop(addr, None)
        if riga is not None:
            self.lista.remove(riga)
        self.contatti.pop(addr, None)
        self.ultimo.pop(addr, None)
        self.ui["non_letti"].pop(addr, None)
        if addr in self.ui["fissati"]:
            self.ui["fissati"].remove(addr)
        self._salva_ui()
        self.vuoto_contatti.set_visible(len(self.contatti) == 0)
        if addr == self.attivo:
            self._mostra_vuoto()

    def _elimina_messaggio(self, riga):
        if not self._conferma(_t("chat.menu.elimina_msg") + "?", _t("chat.conferma.elimina_msg"),
                              _t("chat.menu.elimina")):
            return
        self._cmd("DELMSG %s %s" % (self.attivo, riga.grezza.encode().hex()))
        self.conv.remove(riga)
        # l'anteprima nell'elenco diventa l'ultimo messaggio rimasto
        rimaste = [r for r in self.conv.get_children() if getattr(r, "grezza", None)]
        if rimaste:
            self._aggiorna_anteprima(self.attivo, rimaste[-1].grezza)
        else:
            self.ultimo.pop(self.attivo, None)
            self._aggiorna_pallino(self.attivo)

    def _salva_come(self, percorso):
        d = Gtk.FileChooserNative.new(_t("chat.menu.salva_come"), self, Gtk.FileChooserAction.SAVE, None, None)
        d.set_current_name(os.path.basename(percorso))
        d.set_do_overwrite_confirmation(True)
        if d.run() == Gtk.ResponseType.ACCEPT:
            try:
                shutil.copy2(percorso, d.get_filename())
            except Exception as e:   # noqa: BLE001
                self._avviso_inline("⚠ %s" % e)
        d.destroy()

    def _inoltra(self, testo=None, file=None, mime=None):
        """Sceglie un contatto e gli rimanda il messaggio o l'allegato."""
        d = Gtk.Dialog(title=_t("chat.menu.inoltra_a"), transient_for=self, modal=True)
        d.add_button(_t("chat.annulla"), Gtk.ResponseType.CANCEL)
        d.add_button(_t("chat.menu.inoltra"), Gtk.ResponseType.OK)
        box = d.get_content_area(); box.set_border_width(12)
        elenco = Gtk.ListBox(); elenco.set_selection_mode(Gtk.SelectionMode.SINGLE)
        for addr, nome in sorted(self.contatti.items(), key=lambda kv: kv[1].lower()):
            r = Gtk.ListBoxRow(); r.addr = addr
            hb = Gtk.Box(spacing=10); hb.set_border_width(6)
            hb.pack_start(fai_avatar(nome, addr), False, False, 0)
            hb.pack_start(Gtk.Label(label=nome, xalign=0), True, True, 0)
            r.add(hb); elenco.add(r)
        sc = Gtk.ScrolledWindow(); sc.set_min_content_height(260); sc.set_min_content_width(300)
        sc.add(elenco); box.pack_start(sc, True, True, 0)
        d.show_all()
        if d.run() == Gtk.ResponseType.OK and elenco.get_selected_row() is not None:
            dest = elenco.get_selected_row().addr
            if testo is not None:
                self._cmd("SEND %s %s" % (dest, testo.encode().hex()))
            elif file:
                self._cmd("SENDFILE %s %s %s" % (dest, file.encode().hex(), mime or ""))
        d.destroy()

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
        if self.ui["non_letti"].pop(riga.addr, None) is not None:
            self._salva_ui()
            self._aggiorna_badge(riga.addr)
        self.comp.set_sensitive(True)
        self.comp.set_visible(True)          # il compositore compare con la chat
        self.comp.show_all()
        self.testa_conv.set_visible(True)
        self._aggiorna_testa()
        for figlio in self.conv.get_children():
            self.conv.remove(figlio)
        self.trasferimenti = {}
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
        self.conv_sotto.set_text(_t("chat.online") if on else ultimo_accesso(self.visti.get(self.attivo)))
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
    def _nuova_bolla(self, direzione, ts, extra=None):
        """Crea riga + bolla (allineata a destra se nostra) e restituisce
        (riga, bolla) per riempirla; l'ora va in fondo alla bolla."""
        riga = Gtk.ListBoxRow()
        riga.set_selectable(False)
        contenitore = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL)
        bolla = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        bolla.get_style_context().add_class("bolla")
        bolla.get_style_context().add_class("bolla-out" if direzione == "out" else "bolla-in")
        if extra:
            bolla.get_style_context().add_class(extra)
        if direzione == "out":
            contenitore.pack_end(bolla, False, False, 0)
        else:
            contenitore.pack_start(bolla, False, False, 0)
        riga.add(contenitore)
        return riga, bolla

    def _chiudi_bolla(self, riga, bolla, ts):
        if ts is not None:
            oraet = Gtk.Label(label=ora_hhmm(ts), xalign=1); oraet.get_style_context().add_class("ora")
            bolla.pack_start(oraet, False, False, 0)
        riga.show_all()
        self.conv.add(riga)
        GLib.idle_add(self._scorri_in_fondo)

    def _bolla(self, direzione, testo, ts, grezza=None):
        riga, bolla = self._nuova_bolla(direzione, ts)
        riga.grezza, riga.testo = grezza, testo
        et = Gtk.Label(label=testo, xalign=0); et.get_style_context().add_class("testo")
        et.set_line_wrap(True)
        et.set_line_wrap_mode(Pango.WrapMode.WORD_CHAR)
        et.set_max_width_chars(46)
        bolla.pack_start(et, False, False, 0)
        self._chiudi_bolla(riga, bolla, ts)

    # --- allegati (stile Telegram) ---------------------------------------------
    def _bolla_allegato(self, direzione, ts, ident, dim, mime, nome, percorso, grezza=None):
        """Foto e video a tutta bolla con l'ora sopra; audio con lettore nella
        chat; gli altri file con il cerchio colorato dell'estensione."""
        self._togli_trasferimento(ident)
        esiste = bool(percorso) and os.path.isfile(percorso)
        ora = ora_hhmm(ts)
        visuale = esiste and (mime in TIPI_ANTEPRIMA or mime.startswith("video/"))
        riga, bolla = self._nuova_bolla(direzione, ts, "foto" if visuale else "allegato")
        riga.grezza, riga.allegato = grezza, (percorso, mime, nome)
        if visuale and mime in TIPI_ANTEPRIMA and media.dimensione_logica(percorso):
            w, h = media.dimensione_logica(percorso)
            img = media.ImmagineArrotondata(percorso, w, h, ora=ora)
            self._cliccabile(img, lambda: self._apri_allegato(percorso, mime))
            img.set_tooltip_text("%s · %s" % (nome, dimensione_leggibile(dim)))
            bolla.pack_start(img, False, False, 0)
            return self._chiudi_bolla(riga, bolla, None)
        if visuale and mime.startswith("video/"):
            img = media.ImmagineArrotondata(None, media.LATO_FOTO, int(media.LATO_FOTO * 9 / 16), ora=ora, play=True)
            self._cliccabile(img, lambda: self._apri_allegato(percorso, mime))
            img.set_tooltip_text("%s · %s" % (nome, dimensione_leggibile(dim)))

            def pronta(jpg, durata, img=img):
                if jpg:
                    info = media.dimensione_logica(jpg)
                    img.imposta(jpg, *(info or (img.w, img.h)))
                img.durata = media.durata_leggibile(durata) or dimensione_leggibile(dim)
                img.queue_draw()
            media.miniatura_video(percorso, pronta)
            bolla.pack_start(img, False, False, 0)
            return self._chiudi_bolla(riga, bolla, None)

        # scheda: cerchio colorato + nome + dettagli
        fam = media.famiglia(mime, nome)
        audio = esiste and fam == "audio" and self.lettore.disponibile()
        cerchio = media.CerchioFile(media.COLORI_TIPO[fam], media.estensione(nome),
                                    simbolo="play" if audio else None)
        scheda = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
        scheda.pack_start(cerchio, False, False, 0)
        col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2)
        col.set_valign(Gtk.Align.CENTER)
        n = Gtk.Label(label=nome, xalign=0); n.get_style_context().add_class("nome-file")
        n.set_ellipsize(Pango.EllipsizeMode.MIDDLE); n.set_max_width_chars(28)
        dettagli = dimensione_leggibile(dim) + ("" if fam == "audio" else " · " + media.estensione(nome))
        d = Gtk.Label(label=dettagli if esiste else _t("chat.file_mancante"), xalign=0)
        d.get_style_context().add_class("dim-file")
        col.pack_start(n, False, False, 0); col.pack_start(d, False, False, 0)
        scheda.pack_start(col, True, True, 0)
        bolla.pack_start(scheda, False, False, 0)
        if audio:
            def aggiorna(frazione, pos, durata, suona, cerchio=cerchio, d=d):
                if suona is not None:
                    cerchio.imposta(None, "pausa" if suona else "play")
                if pos is not None and durata:
                    d.set_text("%s / %s" % (media.durata_leggibile(pos), media.durata_leggibile(durata)))
                elif suona is False:
                    d.set_text(dettagli)
            media.durata_in_sottofondo(percorso, lambda s: s and d.set_text(
                "%s · %s" % (media.durata_leggibile(s), dimensione_leggibile(dim))))
            self._cliccabile(cerchio, lambda: self.lettore.alterna(percorso, aggiorna))
        elif esiste:
            self._cliccabile(cerchio, lambda: self._apri_allegato(percorso, mime))
        self._chiudi_bolla(riga, bolla, ts)

    def _cliccabile(self, area, azione):
        """Clic sinistro → azione; il destro prosegue verso il menu del messaggio."""
        area.add_events(Gdk.EventMask.BUTTON_PRESS_MASK)
        area.connect("button-press-event", lambda _w, ev: ev.button == 1 and (azione() or True))
        area.connect("realize", lambda w: w.get_window().set_cursor(
            Gdk.Cursor.new_from_name(w.get_display(), "pointer")))

    def _avanzamento(self, direzione, ident, fatti, totale, mime="", nome=""):
        """Bolla del file in viaggio: cerchio con l'anello di avanzamento."""
        if ident not in self.trasferimenti:
            riga, bolla = self._nuova_bolla(direzione, None, "allegato")
            fam = media.famiglia(mime, nome)
            cerchio = media.CerchioFile(media.COLORI_TIPO[fam], media.estensione(nome or "file"))
            scheda = Gtk.Box(orientation=Gtk.Orientation.HORIZONTAL, spacing=12)
            scheda.pack_start(cerchio, False, False, 0)
            col = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=2); col.set_valign(Gtk.Align.CENTER)
            n = Gtk.Label(label=nome or "…", xalign=0); n.get_style_context().add_class("nome-file")
            n.set_ellipsize(Pango.EllipsizeMode.MIDDLE); n.set_max_width_chars(28)
            et = Gtk.Label(xalign=0); et.get_style_context().add_class("dim-file")
            col.pack_start(n, False, False, 0); col.pack_start(et, False, False, 0)
            scheda.pack_start(col, True, True, 0)
            bolla.pack_start(scheda, False, False, 0)
            self._chiudi_bolla(riga, bolla, None)
            self.trasferimenti[ident] = (riga, cerchio, et)
        _riga, cerchio, et = self.trasferimenti[ident]
        cerchio.imposta(min(1.0, (fatti / totale) if totale else 1.0))
        verbo = _t("chat.invio_in_corso") if direzione == "out" else _t("chat.ricezione")
        et.set_text("%s %s / %s" % (verbo, dimensione_leggibile(fatti), dimensione_leggibile(totale)))

    def _togli_trasferimento(self, ident):
        voce = self.trasferimenti.pop(ident, None)
        if voce is not None:
            try:
                self.conv.remove(voce[0])
            except Exception:    # noqa: BLE001
                pass

    def _avviso_inline(self, testo):
        riga = Gtk.ListBoxRow(); riga.set_selectable(False)
        et = Gtk.Label(label=testo); et.get_style_context().add_class("avviso")
        et.set_line_wrap(True); et.set_max_width_chars(60)
        riga.add(et); riga.show_all()
        self.conv.add(riga)
        GLib.idle_add(self._scorri_in_fondo)

    def _apri_allegato(self, percorso, mime):
        """Apre con il programma di sistema solo i tipi innocui; gli altri
        (eseguibili, script, archivi…) si mostrano nella cartella."""
        if not os.path.isfile(percorso):
            return
        if not apribile(mime):
            return self._mostra_cartella(percorso)
        try:
            Gtk.show_uri_on_window(self, GLib.filename_to_uri(percorso, None), Gdk.CURRENT_TIME)
        except Exception:        # noqa: BLE001
            self._mostra_cartella(percorso)

    def _mostra_cartella(self, percorso):
        try:
            Gtk.show_uri_on_window(self, GLib.filename_to_uri(os.path.dirname(percorso), None), Gdk.CURRENT_TIME)
        except Exception:        # noqa: BLE001
            pass

    # --- emoticon: pannello con categorie e «recenti» --------------------------------
    def _pannello_emoji(self, bottone):
        from . import emoji
        pop = Gtk.Popover.new(bottone)
        pop.get_style_context().add_class("pannello-emoji")
        scatola = Gtk.Box(orientation=Gtk.Orientation.VERTICAL, spacing=4)
        scatola.set_border_width(6)
        schede = Gtk.Box(spacing=0)
        schede.get_style_context().add_class("schede-emoji")
        titolo = Gtk.Label(xalign=0)
        titolo.get_style_context().add_class("titolo-emoji")
        bottoni_schede = []
        scorri = Gtk.ScrolledWindow()
        scorri.set_policy(Gtk.PolicyType.NEVER, Gtk.PolicyType.AUTOMATIC)
        scorri.set_size_request(400, 280)
        griglia = Gtk.FlowBox()
        griglia.set_selection_mode(Gtk.SelectionMode.NONE)
        griglia.set_max_children_per_line(10)
        griglia.set_min_children_per_line(8)
        griglia.set_homogeneous(True)
        scorri.add(griglia)
        categorie = [("🕘", _t("chat.emoji_recenti"), list(self.ui.get("emoji_recenti", [])))] + emoji.elenco()

        def inserisci(_b, e):
            pos = self.ingresso.get_position()
            self.ingresso.insert_text(e, pos)
            self.ingresso.set_position(pos + len(e))
            recenti = [e] + [x for x in self.ui.get("emoji_recenti", []) if x != e]
            self.ui["emoji_recenti"] = recenti[:32]
            self._salva_ui()

        def mostra(_b, k):
            titolo.set_text(categorie[k][1].upper())
            for i, bs in enumerate(bottoni_schede):
                ctx = bs.get_style_context()
                ctx.add_class("sel") if i == k else ctx.remove_class("sel")
            for figlio in griglia.get_children():
                griglia.remove(figlio)
            for e in categorie[k][2]:
                b = Gtk.Button(label=e)
                b.get_style_context().add_class("emoji")
                b.set_relief(Gtk.ReliefStyle.NONE)
                b.connect("clicked", inserisci, e)
                griglia.add(b)
            if not categorie[k][2]:
                griglia.add(Gtk.Label(label=_t("chat.emoji_vuoto")))
            griglia.show_all()

        for k, (icona, nome, _e) in enumerate(categorie):
            b = Gtk.Button(label=icona)
            b.set_relief(Gtk.ReliefStyle.NONE)
            b.set_tooltip_text(nome)
            b.connect("clicked", mostra, k)
            schede.pack_start(b, False, False, 0)
            bottoni_schede.append(b)
        scatola.pack_start(schede, False, False, 0)
        scatola.pack_start(titolo, False, False, 0)
        scatola.pack_start(scorri, True, True, 0)
        pop.add(scatola)
        mostra(None, 0 if self.ui.get("emoji_recenti") else 1)
        pop.show_all()
        pop.popup()

    # --- graffetta: menu, scelta, fotocamera, anteprima --------------------------
    def _menu_graffetta(self, _bottone):
        """📎 → il selettore di Telegram: griglia delle foto/video recenti con
        selezione numerata; le schede portano a fotocamera, video e file."""
        if not self.attivo:
            return
        chiavi = ("titolo", "galleria", "fotocamera", "scheda_file", "scheda_altre", "scheda_foto", "scheda_video",
                  "didascalia", "exif", "invia", "invia_n", "selezionati", "vuoto")
        dlg = media.SelettoreAllegati(self, {k: _t("chat.gal." + k) for k in chiavi}, self.ui.get("senza_exif", True))
        esito = dlg.run()
        scelti, testo, exif, azione = list(dlg.scelti), dlg.didascalia(), dlg.senza_exif(), dlg.azione
        dlg.destroy()
        if esito == Gtk.ResponseType.OK and scelti:
            for p in scelti:
                self._invia_file(p, mime_di(p), exif)
            if testo:
                self._cmd("SEND %s %s" % (self.attivo, testo.encode().hex()))
        elif esito == Gtk.ResponseType.ACCEPT:
            {"foto": lambda: self._scatta("foto"), "video": lambda: self._scatta("video"),
             "file": lambda: self._scegli_file("tutti"), "altre": lambda: self._scegli_file("media")}[azione]()

    def _scegli_file(self, tipo):
        d = Gtk.FileChooserNative.new(_t("chat.allega"), self, Gtk.FileChooserAction.OPEN, None, None)
        d.set_select_multiple(True)
        filtri = (("media", ("image/*", "video/*")),) + FILTRI if tipo == "media" else FILTRI
        for chiave, schemi in filtri:
            f = Gtk.FileFilter(); f.set_name(_t("chat.filtro." + chiave))
            for sch in schemi:
                (f.add_mime_type if "/" in sch else f.add_pattern)(sch)
            d.add_filter(f)
        percorsi = d.get_filenames() if d.run() == Gtk.ResponseType.ACCEPT else []
        d.destroy()
        if percorsi:
            self._prepara_invio(percorsi)

    def _file_trascinati(self, _w, ctx, _x, _y, dati, _info, tempo):
        percorsi = []
        for uri in dati.get_uris() or []:
            try:
                p, _h = GLib.filename_from_uri(uri)
            except Exception:    # noqa: BLE001
                continue
            if os.path.isfile(p):
                percorsi.append(p)
        Gtk.drag_finish(ctx, True, False, tempo)
        if self.attivo and percorsi:
            GLib.idle_add(lambda: (self._prepara_invio(percorsi), False)[1])

    def _scatta(self, modalita):
        testi = {k: _t("chat.cam." + k) for k in ("titolo_foto", "titolo_video", "scatta", "registra",
                                                  "stop", "annulla", "errore", "attendi")}
        cam = media.Fotocamera(self, modalita, testi)
        ok = cam.run() == Gtk.ResponseType.OK and cam.risultato
        risultato = cam.risultato
        cam.destroy()
        if ok:
            self._prepara_invio([risultato])

    def _prepara_invio(self, percorsi):
        """Anteprima con didascalia (come Telegram), poi l'invio vero."""
        voci = [(p, mime_di(p)) for p in percorsi]
        testi = {k: _t("chat.ant." + k) for k in ("titolo", "titolo1", "didascalia", "exif", "invia", "annulla")}
        dlg = media.AnteprimaInvio(self, voci, testi, self.ui.get("senza_exif", True), dimensione_leggibile)
        if dlg.run() == Gtk.ResponseType.OK and self.attivo:
            senza_exif = dlg.senza_exif()
            for p, m in voci:
                self._invia_file(p, m, senza_exif)
            if dlg.didascalia():
                self._cmd("SEND %s %s" % (self.attivo, dlg.didascalia().encode().hex()))
        dlg.destroy()

    def _invia_file(self, percorso, mime=None, senza_exif=True):
        mime = mime or mime_di(percorso)
        if senza_exif:
            percorso = senza_metadati(percorso, mime)
        self._cmd("SENDFILE %s %s %s" % (self.attivo, percorso.encode().hex(), mime))

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

    # --- aspetto: temi e sfondi -------------------------------------------------
    def _file_ui(self):
        return Path.home() / ".nyctalus-chat" / "ui.json"

    def _carica_ui(self):
        try:
            d = json.loads(self._file_ui().read_text(encoding="utf-8"))
        except Exception:        # noqa: BLE001
            d = {}
        if d.get("tema") not in TEMI:
            d["tema"] = TEMA_DEF
        if d.get("sfondo") not in SFONDI:
            d["sfondo"] = SFONDO_DEF
        if not isinstance(d.get("non_letti"), dict):
            d["non_letti"] = {}
        if not isinstance(d.get("fissati"), list):
            d["fissati"] = []
        if not isinstance(d.get("emoji_recenti"), list):
            d["emoji_recenti"] = []
        return d

    def _salva_ui(self):
        try:
            self._file_ui().parent.mkdir(parents=True, exist_ok=True)
            self._file_ui().write_text(json.dumps(self.ui, indent=2), encoding="utf-8")
        except Exception:        # noqa: BLE001
            pass

    def _finestra_cambiata(self, *_a):
        # salva poco dopo l'ultimo ridimensionamento (non a ogni pixel)
        if self._salva_dim_pendente:
            GLib.source_remove(self._salva_dim_pendente)
        self._salva_dim_pendente = GLib.timeout_add(600, self._salva_dimensioni)
        return False

    def _stato_finestra(self, _w, ev):
        self.ui["massimizzata"] = bool(ev.new_window_state & Gdk.WindowState.MAXIMIZED)
        self._salva_ui()
        return False

    def _salva_dimensioni(self):
        self._salva_dim_pendente = 0
        if not self.ui.get("massimizzata"):   # da massimizzata non sovrascrive la misura normale
            w, h = self.get_size()
            if w > 200 and h > 200:
                self.ui["larghezza"], self.ui["altezza"] = w, h
            x, y = self.get_position()
            self.ui["x"], self.ui["y"] = int(x), int(y)
        self._salva_ui()
        return False

    def _applica_aspetto(self):
        self.prov.load_from_data(_css(self.ui["tema"], self.ui["sfondo"]).encode())

    def _menu_aspetto(self, bottone):
        menu = self._nuovo_menu()

        def sezione(titolo):
            t = Gtk.MenuItem(label=titolo)
            t.set_sensitive(False)
            menu.append(t)

        def gruppo(chiave, valori):
            primo = None
            for v in valori:
                voce = Gtk.RadioMenuItem.new_with_label_from_widget(primo, _t("chat.%s.%s" % (chiave, v)))
                primo = primo or voce
                voce.set_active(self.ui[chiave] == v)
                voce.connect("toggled", self._scelto_aspetto, chiave, v)
                menu.append(voce)

        sezione(_t("chat.tema"))
        gruppo("tema", list(TEMI))
        menu.append(Gtk.SeparatorMenuItem())
        sezione(_t("chat.sfondo"))
        gruppo("sfondo", list(SFONDI))
        menu.append(Gtk.SeparatorMenuItem())
        exif = Gtk.CheckMenuItem(label=_t("chat.senza_exif"))
        exif.set_active(self.ui.get("senza_exif", True))
        exif.connect("toggled", lambda v: (self.ui.__setitem__("senza_exif", v.get_active()), self._salva_ui()))
        menu.append(exif)
        menu.show_all()
        menu.popup_at_widget(bottone, Gdk.Gravity.SOUTH_WEST, Gdk.Gravity.NORTH_WEST, None)

    def _scelto_aspetto(self, voce, chiave, valore):
        if not voce.get_active() or self.ui.get(chiave) == valore:
            return
        self.ui[chiave] = valore
        self._salva_ui()
        self._applica_aspetto()

    def _apri_manuale(self, _w):
        """Apre il manuale nel browser Nyctalus; se non c'è, nel browser di
        sistema."""
        env = dict(os.environ)
        env["PYTHONPATH"] = str(RADICE / "browser") + os.pathsep + env.get("PYTHONPATH", "")
        try:
            subprocess.Popen([sys.executable, "-m", "nyctalus_browser", MANUALE_URL],
                             env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        except Exception:        # noqa: BLE001
            import webbrowser
            webbrowser.open(MANUALE_URL)

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
