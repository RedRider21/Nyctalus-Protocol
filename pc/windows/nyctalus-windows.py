# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Avvio di Nyctalus su Windows (doppio clic su Nyctalus.cmd).

1. accende il servizio della chat (lo stesso del telefono: orchestratore + motore nyctalus.exe),
   senza finestre di console, se non è già acceso;
2. apre la finestra d'avvio (come la schermata dell'app del telefono) con Edge in «modalità
   app», cioè senza barra degli indirizzi; se Edge non c'è, nel browser predefinito.

Solo libreria standard di Python: gira con il Python «embeddable» incluso nel pacchetto.
"""
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request
import webbrowser
from pathlib import Path

QUI = Path(__file__).resolve().parent.parent          # la cartella scompattata del pacchetto
DATI = Path.home() / ".nyctalus-mobile"
SENZA_FINESTRA = getattr(subprocess, "CREATE_NO_WINDOW", 0) | getattr(subprocess, "DETACHED_PROCESS", 0)


def porta():
    try:
        return int(json.loads((DATI / "config.json").read_text()).get("porte", {}).get("http", 18731))
    except Exception:    # noqa: BLE001
        return 18731


def acceso(url):
    """Il servizio risponde (anche con 403: è acceso, manca solo il segreto)."""
    try:
        urllib.request.urlopen(url + "/api/stato", timeout=1)
        return True
    except urllib.error.HTTPError:
        return True
    except Exception:    # noqa: BLE001
        return False


def accendi():
    env = dict(os.environ, NYCTALUS_WEBAPP=str(QUI / "webapp"))
    env.setdefault("NYCTALUS_BIN", str(QUI / "bin" / "nyctalus.exe"))   # (le prove ne usano un altro)
    pythonw = Path(sys.executable).with_name("pythonw.exe")
    interprete = str(pythonw if pythonw.exists() else sys.executable)
    DATI.mkdir(parents=True, exist_ok=True)
    log = open(DATI / "nyctalus-windows.log", "ab")
    subprocess.Popen([interprete, str(QUI / "orchestratore" / "nyctalus_mobile.py")], env=env, cwd=str(QUI),
                     stdout=log, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, creationflags=SENZA_FINESTRA)


def edge():
    for base in (os.environ.get("ProgramFiles(x86)"), os.environ.get("ProgramFiles"), os.environ.get("LOCALAPPDATA")):
        if base:
            p = Path(base) / "Microsoft" / "Edge" / "Application" / "msedge.exe"
            if p.exists():
                return str(p)
    return None


def main():
    url = "http://127.0.0.1:%d" % porta()
    if not acceso(url):
        accendi()
        for _ in range(100):                                  # al massimo 20 secondi
            if acceso(url) and (DATI / "token").exists():
                break
            time.sleep(0.2)
    try:
        segreto = (DATI / "token").read_text(encoding="utf-8").strip()
    except OSError:
        segreto = ""
    pagina = "%s/avvio.html?t=%s" % (url, segreto)
    if os.environ.get("NYCTALUS_NON_APRIRE"):                 # prove: niente finestre
        return print(pagina)
    e = edge()
    if e:
        subprocess.Popen([e, "--app=" + pagina, "--window-size=440,700"], creationflags=SENZA_FINESTRA)
    else:
        webbrowser.open(pagina)


if __name__ == "__main__":
    main()
