#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""CONFRONTO DI VELOCITÀ Nyctalus / Tor dentro Shadow, in condizioni realistiche (la prova del
punto K era la più comoda per Tor: niente perdite, nodi scarichi, file piccolo).

Stessa Internet simulata per i due: 30 ms fra due computer qualsiasi, utenti 50/20 Mbit, PERDITA
di pacchetti regolabile, nodi con BANDA regolabile (nodi «carichi» = banda stretta), N utenti che
scaricano INSIEME un file di M MB. Tor: rete di prova di Shadow (1 autorità + 4 relè + 2 uscite,
configurazione di serie, conflux compreso). Nyctalus: 7 nodi (come Tor), 3 salti, K circuiti.

uso: velocita.py CARTELLA --sistema tor|nyctalus [--perdita 0.01] [--banda-nodi 10] [--utenti 4]
                 [--mb 5] [--circuiti 2] [--seme 1]
stampa una riga «RISULTATO …» con tempo medio, velocità media e scaricamenti riusciti."""
import argparse
import os
import random
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

QUI = Path(__file__).resolve().parent
BIN = QUI.parent / "target" / "release" / "nyctalus"
SIM = Path(os.environ.get("NYCTALUS_SIM", Path.home() / ".local/share/nyctalus-sim"))
SHADOW = SIM / "installato" / "bin" / "shadow"
TOR = SIM / "installato" / "bin" / "tor"
MINIMA = SIM / "shadow" / "src" / "test" / "tor" / "minimal"
RELE_TOR = ["4uthority", "relay1", "relay2", "relay3", "relay4", "exit1", "exit2"]
NODI_NYCT = 7
ATTESA = 600            # secondi simulati concessi a ogni scaricamento
lista = lambda *x: "[" + ", ".join('"%s"' % v for v in x) + "]"


def testa(a, stop):
    return ["general:", "  stop_time: %ds" % stop, "  model_unblocked_syscall_latency: true", "  seed: %d" % a.seme,
            "network:", "  graph:", "    type: gml", "    inline: |", "      graph [",
            "        node [", "          id 0", '          host_bandwidth_down "50 Mbit"', '          host_bandwidth_up "20 Mbit"',
            "        ]", "        edge [", "          source 0", "          target 0", '          latency "30 ms"',
            "          packet_loss %.4f" % a.perdita, "        ]", "      ]", "hosts:"]


def banda(a, i=0):
    """Banda del nodo i (0-6): --bande «a,b,…» (nodi di velocità diverse) o --banda-nodi uguale per tutti."""
    if a.bande:
        v = [int(x) for x in a.bande.split(",")]
        b = v[i % len(v)]
    else:
        b = a.banda_nodi
    return ['    bandwidth_down: "%d Mbit"' % b, '    bandwidth_up: "%d Mbit"' % b] if b else []


def sito(out):
    return ["  sito:", "    network_node_id: 0", "    ip_addr: 12.0.0.20", "    processes:",
            "    - path: /usr/bin/python3",
            "      args: %s" % lista("-m", "http.server", "80", "--bind", "12.0.0.20", "--directory", str(out / "sito")),
            "      start_time: 1s", "      expected_final_state: running"]


def scaricamento(u, t, socks):
    return ["    - path: /usr/bin/curl",
            "      args: %s" % lista("-s", "-o", "/dev/null", "--max-time", str(ATTESA - 20),
                                     "-w", "%{http_code} %{size_download} %{time_total} %{time_starttransfer}\\n",
                                     "--socks5-hostname", socks, "http://12.0.0.20/file.bin?u=%d" % u),
            "      start_time: %d ms" % int(t * 1000), "      expected_final_state: { exited: 0 }"]


def prepara_tor(out, a):
    shutil.copytree(MINIMA / "conf", out / "conf")
    modello = out / "modello" / "hosts"
    for h in RELE_TOR + ["torflowauthority"]:
        shutil.copytree(MINIMA / "shadow.data.template" / "hosts" / h, modello / h)
    for u in range(a.utenti):
        shutil.copytree(MINIMA / "shadow.data.template" / "hosts" / "torclient", modello / ("utente%d" % u))
    inizio = 1500
    y = testa(a, inizio + ATTESA)
    for h in RELE_TOR:
        y += ["  %s:" % h, "    network_node_id: 0"] + (["    ip_addr: 100.0.0.1"] if h == "4uthority" else []) + banda(a, RELE_TOR.index(h)) + [
            "    processes:", "    - path: %s" % TOR,
            "      args: %s" % lista("--Address", h, "--Nickname", h, "--defaults-torrc", "torrc-defaults", "-f", "torrc"),
            "      start_time: %ds" % (1 if h == "4uthority" else 60), "      expected_final_state: running"]
    y += sito(out)
    rnd = random.Random(a.seme)
    for u in range(a.utenti):
        y += ["  utente%d:" % u, "    network_node_id: 0", "    ip_addr: 13.0.0.%d" % (10 + u), "    processes:",
              "    - path: %s" % TOR,
              "      args: %s" % lista("--Address", "utente%d" % u, "--Nickname", "utente%d" % u,
                                       "--defaults-torrc", "torrc-defaults", "-f", "torrc"),
              "      start_time: 900s", "      expected_final_state: running"]
        y += scaricamento(u, inizio + rnd.uniform(0, 1), "127.0.0.1:9000")
    (out / "shadow.yaml").write_text("\n".join(y) + "\n")
    return ["--template-directory", str(out / "modello")]


# Identità RIPETIBILI: chiavi dei nodi e dell'autorità si creano una volta per seme e si riusano.
# Dentro Shadow tutto il resto è già deterministico (anche i numeri casuali), quindi stesso seme =
# stessa simulazione: un caso fallito si può rifare identico (anche con le catture dei pacchetti).
CACHE = SIM / "identita"


def lucchetto(a):
    """Un lucchetto per seme: due corse dello stesso seme insieme non creano la stessa identità."""
    import fcntl
    (CACHE / ("seme-%d" % a.seme)).mkdir(parents=True, exist_ok=True)
    f = open(CACHE / ("seme-%d" % a.seme) / ".lucchetto", "w")
    fcntl.flock(f, fcntl.LOCK_EX)
    return f


def identita_nodo(a, d, nome, annuncio):
    """Copia in `d` l'identità del nodo `nome` per questo seme (creandola la prima volta)."""
    with lucchetto(a):
        return _identita_nodo(a, d, nome, annuncio)


def _identita_nodo(a, d, nome, annuncio):
    c = CACHE / ("seme-%d" % a.seme) / nome
    f = c / "riga-elenco.txt"
    if not (f.exists() and f.read_text().startswith(annuncio + ",")):
        shutil.rmtree(c, ignore_errors=True)
        c.mkdir(parents=True)
        p = subprocess.Popen([str(BIN), "node", "--listen", "127.0.0.1:0", "--data", str(c), "--announce", annuncio],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            if f.exists() and f.read_text().strip():
                break
            time.sleep(0.1)
        p.kill()
        p.wait()
    shutil.copytree(c, d)
    return (d / "riga-elenco.txt").read_text().strip()


def autorita(a):
    f = CACHE / ("seme-%d" % a.seme) / "autorita.txt"
    with lucchetto(a):
        if not f.exists() or "public:" not in f.read_text():
            f.write_text(subprocess.run([str(BIN), "authority"], capture_output=True, text=True).stdout)
        return f.read_text()


def prepara_nyctalus(out, a):
    aut = autorita(a)
    sec = re.search(r"secret:\s+([0-9a-f]+)", aut).group(1)
    pub = re.search(r"public:\s+([0-9a-f]+)", aut).group(1)
    righe = []
    for i in range(1, NODI_NYCT + 1):
        riga = identita_nodo(a, out / ("nodo%d" % i), "nodo%d" % i, "11.%d.0.1:4600" % i) + ",gestore%d" % i
        if a.bande and not a.senza_pesi:       # la banda di ogni nodo nell'elenco firmato
            riga += ",%s" % a.bande.split(",")[(i - 1) % len(a.bande.split(","))]
        righe.append(riga)
    # «Aiuta la rete»: ogni utente fa anche da nodo SOLO INTERMEDIO, con un tetto di banda
    for u in range(a.utenti if a.aiuto else 0):
        riga = identita_nodo(a, out / ("aiuto%d" % u), "aiuto%d" % u, "13.0.0.%d:4700" % (10 + u))
        righe.append(riga + ",utente%d,%d,middle" % (u, a.aiuto))
    (out / "directory.txt").write_text(subprocess.run(
        [str(BIN), "directory", "--authority-secret", sec, "--entries", ";".join(righe)],
        capture_output=True, text=True).stdout)
    inizio = 40
    y = testa(a, inizio + ATTESA)
    for i in range(1, NODI_NYCT + 1):
        y += ["  nodo%d:" % i, "    network_node_id: 0", "    ip_addr: 11.%d.0.1" % i] + banda(a, i - 1) + [
              "    processes:", "    - path: %s" % BIN,
              "      args: %s" % lista("node", "--listen", "0.0.0.0:4600", "--data", str(out / ("nodo%d" % i)),
                                       "--announce", "11.%d.0.1:4600" % i),
              '      environment: { NYCTALUS_CONGESTIONE: "%s", NYCTALUS_FINESTRA_KB: "%s", NYCTALUS_RIUSO: "%s" }' % (a.congestione, a.finestra, "0" if a.senza_riuso else "1"),
              "      start_time: 1s", "      expected_final_state: running"]
    y += sito(out)
    rnd = random.Random(a.seme)
    for u in range(a.utenti):
        y += ["  utente%d:" % u, "    network_node_id: 0", "    ip_addr: 13.0.0.%d" % (10 + u), "    processes:",
              "    - path: %s" % BIN,
              "      args: %s" % lista("start", "--directory", str(out / "directory.txt"), "--authority", pub,
                                       "--socks", "127.0.0.1:1080", "--circuits", str(a.circuiti)),
              '      environment: { NYCTALUS_GUARDIE: "%s", NYCTALUS_CONGESTIONE: "%s", NYCTALUS_FINESTRA_KB: "%s", NYCTALUS_CODIFICA: "%s" }' % (out / ("guardie%d" % u), a.congestione, a.finestra, a.codifica),
              "      start_time: 5s", "      expected_final_state: running"]
        if a.aiuto:
            y += ["    - path: %s" % BIN,
                  "      args: %s" % lista("node", "--listen", "0.0.0.0:4700", "--data", str(out / ("aiuto%d" % u)),
                                           "--announce", "13.0.0.%d:4700" % (10 + u), "--middle-only", "--max-rate", str(a.aiuto)),
                  "      start_time: 1s", "      expected_final_state: running"]
        y += scaricamento(u, inizio + rnd.uniform(0, 1), "127.0.0.1:1080")
    testo = "\n".join(y) + "\n"
    if a.diagnosi:      # registri dettagliati del motore (NYCTALUS_DIAGNOSI) su nodi e utenti
        testo = testo.replace("environment: { ", 'environment: { NYCTALUS_DIAGNOSI: "1", ')
    (out / "shadow.yaml").write_text(testo)
    return []


def risultati(out, byte):
    tempi, primi, falliti = [], [], 0
    for f in sorted((out / "shadow.data" / "hosts").glob("utente*/curl.*.stdout")):
        righe = [r.split() for r in f.read_text().splitlines() if r.strip()]
        for p in righe:
            if len(p) == 4 and p[0] == "200" and int(p[1]) == byte:
                tempi.append(float(p[2]))
                primi.append(float(p[3]))
            else:
                falliti += 1
        if not righe:
            falliti += 1
    return tempi, primi, falliti


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cartella")
    ap.add_argument("--sistema", choices=["tor", "nyctalus"], required=True)
    ap.add_argument("--perdita", type=float, default=0.0)
    ap.add_argument("--banda-nodi", type=int, default=0, help="Mbit dei nodi (0 = come gli utenti)")
    ap.add_argument("--senza-pesi", action="store_true", help="Nyctalus: niente banda nell'elenco (scelta a caso)")
    ap.add_argument("--bande", default="", help="Mbit di ciascuno dei 7 nodi, es. 100,50,20,10,5,50,20")
    ap.add_argument("--utenti", type=int, default=1)
    ap.add_argument("--mb", type=float, default=5)
    ap.add_argument("--circuiti", type=int, default=2)
    ap.add_argument("--seme", type=int, default=1)
    ap.add_argument("--diagnosi", action="store_true", help="Nyctalus: registri dettagliati (e la cartella resta)")
    ap.add_argument("--senza-riuso", action="store_true", help="Nyctalus: un collegamento per circuito (come fino alla 0.3.10)")
    ap.add_argument("--aiuto", type=int, default=0, help="Nyctalus: ogni utente fa anche da nodo solo intermedio con questo tetto (Mbit/s)")
    ap.add_argument("--codifica", default="", help="Nyctalus: ragnatela codificata k,r (es. 8,2)")
    ap.add_argument("--finestra", default="", help="Nyctalus: finestra per flusso in KB (NYCTALUS_FINESTRA_KB)")
    ap.add_argument("--congestione", default="", help="Nyctalus: bbr (di serie)|cubic|newreno (NYCTALUS_CONGESTIONE)")
    a = ap.parse_args()
    out = Path(a.cartella).resolve()
    if out.exists():
        shutil.rmtree(out)
    (out / "sito").mkdir(parents=True)
    byte = int(a.mb * 1024 * 1024)
    (out / "sito" / "file.bin").write_bytes(random.Random(a.seme).randbytes(byte))
    extra = prepara_tor(out, a) if a.sistema == "tor" else prepara_nyctalus(out, a)
    subprocess.run([str(SHADOW), "--progress", "false", *extra, "shadow.yaml"], cwd=out,
                   stdout=open(out / "shadow.log", "w"), stderr=subprocess.STDOUT)
    tempi, primi, falliti = risultati(out, byte)
    media = sum(tempi) / len(tempi) if tempi else 0.0
    vel = sum(byte / t for t in tempi) / len(tempi) * 8 / 1e6 if tempi else 0.0
    print("RISULTATO riuso=%s aiuto=%s codifica=%s pesi=%s finestra=%s congestione=%s sistema=%s perdita=%g banda_nodi=%s utenti=%d mb=%g circuiti=%s seme=%d riusciti=%d falliti=%d "
          "tempo_medio=%.2f primo_byte=%.2f mbit_s=%.2f"
          % ("no" if a.senza_riuso else "si", a.aiuto or "no", a.codifica.replace(",", "+") or "no", "no" if a.senza_pesi or not a.bande else "si", a.finestra or "-", a.congestione or "bbr", a.sistema, a.perdita, (a.bande.replace(",", "/") if a.bande else (a.banda_nodi or "-")), a.utenti, a.mb, a.circuiti if a.sistema == "nyctalus" else "-",
             a.seme, len(tempi), falliti, media, sum(primi) / len(primi) if primi else 0.0, vel))
    # i file scaricati non servono più: si tiene solo il registro (il disco è quasi pieno)
    shutil.rmtree(out / "sito", ignore_errors=True)
    if falliti and a.diagnosi:
        print("FALLITA: registri in", out)


if __name__ == "__main__":
    sys.exit(main())
