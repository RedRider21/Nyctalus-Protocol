#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""PIANO-ANONIMATO, punto K: CHI SCRIVE A CHI? Chat normale contro modalità riservata, con Nyctalus
vero dentro Shadow e un avversario che vede le linee di TUTTI gli utenti.

Rete: 4 nodi di gestori diversi, rubrica + casella, 6 utenti che chattano a coppie (0↔1, 2↔3, 4↔5)
con messaggi in momenti casuali. L'avversario, per ogni utente A, confronta i byte che A manda con i
byte che ogni altro utente B riceve poco dopo (ritardi da 0 a 60 s) e indica come compagno di A
quello che corrisponde di più. Misura: quanti compagni indovina (a caso: 1 su 5, il 20%).

uso: riservata.py CARTELLA (--normale | --riservata) [--messaggi 6] [--seme 1]
"""
import argparse
import os
import random
import re
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from correlazione import leggi_pcap, pearson  # noqa: E402

QUI = Path(__file__).resolve().parent
ROOT = QUI.parent
BIN = ROOT / "target" / "release" / "nyctalus"
RUBRICA_PY = ROOT / "pc" / "gestore-rete" / "rubrica.py"
SHADOW = Path(os.environ.get("SHADOW", Path.home() / ".local/share/nyctalus-sim/installato/bin/shadow"))
UTENTI = 6
SEGRETO = "segretosimulazione0123456789ab"
INIZIO, FINE = 150, 900             # i messaggi partono fra questi secondi della simulazione


def indirizzo(file):
    testo = subprocess.run([str(BIN), "address", "--identity", str(file)], capture_output=True, text=True).stdout
    return re.findall(r"[a-z2-7]{56}\.nyct", testo)[-1]


def prepara(out, a):
    if out.exists():
        shutil.rmtree(out)
    out.mkdir(parents=True)
    aut = subprocess.run([str(BIN), "authority"], capture_output=True, text=True).stdout
    sec = re.search(r"secret:\s+([0-9a-f]+)", aut).group(1)
    pub = re.search(r"public:\s+([0-9a-f]+)", aut).group(1)
    righe = []
    for i in range(1, 5):
        d = out / ("nodo%d" % i)
        d.mkdir()
        p = subprocess.Popen([str(BIN), "node", "--listen", "127.0.0.1:0", "--data", str(d),
                              "--announce", "11.%d.0.1:4600" % i], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            if (d / "riga-elenco.txt").exists() and (d / "riga-elenco.txt").read_text().strip():
                break
            __import__("time").sleep(0.1)
        p.kill()
        righe.append((d / "riga-elenco.txt").read_text().strip() + ",gestore%d" % i)
    elenco = out / "directory.txt"
    elenco.write_text(subprocess.run([str(BIN), "directory", "--authority-secret", sec, "--entries", ";".join(righe)],
                                     capture_output=True, text=True).stdout)
    rd = out / "rubrica"
    rd.mkdir()
    rub, cas = indirizzo(rd / "identita"), indirizzo(rd / "casella-identita")
    ind = []
    for u in range(UTENTI):
        (out / ("utente%d" % u)).mkdir()
        ind.append(indirizzo(out / ("utente%d" % u) / "identita"))

    rnd = random.Random(a.seme)
    lista = lambda *x: "[" + ", ".join('"%s"' % v for v in x) + "]"
    proc = lambda path, args, t, env=None, finale="running": (
        ["    - path: %s" % path, "      args: %s" % lista(*args)]
        + (["      environment: { %s }" % ", ".join('%s: "%s"' % kv for kv in env.items())] if env else [])
        + ["      start_time: %ds" % t, "      expected_final_state: %s" % finale])
    rete = ["--directory", str(elenco), "--authority", pub]
    y = ["general:", "  stop_time: %ds" % (FINE + 120), "  model_unblocked_syscall_latency: true",
         "network:", "  graph:", "    type: gml", "    inline: |", "      graph [",
         "        node [", "          id 0", '          host_bandwidth_down "50 Mbit"', '          host_bandwidth_up "20 Mbit"',
         "        ]", "        edge [", "          source 0", "          target 0", '          latency "30 ms"',
         "          packet_loss 0.0", "        ]", "      ]", "hosts:"]
    for i in range(1, 5):
        y += ["  nodo%d:" % i, "    network_node_id: 0", "    ip_addr: 11.%d.0.1" % i, "    processes:"]
        y += proc(BIN, ["node", "--listen", "0.0.0.0:4600", "--data", str(out / ("nodo%d" % i)), "--announce", "11.%d.0.1:4600" % i], 1)
    tok = {"NYCTALUS_CHAT_TOKEN": SEGRETO}
    # Shadow 3.3: una riga molto lunga (lotto di 4 buste, ~67 KB) sul collegamento LOCALE fra
    # rubrica.py e il suo motore blocca quel collegamento simulato (sul PC vero no: prova 26).
    # In simulazione la rubrica consegna una busta per messaggio; le buste sono le stesse.
    rub_env = dict(tok, NYCTALUS_LOTTO_POSTA="9000")
    if os.environ.get("DIAGNOSI"):
        tok["NYCTALUS_DIAGNOSI"] = "1"
    y += ["  rubrica:", "    network_node_id: 0", "    ip_addr: 12.0.0.5", "    processes:"]
    y += proc(BIN, ["start", *rete, "--socks", "127.0.0.1:1080"], 5, {"NYCTALUS_GUARDIE": str(rd / "guardie")})
    y += proc(BIN, ["chat", "--data", str(rd), "--backend", "127.0.0.1:1082", "--control", "127.0.0.1:1091",
                    "--socks", "127.0.0.1:1080", "--max-file", "1"], 5, tok)   # (tok porta anche la diagnosi)
    y += proc(BIN, ["site", "--backend", "127.0.0.1:1082", "--identity", str(rd / "identita"), *rete, "--hops", "3"], 10,
              dict({"NYCTALUS_GUARDIE": str(rd / "guardie")}, **({"NYCTALUS_DIAGNOSI": "1"} if os.environ.get("DIAGNOSI") else {})))
    y += proc("/usr/bin/python3", [str(QUI / "rubrica_diagnosi.py") if os.environ.get("DIAGNOSI") else str(RUBRICA_PY), str(rd), "1091", "1095"], 10, rub_env)
    y += proc(BIN, ["site", "--backend", "127.0.0.1:1095", "--identity", str(rd / "casella-identita"), *rete, "--hops", "3"], 10,
              {"NYCTALUS_GUARDIE": str(rd / "guardie")})
    for u in range(UTENTI):
        d = out / ("utente%d" % u)
        compagno = u ^ 1
        orari = sorted(rnd.uniform(INIZIO, FINE) for _ in range(a.messaggi))
        g = {"NYCTALUS_GUARDIE": str(d / "guardie")}
        y += ["  utente%d:" % u, "    network_node_id: 0", "    ip_addr: 13.0.0.%d" % (10 + u),
              "    host_options:", "      pcap_enabled: true", "      pcap_capture_size: 64 B", "    processes:"]
        y += proc(BIN, ["start", *rete, "--socks", "127.0.0.1:1080"], 5, g)
        y += proc(BIN, ["chat", "--data", str(d), "--backend", "127.0.0.1:1082", "--control", "127.0.0.1:1091",
                        "--socks", "127.0.0.1:1080", "--rubrica", rub, "--casella", cas], 5, dict(tok, **g))
        y += proc(BIN, ["site", "--backend", "127.0.0.1:1082", "--identity", str(d / "identita"), *rete, "--hops", "3"], 10, g)
        y += proc("/usr/bin/python3", [str(QUI / "pilota_chat.py"), "1091", SEGRETO, "1" if a.riservata else "0",
                                       ind[compagno], ",".join("%.1f" % t for t in orari), "60"], 60, None, "{ exited: 0 }")
    (out / "shadow.yaml").write_text("\n".join(y) + "\n")


def serie(pacchetti, t0, t1, casella):
    v = [0.0] * (int((t1 - t0) / casella) + 1)
    for t, lung in pacchetti:
        if t0 <= t <= t1:
            v[int((t - t0) / casella)] += lung
    return v


# due attacchi, e per ogni modalità vale il MIGLIORE per l'avversario:
#  - «veloce»: caselle da 0,1 s, ritardi 0-2 s (chat diretta: il messaggio arriva subito)
#  - «lento»: caselle da 1 s, ritardi 0-60 s (casella: il messaggio arriva al ritiro)
ATTACCHI = (("veloce", 0.1, 20), ("lento", 1.0, 60))


def analizza(out):
    h = out / "shadow.data" / "hosts"
    su, giu = {}, {}
    t0 = None
    for u in range(UTENTI):
        pk = leggi_pcap(next((h / ("utente%d" % u)).glob("eth0*.pcap")))
        mio = "13.0.0.%d" % (10 + u)
        su[u] = [(p[0], p[6]) for p in pk if p[3] == 17 and p[1] == mio]
        giu[u] = [(p[0], p[6]) for p in pk if p[3] == 17 and p[2] == mio]
        if pk:
            t0 = pk[0][0] if t0 is None else min(t0, pk[0][0])
    a0, a1 = t0 + INIZIO - 20, t0 + FINE + 60
    risultati = {}
    for nome, cas, ritardi in ATTACCHI:
        S = {u: serie(su[u], a0, a1, cas) for u in su}
        G = {u: serie(giu[u], a0, a1, cas) for u in giu}
        punteggio = {}
        for x in range(UTENTI):
            for y in range(UTENTI):
                if x != y:
                    punteggio[(x, y)] = max(pearson(S[x], G[y][lag:] + [0.0] * lag) for lag in range(ritardi + 1))
        giusti = 0
        for x in range(UTENTI):
            # la coppia è simmetrica: A→B e B→A
            scelto = max((y for y in range(UTENTI) if y != x), key=lambda y: punteggio[(x, y)] + punteggio[(y, x)])
            giusti += scelto == (x ^ 1)
        risultati[nome] = giusti
    # attacco A EVENTI (il più forte qui): gli «eventi» sono le caselle con un traffico della misura
    # di un messaggio; per A→B conta quante volte, dopo un invio di A, B riceve entro la finestra,
    # meno quante volte capiterebbe per caso
    for nome, cas, finestra in (("eventi-veloce", 0.5, 2.0), ("eventi-lento", 1.0, 60.0)):
        ev_su, ev_giu = {}, {}
        for u in range(UTENTI):
            S = serie(su[u], a0, a1, cas)
            G = serie(giu[u], a0, a1, cas)
            ev_su[u] = [a0 + i * cas for i, b in enumerate(S) if 1500 <= b <= 9000]
            ev_giu[u] = [a0 + i * cas for i, b in enumerate(G) if 1500 <= b <= 9000]
        durata = a1 - a0
        punteggio = {}
        for x in range(UTENTI):
            for y in range(UTENTI):
                if x == y or not ev_su[x]:
                    punteggio[(x, y)] = 0.0
                    continue
                presi = sum(any(t < r <= t + finestra for r in ev_giu[y]) for t in ev_su[x]) / len(ev_su[x])
                caso = min(1.0, len(ev_giu[y]) * finestra / durata)
                punteggio[(x, y)] = presi - caso
        giusti = 0
        for x in range(UTENTI):
            scelto = max((y for y in range(UTENTI) if y != x), key=lambda y: punteggio[(x, y)] + punteggio[(y, x)])
            giusti += scelto == (x ^ 1)
        risultati[nome] = giusti
    return risultati


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cartella")
    modo = ap.add_mutually_exclusive_group(required=True)
    modo.add_argument("--normale", action="store_true")
    modo.add_argument("--riservata", action="store_true")
    ap.add_argument("--messaggi", type=int, default=6)
    ap.add_argument("--seme", type=int, default=1)
    a = ap.parse_args()
    out = Path(a.cartella).resolve()
    prepara(out, a)
    r = subprocess.run([str(SHADOW), "--progress", "false", "shadow.yaml"], cwd=out,
                       stdout=open(out / "shadow.log", "w"), stderr=subprocess.STDOUT)
    if r.returncode != 0:
        print("shadow: uscito con errore (vedi %s)" % (out / "shadow.log"))
    risultati = analizza(out)
    giusti = max(risultati.values())
    inviati = sum(len(re.findall(r"^inviato", f.read_text(), re.M)) for f in (out / "shadow.data" / "hosts").glob("utente*/python3.*.stdout"))
    ricevuti = sum(len(re.findall(r"^in ", f.read_text(), re.M)) for f in out.glob("utente*/cronologia/*.log"))
    print("%s · %d utenti, %d messaggi inviati, %d arrivati → l'avversario indovina il compagno di %d utenti su %d (%.0f%%; a caso 20%%) %s"
          % ("RISERVATA" if a.riservata else "normale", UTENTI, inviati, ricevuti, giusti, UTENTI, 100.0 * giusti / UTENTI,
             risultati))


if __name__ == "__main__":
    sys.exit(main())
