#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""PIANO-ANONIMATO, punto K: ATTACCO DI CORRELAZIONE DEI TEMPI, con Nyctalus vero dentro Shadow.

N utenti scaricano ciascuno alcune volte un file in momenti casuali e sovrapposti, attraverso 4 nodi
di gestori diversi. Shadow registra i pacchetti sulle linee degli utenti e del sito. L'«avversario
globale» vede entrambe: per ogni scaricamento visto dal sito confronta l'andamento nel tempo dei byte
con quello di ogni utente e sceglie il più simile. Misura: quanti scaricamenti attribuisce giusti
(il caso puro è 1/N). Si può accendere il rumore di fondo (--rumore HZ, su utenti e nodi).

uso: correlazione.py CARTELLA [--utenti 6] [--giri 3] [--rumore 0] [--ritardo 30] [--seme 1]
"""
import argparse
import os
import random
import re
import shutil
import struct
import subprocess
import sys
from pathlib import Path

QUI = Path(__file__).resolve().parent
ROOT = QUI.parent
BIN = ROOT / "target" / "release" / "nyctalus"
SHADOW = Path(os.environ.get("SHADOW", Path.home() / ".local/share/nyctalus-sim/installato/bin/shadow"))
CASELLA = 0.05          # secondi per casella delle serie temporali
FILE_KB = 500


def prepara(out, a):
    if out.exists():
        shutil.rmtree(out)
    (out / "sito").mkdir(parents=True)
    (out / "sito" / "file.bin").write_bytes(os.urandom(FILE_KB * 1024))
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
    (out / "directory.txt").write_text(subprocess.run(
        [str(BIN), "directory", "--authority-secret", sec, "--entries", ";".join(righe)],
        capture_output=True, text=True).stdout)

    rnd = random.Random(a.seme)
    lista = lambda *x: "[" + ", ".join('"%s"' % v for v in x) + "]"
    rumore = ["--cover", str(a.rumore)] if a.rumore > 0 else []
    y = ["general:", "  stop_time: %ds" % (60 + 25 * a.giri + 30), "  model_unblocked_syscall_latency: true",
         "network:", "  graph:", "    type: gml", "    inline: |", "      graph [",
         "        node [", "          id 0", '          host_bandwidth_down "50 Mbit"', '          host_bandwidth_up "20 Mbit"',
         "        ]", "        edge [", "          source 0", "          target 0", '          latency "%d ms"' % a.ritardo,
         "          packet_loss 0.0", "        ]", "      ]",
         "host_option_defaults:", "  pcap_enabled: false", "hosts:"]
    for i in range(1, 5):
        y += ["  nodo%d:" % i, "    network_node_id: 0", "    ip_addr: 11.%d.0.1" % i, "    processes:",
              "    - path: %s" % BIN,
              "      args: %s" % lista("node", "--listen", "0.0.0.0:4600", "--data", str(out / ("nodo%d" % i)),
                                       "--announce", "11.%d.0.1:4600" % i, *rumore),
              "      start_time: 1s", "      expected_final_state: running"]
    y += ["  sito:", "    network_node_id: 0", "    ip_addr: 12.0.0.20",
          "    host_options:", "      pcap_enabled: true", "      pcap_capture_size: 400 B", "    processes:",
          "    - path: /usr/bin/python3",
          "      args: %s" % lista("-m", "http.server", "80", "--bind", "12.0.0.20", "--directory", str(out / "sito")),
          "      start_time: 1s", "      expected_final_state: running"]
    verita = []
    for u in range(a.utenti):
        y += ["  utente%d:" % u, "    network_node_id: 0", "    ip_addr: 13.0.%d.%d" % (u // 200, u % 200 + 10),
              "    host_options:", "      pcap_enabled: true", "      pcap_capture_size: 64 B", "    processes:",
              "    - path: %s" % BIN,
              "      args: %s" % lista("start", "--directory", str(out / "directory.txt"), "--authority",
                                       pub, "--socks", "127.0.0.1:1080", "--circuits", "2", *rumore),
              '      environment: { NYCTALUS_GUARDIE: "%s" }' % (out / ("guardie%d" % u)),
              "      start_time: 5s", "      expected_final_state: running"]
        for g in range(a.giri):
            t = 40 + 25 * g + rnd.uniform(0, 8)          # scaricamenti sovrapposti fra utenti
            verita.append((u, g, t))
            y += ["    - path: /usr/bin/curl",
                  "      args: %s" % lista("-s", "-o", "/dev/null", "-w", "%{time_total} %{time_starttransfer}\\n", "--socks5-hostname", "127.0.0.1:1080",
                                           "http://12.0.0.20/file.bin?u=%d&g=%d" % (u, g)),
                  "      start_time: %d ms" % int(t * 1000), "      expected_final_state: { exited: 0 }"]
    (out / "shadow.yaml").write_text("\n".join(y) + "\n")
    return verita


def leggi_pcap(p):
    """[(tempo, ip_sorgente, ip_destinazione, proto, porta_s, porta_d, lunghezza_ip, dati)]"""
    b = p.read_bytes()
    if len(b) < 24:
        return []
    fine = "<" if b[:4] in (b"\xd4\xc3\xb2\xa1", b"\x4d\x3c\xb2\xa1") else ">"
    nano = b[:4] in (b"\x4d\x3c\xb2\xa1", b"\xa1\xb2\x3c\x4d")
    tipo = struct.unpack(fine + "I", b[20:24])[0]
    off, out = 24, []
    while off + 16 <= len(b):
        s, us, incl, _orig = struct.unpack(fine + "IIII", b[off:off + 16])
        d = b[off + 16:off + 16 + incl]
        off += 16 + incl
        ip = d[14:] if tipo == 1 else d
        if len(ip) < 20 or ip[0] >> 4 != 4:
            continue
        hl = (ip[0] & 15) * 4
        lung = struct.unpack("!H", ip[2:4])[0]
        proto = ip[9]
        src, dst = ".".join(map(str, ip[12:16])), ".".join(map(str, ip[16:20]))
        ps = pd = 0
        dati = b""
        if proto in (6, 17) and len(ip) >= hl + 4:
            ps, pd = struct.unpack("!HH", ip[hl:hl + 4])
            if proto == 6 and len(ip) >= hl + 13:
                dati = ip[hl + (ip[hl + 12] >> 4) * 4:]
        out.append((s + us / (1e9 if nano else 1e6), src, dst, proto, ps, pd, lung, dati))
    return out


def tempi(out):
    """(tempo medio di uno scaricamento, tempo medio del primo byte) dai curl degli utenti."""
    v = []
    for f in (out / "shadow.data" / "hosts").glob("utente*/curl.*.stdout"):
        for r in f.read_text().split("\n"):
            p = r.split()
            if len(p) == 2:
                v.append((float(p[0]), float(p[1])))
    if not v:
        return 0.0, 0.0
    return sum(x for x, _ in v) / len(v), sum(y for _, y in v) / len(v)


def serie(pacchetti, t0, t1):
    n = int((t1 - t0) / CASELLA) + 1
    v = [0.0] * n
    for t, lung in pacchetti:
        if t0 <= t <= t1:
            v[int((t - t0) / CASELLA)] += lung
    return v


def pearson(a, b):
    n = len(a)
    if n < 3:
        return 0.0
    ma, mb = sum(a) / n, sum(b) / n
    sa = sum((x - ma) ** 2 for x in a) ** .5
    sb = sum((y - mb) ** 2 for y in b) ** .5
    if sa == 0 or sb == 0:
        return 0.0
    return sum((x - ma) * (y - mb) for x, y in zip(a, b)) / (sa * sb)


def analizza(out, a, proto=17, ip_utente=None):
    """proto: 17 = UDP (Nyctalus/QUIC), 6 = TCP (Tor). ip_utente(u) → indirizzo dell'utente u."""
    ip_utente = ip_utente or (lambda u: "13.0.%d.%d" % (u // 200, u % 200 + 10))
    h = out / "shadow.data" / "hosts"
    sito = leggi_pcap(next((h / "sito").glob("eth0*.pcap")))
    # scaricamenti visti dal sito: connessioni TCP, con la richiesta (u=…&g=…) come verità
    flussi = {}
    for t, src, dst, pr_sito, ps, pd, lung, dati in sito:
        if pr_sito != 6:
            continue
        chiave = (dst, pd) if src == "12.0.0.20" else (src, ps)
        f = flussi.setdefault(chiave, {"giu": [], "u": None})
        m = re.search(rb"GET /file\.bin\?u=(\d+)&g=(\d+)", dati or b"")
        if m:
            f["u"] = int(m.group(1))
        if src == "12.0.0.20":
            f["giu"].append((t, lung))
    utenti = {}
    for u in range(a.utenti):
        pk = leggi_pcap(next((h / ("utente%d" % u)).glob("eth0*.pcap")))
        mio = ip_utente(u)
        utenti[u] = [(t, lung) for t, src, dst, pr, ps, pd, lung, _ in pk if pr == proto and dst == mio]
    giusti = tot = 0
    for f in flussi.values():
        if f["u"] is None or len(f["giu"]) < 5:
            continue
        t0, t1 = f["giu"][0][0] - 0.5, f["giu"][-1][0] + 1.5
        s = serie(f["giu"], t0, t1)
        punteggi = {}
        for u, pk in utenti.items():
            migliore = -1.0
            for spost in range(0, int(1.0 / CASELLA) + 1):          # il ritardo della rete: 0-1 s
                su = serie(pk, t0 + spost * CASELLA, t1 + spost * CASELLA)
                migliore = max(migliore, pearson(s, su[:len(s)]))
            punteggi[u] = migliore
        scelto = max(punteggi, key=punteggi.get)
        tot += 1
        giusti += scelto == f["u"]
    return giusti, tot


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cartella")
    ap.add_argument("--utenti", type=int, default=6)
    ap.add_argument("--giri", type=int, default=3)
    ap.add_argument("--rumore", type=float, default=0.0)
    ap.add_argument("--ritardo", type=int, default=30)
    ap.add_argument("--seme", type=int, default=1)
    a = ap.parse_args()
    out = Path(a.cartella).resolve()
    prepara(out, a)
    r = subprocess.run([str(SHADOW), "--progress", "false", "shadow.yaml"], cwd=out,
                       stdout=open(out / "shadow.log", "w"), stderr=subprocess.STDOUT)
    if r.returncode != 0:
        print("shadow: errore (vedi %s)" % (out / "shadow.log"))
    giusti, tot = analizza(out, a)
    tt, tp = tempi(out)
    caso = 100.0 / a.utenti
    print("utenti %d · scaricamenti %d · rumore %s/s · ritardo %d ms → l'avversario ne attribuisce giusti %d su %d (%.0f%%; a caso %.0f%%) · 500 KB in %.2f s, primo byte %.2f s"
          % (a.utenti, tot, a.rumore, a.ritardo, giusti, tot, 100.0 * giusti / max(tot, 1), caso, tt, tp))


if __name__ == "__main__":
    sys.exit(main())
