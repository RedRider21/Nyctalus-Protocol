#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""PIANO-ANONIMATO, punto K: lo STESSO attacco di correlazione di `correlazione.py`, ma con TOR
(0.4.9) dentro Shadow, per confrontare alla pari: stessa Internet simulata (30 ms, 50/20 Mbit),
stesso sito, stessi utenti e scaricamenti sovrapposti, stessa analisi.

La rete Tor è quella di prova di Shadow (src/test/tor/minimal: 1 autorità, 4 relè, 2 uscite, chiavi
già pronte). I client Tor partono a 15 minuti simulati (Tor deve costruire il suo consenso) e gli
scaricamenti a 25 minuti.

uso: tor_correlazione.py CARTELLA [--utenti 6] [--giri 3] [--seme 1]"""
import argparse
import os
import random
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from correlazione import analizza, tempi  # noqa: E402

SIM = Path(os.environ.get("NYCTALUS_SIM", Path.home() / ".local/share/nyctalus-sim"))
SHADOW = SIM / "installato" / "bin" / "shadow"
TOR = SIM / "installato" / "bin" / "tor"
MINIMA = SIM / "shadow" / "src" / "test" / "tor" / "minimal"
RELE = ["4uthority", "relay1", "relay2", "relay3", "relay4", "exit1", "exit2"]
INIZIO = 1500


def prepara(out, a):
    if out.exists():
        shutil.rmtree(out)
    (out / "sito").mkdir(parents=True)
    (out / "sito" / "file.bin").write_bytes(os.urandom(500 * 1024))
    shutil.copytree(MINIMA / "conf", out / "conf")
    modello = out / "modello" / "hosts"
    for h in RELE + ["torflowauthority"]:
        shutil.copytree(MINIMA / "shadow.data.template" / "hosts" / h, modello / h)
    for u in range(a.utenti):
        shutil.copytree(MINIMA / "shadow.data.template" / "hosts" / "torclient", modello / ("utente%d" % u))
    rnd = random.Random(a.seme)
    lista = lambda *x: "[" + ", ".join('"%s"' % v for v in x) + "]"
    y = ["general:", "  stop_time: %ds" % (INIZIO + 25 * a.giri + 60), "  model_unblocked_syscall_latency: true",
         "network:", "  graph:", "    type: gml", "    inline: |", "      graph [",
         "        node [", "          id 0", '          host_bandwidth_down "50 Mbit"', '          host_bandwidth_up "20 Mbit"',
         "        ]", "        edge [", "          source 0", "          target 0", '          latency "30 ms"',
         "          packet_loss 0.0", "        ]", "      ]", "hosts:"]
    for h in RELE:
        y += ["  %s:" % h, "    network_node_id: 0"] + (["    ip_addr: 100.0.0.1"] if h == "4uthority" else []) + [
            "    processes:", "    - path: %s" % TOR,
            "      args: %s" % lista("--Address", h, "--Nickname", h, "--defaults-torrc", "torrc-defaults", "-f", "torrc"),
            "      start_time: %ds" % (1 if h == "4uthority" else 60), "      expected_final_state: running"]
    y += ["  sito:", "    network_node_id: 0", "    ip_addr: 12.0.0.20",
          "    host_options:", "      pcap_enabled: true", "      pcap_capture_size: 400 B", "    processes:",
          "    - path: /usr/bin/python3",
          "      args: %s" % lista("-m", "http.server", "80", "--bind", "12.0.0.20", "--directory", str(out / "sito")),
          "      start_time: 1s", "      expected_final_state: running"]
    for u in range(a.utenti):
        y += ["  utente%d:" % u, "    network_node_id: 0", "    ip_addr: 13.0.0.%d" % (10 + u),
              "    host_options:", "      pcap_enabled: true", "      pcap_capture_size: 64 B", "    processes:",
              "    - path: %s" % TOR,
              "      args: %s" % lista("--Address", "utente%d" % u, "--Nickname", "utente%d" % u,
                                       "--defaults-torrc", "torrc-defaults", "-f", "torrc"),
              "      start_time: 900s", "      expected_final_state: running"]
        for g in range(a.giri):
            t = INIZIO + 25 * g + rnd.uniform(0, 8)
            y += ["    - path: /usr/bin/curl",
                  "      args: %s" % lista("-s", "-o", "/dev/null", "-w", "%{time_total} %{time_starttransfer}\\n", "--socks5-hostname", "127.0.0.1:9000",
                                           "http://12.0.0.20/file.bin?u=%d&g=%d" % (u, g)),
                  "      start_time: %d ms" % int(t * 1000), "      expected_final_state: { exited: 0 }"]
    (out / "shadow.yaml").write_text("\n".join(y) + "\n")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cartella")
    ap.add_argument("--utenti", type=int, default=6)
    ap.add_argument("--giri", type=int, default=3)
    ap.add_argument("--seme", type=int, default=1)
    a = ap.parse_args()
    out = Path(a.cartella).resolve()
    prepara(out, a)
    r = subprocess.run([str(SHADOW), "--progress", "false", "--template-directory", str(out / "modello"), "shadow.yaml"],
                       cwd=out, stdout=open(out / "shadow.log", "w"), stderr=subprocess.STDOUT)
    if r.returncode != 0:
        print("shadow: uscito con errore (vedi %s)" % (out / "shadow.log"))
    giusti, tot = analizza(out, a, proto=6, ip_utente=lambda u: "13.0.0.%d" % (10 + u))
    tt, tp = tempi(out)
    print("TOR · utenti %d · scaricamenti %d · ritardo 30 ms → l'avversario ne attribuisce giusti %d su %d (%.0f%%; a caso %.0f%%) · 500 KB in %.2f s, primo byte %.2f s"
          % (a.utenti, tot, giusti, tot, 100.0 * giusti / max(tot, 1), 100.0 / a.utenti, tt, tp))


if __name__ == "__main__":
    sys.exit(main())
