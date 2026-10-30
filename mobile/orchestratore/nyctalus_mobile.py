#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Orchestratore di Nyctalus per il telefono (gira dentro Termux, ma anche su PC).

Fa da "server.py" della versione mobile, come in Termux-NexusSEC-OS:
  - avvia e ferma i servizi Nyctalus (motore della chat, servizio .nyct, rete);
  - serve la web-app (PWA) solo su 127.0.0.1;
  - fa da ponte WebSocket ↔ porta di controllo del motore della chat;
  - riceve/serve gli allegati (con Range, per scorrere audio e video);
  - apre i file con le app del telefono (termux-open) o del PC (xdg-open).

Solo libreria standard: nessun `pip install`.

Sicurezza: su Android qualunque app può collegarsi a 127.0.0.1, quindi OGNI
richiesta deve presentare il segreto di accesso, che sta in
~/.nyctalus-mobile/token (cartella privata di Termux). L'APK lo scrive lì con
l'intent RUN_COMMAND (permesso concesso dall'utente) e apre
http://127.0.0.1:PORTA/?t=SEGRETO; il server lo trasforma in un cookie.
"""
import asyncio
import base64
import hashlib
import hmac
import json
import mimetypes
import os
import re
import secrets
import shutil
import signal
import struct
import socket
import subprocess
import sys
import threading
import time
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

VERSIONE = "0.1"
HOME = Path.home()
DATI_CHAT = HOME / ".nyctalus-chat"
DATI = HOME / ".nyctalus-mobile"
FILE_SEGRETO = DATI / "token"
CONFIG = DATI / "config.json"
INVIO = DATI / "invio"                       # file caricati dal telefono, in attesa di invio
CACHE = DATI / "cache"
WEBAPP = Path(os.environ.get("NYCTALUS_WEBAPP", Path(__file__).resolve().parent.parent / "webapp"))

# Porte: blocco dedicato (18xxx), così Nyctalus convive con gli altri progetti in
# Termux (es. NexusSEC sulla 8000, Tor sulla 9050, proxy SOCKS sulla 1080).
# Tutte modificabili in ~/.nyctalus-mobile/config.json → "porte": {...}.
PORTE_DEFAULT = {"http": 18731, "controllo": 18091, "backend": 18082, "socks": 18080,
                 "socks_locale": 18086, "monitor": 18596, "nodi": [18840, 18841, 18842, 18843],
                 "socks_browser": 18733}


def leggi_config():
    try:
        return json.loads(CONFIG.read_text(encoding="utf-8"))
    except Exception:            # noqa: BLE001
        return {}


def scrivi_config(cfg):
    DATI.mkdir(parents=True, exist_ok=True)
    tmp = CONFIG.with_suffix(".tmp")
    tmp.write_text(json.dumps(cfg, indent=1) + "\n", encoding="utf-8")
    os.chmod(tmp, 0o600)
    tmp.replace(CONFIG)


PREFISSO_CODICE = "NYCT1:"


def leggi_codice(testo):
    """Codice di rete «NYCT1:…» (lo crea `nyctalus-rete codice` sul PC del gestore)
    → {nome, directory_url, authority}. Contiene solo la chiave PUBBLICA dell'autorità."""
    testo = (testo or "").strip()
    if not testo.startswith(PREFISSO_CODICE) or len(testo) > 2000:
        raise ValueError("non è un codice di rete Nyctalus (inizia con %s)" % PREFISSO_CODICE)
    b = testo[len(PREFISSO_CODICE):]
    try:
        cfg = json.loads(base64.urlsafe_b64decode(b + "=" * (-len(b) % 4)))
    except Exception:            # noqa: BLE001
        raise ValueError("codice di rete rovinato: copialo per intero")
    url, aut = str(cfg.get("directory_url", "")), str(cfg.get("authority", ""))
    if not re.match(r"^https?://[^\s]+$", url) or not re.fullmatch(r"[0-9a-f]{64}", aut):
        raise ValueError("codice di rete incompleto")
    return {"nome": str(cfg.get("nome") or "Rete Nyctalus")[:60], "directory_url": url, "authority": aut}


PORTE = dict(PORTE_DEFAULT, **{k: v for k, v in leggi_config().get("porte", {}).items() if k in PORTE_DEFAULT})
PORTA_HTTP = int(os.environ.get("NYCTALUS_MOBILE_PORT", PORTE["http"]))
CONTROLLO = ("127.0.0.1", int(PORTE["controllo"]))
BACKEND = "127.0.0.1:%d" % int(PORTE["backend"])
MAX_CARICAMENTO = 200 * 1024 * 1024
MAX_INTESTAZIONE = 16 * 1024
MAX_FRAME_WS = 1024 * 1024
WS_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"


def log(*a):
    print(time.strftime("%H:%M:%S"), *a, flush=True)


# --- segreto di accesso -------------------------------------------------------------
class Segreto:
    """Il segreto vive nel file; lo rileggiamo se cambia (l'APK può riscriverlo)."""

    def __init__(self):
        self._valore, self._mtime = None, None

    def valore(self):
        try:
            st = FILE_SEGRETO.stat()
            if st.st_mtime != self._mtime:
                v = FILE_SEGRETO.read_text(encoding="utf-8").strip()
                self._valore, self._mtime = (v if len(v) >= 16 else None), st.st_mtime
        except FileNotFoundError:
            self._valore = None
        return self._valore

    def crea_se_manca(self):
        if self.valore():
            return self.valore()
        DATI.mkdir(parents=True, exist_ok=True)
        v = secrets.token_hex(16)
        FILE_SEGRETO.write_text(v, encoding="utf-8")
        os.chmod(FILE_SEGRETO, 0o600)
        return self.valore()

    def verifica(self, candidato):
        atteso = self.valore()
        return bool(atteso and candidato) and hmac.compare_digest(atteso, candidato)


SEGRETO = Segreto()


# --- servizi Nyctalus ---------------------------------------------------------------
def trova_binario():
    candidati = [os.environ.get("NYCTALUS_BIN", ""),
                 str(Path(__file__).resolve().parents[2] / "target" / "release" / "nyctalus"),
                 str(HOME / "nyctalus" / "bin" / "nyctalus"),
                 shutil.which("nyctalus") or ""]
    for c in candidati:
        if c and os.path.isfile(c) and os.access(c, os.X_OK):
            return c
    return None


def porta_libera(porta):
    with socket.socket() as s:
        return s.connect_ex(("127.0.0.1", porta)) != 0


class Servizi:
    """Porta su (e giù) motore della chat, rete e servizio .nyct — l'equivalente
    di avvia-chat.sh del PC, ma comandabile dalla web-app."""

    def __init__(self):
        self.proc, self.stato, self.dettaglio, self.modalita = [], "fermo", "", ""
        self.indirizzo = ""
        self.segreto_motore = secrets.token_hex(16)
        self.lock = threading.Lock()

    def info(self):
        return {"stato": self.stato, "dettaglio": self.dettaglio, "modalita": self.modalita,
                "rete": leggi_config().get("nome_rete", ""),
                "socks_browser": int(PORTE["socks_browser"]),
                "indirizzo": self.indirizzo, "binario": bool(trova_binario())}

    def _lancia(self, argomenti, log_file, env=None):
        f = open(log_file, "ab")
        p = subprocess.Popen(argomenti, stdout=f, stderr=subprocess.STDOUT, env=env,
                             start_new_session=True)
        self.proc.append(p)
        return p

    def avvia(self):
        with self.lock:
            if self.stato in ("avvio", "pronto"):
                return
            self.stato, self.dettaglio = "avvio", "motore"
        threading.Thread(target=self._avvia, daemon=True).start()

    def _attendi(self, file_log, schema, secondi=15):
        fine = time.time() + secondi
        while time.time() < fine:
            try:
                m = re.search(schema, Path(file_log).read_text(errors="replace"))
                if m:
                    return m.group(0)
            except FileNotFoundError:
                pass
            time.sleep(0.2)
        return None

    def _avvia(self):
        try:
            binario = trova_binario()
            if not binario:
                raise RuntimeError("binario 'nyctalus' non trovato (vedi l'installatore)")
            lavoro = DATI / "servizi"
            if lavoro.exists():
                shutil.rmtree(lavoro, ignore_errors=True)
            lavoro.mkdir(parents=True)
            DATI_CHAT.mkdir(parents=True, exist_ok=True)
            cfg = leggi_config()
            pubblica = bool(cfg.get("directory_url") and cfg.get("authority"))
            porta_socks = int(PORTE["socks"] if pubblica else PORTE["socks_locale"])
            socks = "127.0.0.1:%d" % porta_socks
            occupate = [p for p in (CONTROLLO[1], int(PORTE["backend"]), porta_socks) if not porta_libera(p)]
            if occupate:
                raise RuntimeError("porte già usate da un altro programma: %s (cambiale in %s)"
                                   % (", ".join(map(str, occupate)), CONFIG))

            # 1) il motore della chat (parte subito: la web-app vede il Contact ID)
            env = dict(os.environ, NYCTALUS_CHAT_TOKEN=self.segreto_motore)
            self._lancia([binario, "chat", "--data", str(DATI_CHAT), "--backend", BACKEND,
                          "--control", "%s:%d" % CONTROLLO, "--socks", socks],
                         lavoro / "chat.log", env=env)
            addr = self._attendi(lavoro / "chat.log", r"[a-z2-7]{56}\.nyct", 10)
            if not addr:
                raise RuntimeError("il motore della chat non è partito")
            self.indirizzo = addr

            # 2) la rete
            if pubblica:
                self.modalita, self.dettaglio = "pubblica", "elenco dei nodi"
                dirf = lavoro / "dir.txt"
                with urllib.request.urlopen(cfg["directory_url"], timeout=25) as r:
                    dirf.write_bytes(r.read())
                autorita = cfg["authority"]
                self._lancia([binario, "start", "--directory", str(dirf), "--authority", autorita,
                              "--socks", socks, "--control", "127.0.0.1:%d" % int(PORTE["monitor"])],
                             lavoro / "demone.log")
            else:
                self.modalita, self.dettaglio = "locale", "rete di prova"
                out = subprocess.run([binario, "authority"], capture_output=True, text=True).stdout
                sec = re.search(r"secret:\s+([0-9a-f]+)", out).group(1)
                autorita = re.search(r"public:\s+([0-9a-f]+)", out).group(1)
                righe = []
                for porta in PORTE["nodi"]:
                    if not porta_libera(int(porta)):
                        raise RuntimeError("porta %s già usata da un altro programma" % porta)
                    lf = lavoro / ("n%d.log" % porta)
                    self._lancia([binario, "node", "--listen", "127.0.0.1:%d" % porta], lf)
                    r = self._attendi(lf, r"127\.0\.0\.1:%d,[0-9a-f]+,[0-9a-f]+" % porta)
                    if not r:
                        raise RuntimeError("nodo di prova %d non partito" % porta)
                    righe.append(r)
                dirf = lavoro / "dir.txt"
                dirf.write_text(subprocess.run([binario, "directory", "--authority-secret", sec,
                                                "--entries", ";".join(righe)],
                                               capture_output=True, text=True).stdout)
                self._lancia([binario, "circuit", "--directory", str(dirf), "--authority", autorita,
                              "--socks", socks, "--hops", "3", "--control", "127.0.0.1:%d" % int(PORTE["monitor"])],
                             lavoro / "demone.log")

            # 3) il nostro servizio .nyct (entrata), quando il SOCKS risponde
            self.dettaglio = "servizio .nyct"
            for _ in range(60):
                if not porta_libera(porta_socks):
                    break
                time.sleep(0.25)
            self._lancia([binario, "site", "--backend", BACKEND, "--identity", str(DATI_CHAT / "identita"),
                          "--directory", str(dirf), "--authority", autorita, "--hops", "3"],
                         lavoro / "site.log")
            self.stato, self.dettaglio = "pronto", ""
            log("servizi pronti (%s) — %s" % (self.modalita, self.indirizzo))
        except Exception as e:    # noqa: BLE001
            self.stato, self.dettaglio = "errore", str(e)
            log("errore avvio servizi:", e)
            self._termina()

    def _termina(self):
        for p in reversed(self.proc):
            try:
                os.killpg(p.pid, signal.SIGTERM)
            except Exception:    # noqa: BLE001
                pass
        self.proc = []

    def ferma(self):
        with self.lock:
            self._termina()
            self.stato, self.dettaglio, self.indirizzo, self.modalita = "fermo", "", "", ""


SERVIZI = Servizi()


# --- file: dove si può leggere, nomi sicuri ------------------------------------------
# Galleria del telefono: dopo `termux-setup-storage` Termux vede la memoria
# condivisa in ~/storage/. Solo LETTURA, e solo foto/video (per il selettore
# allegati in stile Telegram). NYCTALUS_GALLERIA (cartelle separate da ":")
# permette di indicarne altre, per esempio sul PC.
ESTENSIONI_MEDIA = {".jpg", ".jpeg", ".png", ".webp", ".gif", ".heic", ".mp4", ".mov", ".webm", ".3gp", ".mkv"}


def cartelle_galleria():
    extra = [Path(x) for x in os.environ.get("NYCTALUS_GALLERIA", "").split(os.pathsep) if x]
    if os.environ.get("NYCTALUS_GALLERIA_SOLO"):      # prove: mai le cartelle vere dell'utente
        return extra
    return extra + [HOME / "storage" / "dcim", HOME / "storage" / "pictures", HOME / "storage" / "movies"]


GALLERIA_CACHE = {"ts": 0.0, "voci": []}
GALLERIA_DURATA = 45          # secondi: riaprire la graffetta non riscandisce migliaia di file
GALLERIA_MAX = 300            # quante foto/video recenti al massimo (a pagine nella web-app)
GALLERIA_LOCK = threading.Lock()


def elenco_galleria_recente():
    """Come elenco_galleria, ma tenuto in memoria per qualche decina di secondi."""
    with GALLERIA_LOCK:
        if time.time() - GALLERIA_CACHE["ts"] > GALLERIA_DURATA:
            GALLERIA_CACHE["voci"] = elenco_galleria(GALLERIA_MAX)
            GALLERIA_CACHE["ts"] = time.time()
        return GALLERIA_CACHE["voci"]


def elenco_galleria(quanti=GALLERIA_MAX):
    """Foto e video più recenti (dal più nuovo), senza cartelle nascoste né miniature di sistema."""
    voci, visti = [], set()
    for radice in cartelle_galleria():
        if not radice.exists():
            continue
        for cart, sottocartelle, files in os.walk(radice, followlinks=True):
            profondita = len(Path(cart).relative_to(radice).parts)
            sottocartelle[:] = [d for d in sottocartelle if not d.startswith(".") and profondita < 3]
            for f in files:
                if f.startswith(".") or os.path.splitext(f)[1].lower() not in ESTENSIONI_MEDIA:
                    continue
                pf = os.path.join(cart, f)
                try:
                    reale, st = os.path.realpath(pf), os.stat(pf)
                except OSError:
                    continue
                if reale not in visti:
                    visti.add(reale)
                    voci.append((st.st_mtime, pf, st.st_size))
    voci.sort(reverse=True)
    return voci[:quanti]


def radici_lettura():
    return [DATI_CHAT / "allegati", INVIO, CACHE] + [g for g in cartelle_galleria() if g.exists()]


def leggibile(percorso):
    """Il file è dentro una delle cartelle consentite? (niente ../ né link fuori)"""
    try:
        reale = Path(percorso).resolve(strict=True)
    except Exception:            # noqa: BLE001
        return None
    for r in radici_lettura():
        try:
            reale.relative_to(r.resolve())
            return reale if reale.is_file() else None
        except Exception:        # noqa: BLE001
            continue
    return None


def nome_sicuro(nome):
    base = re.split(r"[\\/]", nome or "")[-1]
    base = "".join("_" if (ord(c) < 32 or c in ':*?"<>|') else c for c in base).strip().lstrip(".")
    return base[:120] or "file"


def percorso_libero(cartella, nome):
    p = cartella / nome
    stem, ext = os.path.splitext(nome)
    n = 2
    while p.exists():
        p = cartella / ("%s (%d)%s" % (stem, n, ext))
        n += 1
    return p


LATO_MINIATURA = 360
MINIATURE = ThreadPoolExecutor(max_workers=2)    # poche alla volta: il telefono resta reattivo


def miniatura_exif(percorso):
    """La miniatura che le fotocamere mettono già dentro il JPEG (EXIF, ~160 px): solo
    libreria standard, istantanea. Le si aggiunge l'orientamento della foto, così il
    browser la gira come l'originale."""
    with open(percorso, "rb") as f:
        testa = f.read(256 * 1024)
    if testa[:2] != b"\xff\xd8":
        return None
    k = 2
    while k + 4 <= len(testa) and testa[k] == 0xFF:
        tipo, lung = testa[k + 1], struct.unpack(">H", testa[k + 2:k + 4])[0]
        if tipo == 0xE1 and testa[k + 4:k + 10] == b"Exif\x00\x00":
            t = testa[k + 10:k + 2 + lung]
            o = "<" if t[:2] == b"II" else ">"
            def ifd(pos):
                n = struct.unpack(o + "H", t[pos:pos + 2])[0]
                voci = {}
                for e in range(n):
                    tag, _ty, _cnt, val = struct.unpack(o + "HHI4s", t[pos + 2 + 12 * e:pos + 14 + 12 * e])
                    voci[tag] = val
                return voci, struct.unpack(o + "I", t[pos + 2 + 12 * n:pos + 6 + 12 * n])[0]
            try:
                ifd0, prossimo = ifd(struct.unpack(o + "I", t[4:8])[0])
                orient = struct.unpack(o + "H", ifd0[0x0112][:2])[0] if 0x0112 in ifd0 else 1
                if not prossimo:
                    return None
                ifd1, _ = ifd(prossimo)
                ini = struct.unpack(o + "I", ifd1[0x0201])[0]
                dim = struct.unpack(o + "I", ifd1[0x0202])[0]
            except (KeyError, struct.error):
                return None
            mini = t[ini:ini + dim]
            if mini[:2] != b"\xff\xd8" or len(mini) != dim:
                return None
            if orient in range(2, 9):
                carico = (b"Exif\x00\x00MM\x00\x2a\x00\x00\x00\x08\x00\x01"
                          + struct.pack(">HHIHH", 0x0112, 3, 1, orient, 0) + b"\x00\x00\x00\x00")
                mini = mini[:2] + b"\xff\xe1" + struct.pack(">H", len(carico) + 2) + carico + mini[2:]
            return mini
        if tipo == 0xDA:          # inizio dell'immagine: niente EXIF
            return None
        k += 2 + lung
    return None


def miniatura(percorso):
    """JPEG piccolo (lato ~360 px) di una foto o di un video, in cache. In ordine:
    miniatura EXIF già nel file (istantanea: con migliaia di foto conta) → Pillow → ffmpeg.
    None se non si riesce: la web-app mostra un'icona invece della foto intera."""
    st = percorso.stat()
    chiave = hashlib.sha1(("%s|%d|%d" % (percorso, st.st_size, int(st.st_mtime))).encode()).hexdigest()
    CACHE.mkdir(parents=True, exist_ok=True)
    dest = CACHE / (chiave + ".jpg")
    if dest.exists():
        return dest
    video = (mimetypes.guess_type(str(percorso))[0] or "").startswith("video/")
    if not video:
        try:
            mini = miniatura_exif(percorso)
        except OSError:
            mini = None
        if mini:
            dest.write_bytes(mini)
            return dest
        try:
            from PIL import Image, ImageOps
            with Image.open(percorso) as im:
                im.draft("RGB", (LATO_MINIATURA, LATO_MINIATURA))   # JPEG: decodifica già ridotta
                im = ImageOps.exif_transpose(im).convert("RGB")
                im.thumbnail((LATO_MINIATURA, LATO_MINIATURA))
                im.save(dest, "JPEG", quality=80)
            return dest
        except Exception:        # noqa: BLE001  (Pillow assente o formato non suo)
            pass
    if shutil.which("ffmpeg"):
        subprocess.run(["ffmpeg", "-v", "error", "-y"] + (["-ss", "0.5"] if video else []) +
                       ["-i", str(percorso), "-frames:v", "1", "-vf", "scale=%d:-2" % LATO_MINIATURA, str(dest)],
                       capture_output=True, timeout=25)
        if dest.exists():
            return dest
    if not video and st.st_size <= 400 * 1024:
        return percorso          # foto già piccola: va bene così
    return None


def apri_con_app(percorso):
    """Apre il file con l'app del sistema (telefono: termux-open; PC: xdg-open)."""
    for cmd in ("termux-open", "xdg-open"):
        if shutil.which(cmd):
            subprocess.Popen([cmd, str(percorso)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                             start_new_session=True)
            return True
    return False


# --- HTTP + WebSocket minimi ----------------------------------------------------------
class Richiesta:
    def __init__(self, metodo, percorso, query, intestazioni):
        self.metodo, self.percorso, self.query, self.h = metodo, percorso, query, intestazioni

    def cookie(self, nome):
        for parte in self.h.get("cookie", "").split(";"):
            k, _, v = parte.strip().partition("=")
            if k == nome:
                return v
        return None


async def leggi_richiesta(lettore):
    dati = await lettore.readuntil(b"\r\n\r\n")
    if len(dati) > MAX_INTESTAZIONE:
        raise ValueError("intestazione troppo grande")
    righe = dati.decode("latin-1").split("\r\n")
    metodo, uri, _ver = righe[0].split(" ", 2)
    h = {}
    for r in righe[1:]:
        if ":" in r:
            k, v = r.split(":", 1)
            h[k.strip().lower()] = v.strip()
    url = urllib.parse.urlsplit(uri)
    return Richiesta(metodo, urllib.parse.unquote(url.path), dict(urllib.parse.parse_qsl(url.query)), h)


INTESTAZIONI_SICUREZZA = (
    "X-Content-Type-Options: nosniff\r\n"
    "Referrer-Policy: no-referrer\r\n"
    "Cache-Control: no-store\r\n"
    "Content-Security-Policy: default-src 'self'; img-src 'self' blob: data:; media-src 'self' blob:; "
    "connect-src 'self' ws://127.0.0.1:* ws://localhost:*; style-src 'self' 'unsafe-inline'; "
    "frame-ancestors 'none'\r\n"
)


async def rispondi(scrittore, stato, corpo=b"", tipo="text/plain; charset=utf-8", extra=""):
    if isinstance(corpo, str):
        corpo = corpo.encode()
    testa = ("HTTP/1.1 %s\r\nContent-Type: %s\r\nContent-Length: %d\r\n%s%sConnection: close\r\n\r\n"
             % (stato, tipo, len(corpo), INTESTAZIONI_SICUREZZA, extra))
    scrittore.write(testa.encode() + corpo)
    await scrittore.drain()


async def rispondi_json(scrittore, obj, stato="200 OK"):
    await rispondi(scrittore, stato, json.dumps(obj), "application/json")


async def servi_file(scrittore, req, percorso, tipo=None, extra=""):
    """File con supporto a Range (necessario per scorrere video e audio)."""
    tipo = tipo or mimetypes.guess_type(str(percorso))[0] or "application/octet-stream"
    totale = percorso.stat().st_size
    inizio, fine, stato = 0, totale - 1, "200 OK"
    m = re.match(r"bytes=(\d*)-(\d*)$", req.h.get("range", ""))
    if m and totale:
        if m.group(1):
            inizio = int(m.group(1))
            fine = int(m.group(2)) if m.group(2) else totale - 1
        else:
            inizio = max(0, totale - int(m.group(2)))
        fine = min(fine, totale - 1)
        if inizio > fine:
            return await rispondi(scrittore, "416 Range Not Satisfiable", b"",
                                  extra="Content-Range: bytes */%d\r\n" % totale)
        stato = "206 Partial Content"
    lunghezza = fine - inizio + 1 if totale else 0
    testa = ("HTTP/1.1 %s\r\nContent-Type: %s\r\nContent-Length: %d\r\nAccept-Ranges: bytes\r\n%s%s"
             "%sConnection: close\r\n\r\n" % (stato, tipo, lunghezza, INTESTAZIONI_SICUREZZA,
                                            ("Content-Range: bytes %d-%d/%d\r\n" % (inizio, fine, totale))
                                            if stato.startswith("206") else "", extra))
    scrittore.write(testa.encode())
    with open(percorso, "rb") as f:
        f.seek(inizio)
        resto = lunghezza
        while resto > 0:
            blocco = f.read(min(65536, resto))
            if not blocco:
                break
            scrittore.write(blocco)
            await scrittore.drain()
            resto -= len(blocco)


async def ws_invia(scrittore, testo):
    dati = testo.encode()
    n = len(dati)
    if n < 126:
        testa = bytes([0x81, n])
    elif n < 65536:
        testa = bytes([0x81, 126]) + n.to_bytes(2, "big")
    else:
        testa = bytes([0x81, 127]) + n.to_bytes(8, "big")
    scrittore.write(testa + dati)
    await scrittore.drain()


async def ws_ricevi(lettore, scrittore):
    """Restituisce il testo del prossimo messaggio, o None a connessione chiusa."""
    pezzi = b""
    while True:
        b0, b1 = await lettore.readexactly(2)
        fin, op, mascherato, n = b0 & 0x80, b0 & 0x0F, b1 & 0x80, b1 & 0x7F
        if n == 126:
            n = int.from_bytes(await lettore.readexactly(2), "big")
        elif n == 127:
            n = int.from_bytes(await lettore.readexactly(8), "big")
        if n > MAX_FRAME_WS or not mascherato:        # il client DEVE mascherare
            return None
        maschera = await lettore.readexactly(4)
        dati = bytearray(await lettore.readexactly(n))
        for i in range(n):
            dati[i] ^= maschera[i % 4]
        if op == 0x8:                                  # chiusura
            return None
        if op == 0x9:                                  # ping → pong
            scrittore.write(bytes([0x8A, len(dati)]) + bytes(dati[:125]))
            await scrittore.drain()
            continue
        if op == 0xA:
            continue
        pezzi += bytes(dati)
        if len(pezzi) > MAX_FRAME_WS:
            return None
        if fin:
            return pezzi.decode("utf-8", errors="replace")


async def ponte_chat(lettore, scrittore, req):
    """WebSocket ↔ porta di controllo del motore: le righe passano così come sono."""
    chiave = req.h.get("sec-websocket-key", "")
    accetta = base64.b64encode(hashlib.sha1((chiave + WS_GUID).encode()).digest()).decode()
    scrittore.write(("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
                     "Sec-WebSocket-Accept: %s\r\n\r\n" % accetta).encode())
    await scrittore.drain()
    try:
        m_lettore, m_scrittore = await asyncio.open_connection(*CONTROLLO)
    except Exception:            # noqa: BLE001
        await ws_invia(scrittore, "ERR motore non raggiungibile")
        return
    m_scrittore.write(("AUTH %s\n" % SERVIZI.segreto_motore).encode())
    await m_scrittore.drain()

    async def dal_motore():
        while True:
            riga = await m_lettore.readline()
            if not riga:
                break
            testo = riga.decode("utf-8", errors="replace").rstrip("\n")
            if testo != "AUTHOK":
                await ws_invia(scrittore, testo)

    async def dalla_webapp():
        while True:
            testo = await ws_ricevi(lettore, scrittore)
            if testo is None:
                break
            for riga in testo.splitlines():
                if riga.startswith("AUTH"):            # il segreto del motore resta qui
                    continue
                m_scrittore.write((riga + "\n").encode())
            await m_scrittore.drain()

    compiti = [asyncio.ensure_future(dal_motore()), asyncio.ensure_future(dalla_webapp())]
    try:
        await asyncio.wait(compiti, return_when=asyncio.FIRST_COMPLETED)
    except Exception:            # noqa: BLE001
        pass
    for c in compiti:
        c.cancel()
    m_scrittore.close()


async def gestisci(lettore, scrittore):
    try:
        req = await leggi_richiesta(lettore)
    except Exception:            # noqa: BLE001
        scrittore.close()
        return
    try:
        await instrada(req, lettore, scrittore)
    except (ConnectionError, asyncio.IncompleteReadError):
        pass
    except Exception as e:       # noqa: BLE001
        log("errore richiesta", req.percorso, e)
        try:
            await rispondi(scrittore, "500 Internal Server Error", "errore interno")
        except Exception:        # noqa: BLE001
            pass
    finally:
        try:
            scrittore.close()
        except Exception:        # noqa: BLE001
            pass


async def instrada(req, lettore, scrittore):
    # 1) accesso: ?t=SEGRETO una volta → cookie; poi il cookie
    if "t" in req.query and SEGRETO.verifica(req.query["t"]):
        resto = {k: v for k, v in req.query.items() if k != "t"}
        dest = req.percorso + ("?" + urllib.parse.urlencode(resto) if resto else "")
        return await rispondi(scrittore, "303 See Other", b"",
                              extra="Location: %s\r\nSet-Cookie: nyct=%s; Path=/; Max-Age=31536000; HttpOnly; SameSite=Strict\r\n"
                              % (dest, req.query["t"]))
    if not SEGRETO.verifica(req.cookie("nyct")):
        return await rispondi(scrittore, "403 Forbidden",
                              "Accesso negato: apri Nyctalus dall'app, oppure usa l'indirizzo "
                              "con ?t=… stampato da nyctalus-mobile.")
    # le richieste che cambiano qualcosa devono venire dalla nostra pagina
    if req.metodo == "POST" and req.h.get("origin") not in (None, "http://127.0.0.1:%d" % PORTA_HTTP,
                                                            "http://localhost:%d" % PORTA_HTTP):
        return await rispondi(scrittore, "403 Forbidden", "origine non ammessa")

    p = req.percorso
    if p == "/ws" and req.h.get("upgrade", "").lower() == "websocket":
        return await ponte_chat(lettore, scrittore, req)
    if p == "/api/stato":
        return await rispondi_json(scrittore, dict(SERVIZI.info(), versione=VERSIONE))
    if p == "/api/rete":
        if req.metodo == "POST":
            return await cambia_rete(lettore, scrittore, req)
        cfg = leggi_config()
        condivisa = bool(cfg.get("directory_url") and cfg.get("authority"))
        imp = ""
        if condivisa:
            h = hashlib.sha256(bytes.fromhex(cfg["authority"])).hexdigest()[:8].upper()
            imp = h[:4] + "-" + h[4:]          # uguale a quella che stampa nyctalus-rete sul PC
        return await rispondi_json(scrittore, {"condivisa": condivisa, "nome": cfg.get("nome_rete", ""),
                                               "url": cfg.get("directory_url", "") if condivisa else "",
                                               "impronta": imp})
    if p == "/api/servizi/avvia" and req.metodo == "POST":
        SERVIZI.avvia()
        return await rispondi_json(scrittore, SERVIZI.info())
    if p == "/api/servizi/ferma" and req.metodo == "POST":
        SERVIZI.ferma()
        return await rispondi_json(scrittore, SERVIZI.info())
    if p == "/api/file":
        f = leggibile(bytes.fromhex(req.query.get("p", "")).decode("utf-8", "replace"))
        if not f:
            return await rispondi(scrittore, "404 Not Found", "file non disponibile")
        return await servi_file(scrittore, req, f)
    if p == "/api/miniatura":
        f = leggibile(bytes.fromhex(req.query.get("p", "")).decode("utf-8", "replace"))
        m = await asyncio.get_running_loop().run_in_executor(MINIATURE, miniatura, f) if f else None
        if not m:
            return await rispondi(scrittore, "404 Not Found", "nessuna miniatura")
        # le miniature non cambiano (la chiave dipende da file, dimensione e data): la
        # WebView le tiene in cache e riaprire la galleria è immediato
        return await servi_file(scrittore, req, m, None if m == f else "image/jpeg",
                                extra="Cache-Control: private, max-age=86400\r\n")
    if p == "/api/galleria":
        tutte = await asyncio.get_running_loop().run_in_executor(None, elenco_galleria_recente)
        try:
            da, quante = max(0, int(req.query.get("da", "0"))), max(1, min(60, int(req.query.get("n", "30"))))
        except ValueError:
            da, quante = 0, 30
        voci = [{"p": pf.encode().hex(), "nome": os.path.basename(pf), "dim": dim, "ts": int(ts),
                 "mime": mimetypes.guess_type(pf)[0] or "application/octet-stream"}
                for ts, pf, dim in tutte[da:da + quante]]
        return await rispondi_json(scrittore, {"disponibile": any(g.exists() for g in cartelle_galleria()),
                                               "voci": voci, "altre": da + quante < len(tutte)})
    if p == "/api/apri" and req.metodo == "POST":
        f = leggibile(bytes.fromhex(req.query.get("p", "")).decode("utf-8", "replace"))
        return await rispondi_json(scrittore, {"ok": bool(f and apri_con_app(f))})
    if p == "/api/carica" and req.metodo == "POST":
        return await carica(lettore, scrittore, req)
    return await servi_statico(scrittore, req)


async def cambia_rete(lettore, scrittore, req):
    """Importa un codice di rete ({"codice": "NYCT1:…"}) o torna alla rete di prova
    ({"prova": true}); poi riavvia i servizi sulla rete nuova."""
    try:
        n = int(req.h.get("content-length", "0"))
    except ValueError:
        n = -1
    if not 0 < n <= 4096:
        return await rispondi_json(scrittore, {"errore": "richiesta non valida"}, "400 Bad Request")
    try:
        dati = json.loads(await lettore.readexactly(n))
        cfg = leggi_config()
        if dati.get("prova"):
            for k in ("directory_url", "authority", "nome_rete"):
                cfg.pop(k, None)
            nome = ""
        else:
            rete = leggi_codice(dati.get("codice", ""))
            cfg.update(directory_url=rete["directory_url"], authority=rete["authority"], nome_rete=rete["nome"])
            nome = rete["nome"]
    except ValueError as e:
        return await rispondi_json(scrittore, {"errore": str(e)}, "400 Bad Request")
    except Exception:            # noqa: BLE001
        return await rispondi_json(scrittore, {"errore": "richiesta non valida"}, "400 Bad Request")
    scrivi_config(cfg)
    log("rete:", ("«%s» (%s)" % (nome, cfg["directory_url"])) if nome else "prova locale")

    def riavvia():
        SERVIZI.ferma()
        time.sleep(0.5)
        SERVIZI.avvia()
    threading.Thread(target=riavvia, daemon=True).start()
    return await rispondi_json(scrittore, {"ok": True, "nome": nome})


async def carica(lettore, scrittore, req):
    """Il telefono manda il file (corpo grezzo, nome in ?nome=); lo salviamo in
    INVIO e restituiamo il percorso, che la web-app passa al motore (SENDFILE)."""
    try:
        n = int(req.h.get("content-length", "-1"))
    except ValueError:
        n = -1
    if n < 0 or n > MAX_CARICAMENTO:
        return await rispondi_json(scrittore, {"errore": "dimensione non valida"}, "413 Payload Too Large")
    INVIO.mkdir(parents=True, exist_ok=True)
    dest = percorso_libero(INVIO, nome_sicuro(req.query.get("nome", "file")))
    resto = n
    with open(dest, "wb") as f:
        while resto > 0:
            blocco = await lettore.read(min(65536, resto))
            if not blocco:
                break
            f.write(blocco)
            resto -= len(blocco)
    if resto:
        dest.unlink(missing_ok=True)
        return await rispondi_json(scrittore, {"errore": "caricamento interrotto"}, "400 Bad Request")
    return await rispondi_json(scrittore, {"percorso": str(dest)})


async def servi_statico(scrittore, req):
    rel = req.percorso.lstrip("/") or "index.html"
    f = (WEBAPP / rel).resolve()
    try:
        f.relative_to(WEBAPP.resolve())
    except ValueError:
        return await rispondi(scrittore, "404 Not Found", "non trovato")
    if not f.is_file():
        return await rispondi(scrittore, "404 Not Found", "non trovato")
    await servi_file(scrittore, req, f)


# --- inoltratore SOCKS per il browser dell'app -------------------------------------------
# Il browser (modalità Nyctalus) manda TUTTO qui. Da qui il traffico entra nella rete
# Nyctalus attraverso il SOCKS del demone. In più:
#  - una pagina di VERIFICA che esiste solo qui (http://verifica.nyctalus.invalid/…): il
#    browser la chiede prima di navigare; se non arriva, il traffico non passa da noi e il
#    browser non apre nulla («chiuso in caso di errore»);
#  - niente indirizzi locali (127.x, 10.x, 192.168.x, localhost…): i siti non devono poter
#    raggiungere i servizi del telefono o della rete di casa attraverso il browser.
DOMINIO_VERIFICA = "verifica.nyctalus.invalid"


def indirizzo_locale(host):
    import ipaddress
    h = host.strip("[]").lower()
    if h in ("localhost", "localhost.localdomain") or h.endswith(".localhost") or h.endswith(".local"):
        return True
    try:
        ip = ipaddress.ip_address(h)
    except ValueError:
        return False
    return ip.is_private or ip.is_loopback or ip.is_link_local or ip.is_unspecified or ip.is_multicast or ip.is_reserved


async def _socks_rispondi(scrittore, codice):
    scrittore.write(bytes([5, codice, 0, 1, 0, 0, 0, 0, 0, 0]))
    await scrittore.drain()


async def _incanala(da, a):
    try:
        while True:
            blocco = await da.read(65536)
            if not blocco:
                break
            a.write(blocco)
            await a.drain()
    except Exception:            # noqa: BLE001
        pass
    finally:
        try:
            a.close()
        except Exception:        # noqa: BLE001
            pass


async def socks_browser(lettore, scrittore):
    try:
        testa = await asyncio.wait_for(lettore.readexactly(2), 10)
        if testa[0] != 5:
            return scrittore.close()
        metodi = await lettore.readexactly(testa[1])
        if 0 not in metodi:
            scrittore.write(b"\x05\xff")
            return scrittore.close()
        scrittore.write(b"\x05\x00")
        ver, cmd, _r, atyp = await asyncio.wait_for(lettore.readexactly(4), 10)
        if atyp == 1:
            host = socket.inet_ntop(socket.AF_INET, await lettore.readexactly(4))
        elif atyp == 4:
            host = socket.inet_ntop(socket.AF_INET6, await lettore.readexactly(16))
        elif atyp == 3:
            host = (await lettore.readexactly((await lettore.readexactly(1))[0])).decode("ascii", "replace")
        else:
            return await _socks_rispondi(scrittore, 8)
        porta = int.from_bytes(await lettore.readexactly(2), "big")
        if ver != 5 or cmd != 1:                       # solo CONNECT (niente UDP: niente fughe)
            return await _socks_rispondi(scrittore, 7)
        if host.lower() == DOMINIO_VERIFICA:
            await _socks_rispondi(scrittore, 0)
            richiesta = await asyncio.wait_for(lettore.readuntil(b"\r\n"), 10)
            percorso = richiesta.split(b" ")[1].decode("ascii", "replace") if b" " in richiesta else "/"
            corpo = ("<!doctype html><title>nyctalus-ok</title><body>nyctalus-ok %s</body>"
                     % re.sub(r"[^A-Za-z0-9/_-]", "", percorso)[:80]).encode()
            scrittore.write(b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nCache-Control: no-store\r\n"
                            b"Content-Length: %d\r\nConnection: close\r\n\r\n" % len(corpo) + corpo)
            await scrittore.drain()
            return scrittore.close()
        if indirizzo_locale(host):
            return await _socks_rispondi(scrittore, 2)        # vietato dalle regole
        if SERVIZI.stato != "pronto":
            return await _socks_rispondi(scrittore, 1)        # rete non pronta: nessuna scorciatoia
        sopra = int(PORTE["socks"] if SERVIZI.modalita == "pubblica" else PORTE["socks_locale"])
        r2, w2 = await asyncio.wait_for(asyncio.open_connection("127.0.0.1", sopra), 10)
        w2.write(b"\x05\x01\x00")
        await w2.drain()
        if await asyncio.wait_for(r2.readexactly(2), 30) != b"\x05\x00":
            w2.close()
            return await _socks_rispondi(scrittore, 1)
        h = host.encode("ascii", "ignore")      # (Chromium manda già i nomi in punycode)
        w2.write(b"\x05\x01\x00\x03" + bytes([len(h)]) + h + porta.to_bytes(2, "big"))
        await w2.drain()
        risposta = await asyncio.wait_for(r2.readexactly(4), 60)
        coda = {1: 6, 4: 18}.get(risposta[3])
        coda = (await r2.readexactly(coda)) if coda else (await r2.readexactly((await r2.readexactly(1))[0] + 2))
        if risposta[1] != 0:
            w2.close()
            return await _socks_rispondi(scrittore, risposta[1])
        await _socks_rispondi(scrittore, 0)
        await asyncio.gather(_incanala(lettore, w2), _incanala(r2, scrittore))
    except (asyncio.IncompleteReadError, asyncio.TimeoutError, ConnectionError):
        try:
            scrittore.close()
        except Exception:        # noqa: BLE001
            pass
    except Exception as e:       # noqa: BLE001
        log("inoltratore del browser:", repr(e))
        try:
            scrittore.close()
        except Exception:        # noqa: BLE001
            pass


async def principale():
    segreto = SEGRETO.crea_se_manca()
    for d in (DATI, INVIO, CACHE):
        d.mkdir(parents=True, exist_ok=True)
    server = await asyncio.start_server(gestisci, "127.0.0.1", PORTA_HTTP)
    try:
        await asyncio.start_server(socks_browser, "127.0.0.1", int(PORTE["socks_browser"]))
    except OSError as e:
        log("inoltratore del browser non avviato (porta %s): %s" % (PORTE["socks_browser"], e))
    log("Nyctalus mobile %s — http://127.0.0.1:%d/?t=%s" % (VERSIONE, PORTA_HTTP, segreto))
    log("web-app da: %s" % WEBAPP)
    if os.environ.get("NYCTALUS_AVVIO_AUTOMATICO", "1") == "1":
        SERVIZI.avvia()
    async with server:
        await server.serve_forever()


def main():
    def esci(*_a):
        SERVIZI.ferma()
        os._exit(0)
    signal.signal(signal.SIGTERM, esci)
    signal.signal(signal.SIGINT, esci)
    try:
        asyncio.run(principale())
    finally:
        SERVIZI.ferma()


if __name__ == "__main__":
    main()
