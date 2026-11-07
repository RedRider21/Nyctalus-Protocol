#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""VERIFICA DELLE SCELTE DI FONDO sul traffico vero (Nyctalus dentro Shadow, pacchetti catturati
su TUTTE le macchine). Si controlla quello che il progetto promette:

  1. trasporto: fra utente e rete solo UDP (QUIC), mai TCP;
  2. QUIC riconoscibile senza camuffamento, irriconoscibile con il camuffamento; e cosa legge un
     osservatore nel primo pacchetto (le cui chiavi sono pubbliche per standard: RFC 9001 §5.2);
  3. l'utente parla solo con la sua entrata;
  4. ragnatela: i nodi intermedi di circuiti diversi portano insieme lo stesso scaricamento;
  5. cipolla: il contenuto non compare mai in chiaro prima dell'uscita;
  6. rumore di fondo: traffico anche quando l'utente è fermo;
  7. forma dei pacchetti fra utente ed entrata.

uso: verifica_paradigmi.py CARTELLA [--camuffamento | --http3 NOME] [--rumore 0.2] [--porta 4600]
"""
import argparse
import collections
import hashlib
import hmac
import os
import random
import re
import shutil
import struct
import subprocess
import sys
from pathlib import Path

QUI = Path(__file__).resolve().parent
BIN = QUI.parent / "target" / "release" / "nyctalus"
SHADOW = Path(os.environ.get("SHADOW", Path.home() / ".local/share/nyctalus-sim/installato/bin/shadow"))
FILE_KB = 500
T_SCARICA = 40          # secondi simulati: lo scaricamento parte qui
FERMO = (15, 38)        # finestra in cui l'utente è fermo (per il rumore di fondo)
UTENTE, SITO = "13.0.0.10", "12.0.0.20"
NODI = ["11.%d.0.1" % i for i in range(1, 5)]


def prepara(out, a):
    if out.exists():
        shutil.rmtree(out)
    (out / "sito").mkdir(parents=True)
    contenuto = os.urandom(FILE_KB * 1024)
    (out / "sito" / "file.bin").write_bytes(contenuto)
    aut = subprocess.run([str(BIN), "authority"], capture_output=True, text=True).stdout
    sec = re.search(r"secret:\s+([0-9a-f]+)", aut).group(1)
    pub = re.search(r"public:\s+([0-9a-f]+)", aut).group(1)
    righe = []
    for i in range(1, 5):
        d = out / ("nodo%d" % i)
        d.mkdir()
        p = subprocess.Popen([str(BIN), "node", "--listen", "127.0.0.1:0", "--data", str(d),
                              "--announce", "%s:%d" % (NODI[i - 1], a.porta)] + (["--mimic", "--mimic-name", a.http3] if a.http3 else []), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(50):
            if (d / "riga-elenco.txt").exists() and (d / "riga-elenco.txt").read_text().strip():
                break
            __import__("time").sleep(0.1)
        p.kill()
        righe.append((d / "riga-elenco.txt").read_text().strip() + ",gestore%d" % i)
    (out / "directory.txt").write_text(subprocess.run(
        [str(BIN), "directory", "--authority-secret", sec, "--entries", ";".join(righe)],
        capture_output=True, text=True).stdout)
    lista = lambda *x: "[" + ", ".join('"%s"' % v for v in x) + "]"
    prot = []
    if a.http3:
        prot += ["--obfuscation", os.urandom(32).hex(), "--mimic", "--mimic-name", a.http3]
    if a.camuffamento:
        prot += ["--obfuscation", os.urandom(32).hex()]
    if a.rumore > 0:
        prot += ["--cover", str(a.rumore)]
    y = ["general:", "  stop_time: %ds" % (T_SCARICA + 30), "  model_unblocked_syscall_latency: true",
         "network:", "  graph:", "    type: gml", "    inline: |", "      graph [",
         "        node [", "          id 0", '          host_bandwidth_down "50 Mbit"', '          host_bandwidth_up "20 Mbit"',
         "        ]", "        edge [", "          source 0", "          target 0", '          latency "30 ms"',
         "          packet_loss 0.0", "        ]", "      ]",
         "host_option_defaults:", "  pcap_enabled: true", "  pcap_capture_size: 2000 B", "hosts:"]
    for i in range(1, 5):
        y += ["  nodo%d:" % i, "    network_node_id: 0", "    ip_addr: %s" % NODI[i - 1], "    processes:",
              "    - path: %s" % BIN,
              "      args: %s" % lista("node", "--listen", "0.0.0.0:%d" % a.porta, "--data", str(out / ("nodo%d" % i)),
                                       "--announce", "%s:%d" % (NODI[i - 1], a.porta), *prot),
              "      start_time: 1s", "      expected_final_state: running"]
    y += ["  sito:", "    network_node_id: 0", "    ip_addr: %s" % SITO, "    processes:",
          "    - path: /usr/bin/python3",
          "      args: %s" % lista("-m", "http.server", "80", "--bind", SITO, "--directory", str(out / "sito")),
          "      start_time: 1s", "      expected_final_state: running",
          "  utente:", "    network_node_id: 0", "    ip_addr: %s" % UTENTE, "    processes:",
          "    - path: %s" % BIN,
          "      args: %s" % lista("start", "--directory", str(out / "directory.txt"), "--authority", pub,
                                   "--socks", "127.0.0.1:1080", "--circuits", "2", *prot),
          '      environment: { NYCTALUS_GUARDIE: "%s" }' % (out / "guardie"),
          "      start_time: 5s", "      expected_final_state: running",
          "    - path: /usr/bin/curl",
          "      args: %s" % lista("-s", "-o", str(out / "scaricato.bin"), "--socks5-hostname", "127.0.0.1:1080",
                                   "http://%s/file.bin" % SITO),
          "      start_time: %ds" % T_SCARICA, "      expected_final_state: { exited: 0 }"]
    (out / "shadow.yaml").write_text("\n".join(y) + "\n")
    return contenuto


def pacchetti(p):
    """[(tempo, src, dst, proto, porta_s, porta_d, carico)] dal pcap (carico = dati UDP/TCP)."""
    b = p.read_bytes()
    fine = "<" if b[:4] in (b"\xd4\xc3\xb2\xa1", b"\x4d\x3c\xb2\xa1") else ">"
    nano = b[:4] in (b"\x4d\x3c\xb2\xa1", b"\xa1\xb2\x3c\x4d")
    tipo = struct.unpack(fine + "I", b[20:24])[0]
    off, out = 24, []
    while off + 16 <= len(b):
        s, us, incl, _ = struct.unpack(fine + "IIII", b[off:off + 16])
        d = b[off + 16:off + 16 + incl]
        off += 16 + incl
        ip = d[14:] if tipo == 1 else d
        if len(ip) < 20 or ip[0] >> 4 != 4:
            continue
        hl = (ip[0] & 15) * 4
        proto = ip[9]
        src, dst = ".".join(map(str, ip[12:16])), ".".join(map(str, ip[16:20]))
        ps = pd = 0
        carico = b""
        if proto == 17 and len(ip) >= hl + 8:
            ps, pd = struct.unpack("!HH", ip[hl:hl + 4])
            carico = ip[hl + 8:]
        elif proto == 6 and len(ip) >= hl + 13:
            ps, pd = struct.unpack("!HH", ip[hl:hl + 4])
            carico = ip[hl + (ip[hl + 12] >> 4) * 4:]
        out.append((s + us / (1e9 if nano else 1e6), src, dst, proto, ps, pd, carico))
    return out


# --- il primo pacchetto QUIC, letto come lo legge chiunque (RFC 9001 §5.2 e appendice A) ---------

def _hkdf_label(segreto, etichetta, lung):
    e = b"tls13 " + etichetta
    info = struct.pack("!H", lung) + bytes([len(e)]) + e + b"\x00"
    out, t, i = b"", b"", 1
    while len(out) < lung:
        t = hmac.new(segreto, t + info + bytes([i]), hashlib.sha256).digest()
        out += t
        i += 1
    return out[:lung]


def _varint(b, i):
    l = 1 << (b[i] >> 6)
    v = b[i] & 0x3f
    for k in range(1, l):
        v = (v << 8) | b[i + k]
    return v, i + l


def leggi_initial(d):
    """Dati CRYPTO (offset → byte) di un Initial QUIC v1 del client, o None."""
    try:
        from cryptography.hazmat.primitives.ciphers import Cipher, algorithms, modes
        from cryptography.hazmat.primitives.ciphers.aead import AESGCM
    except ImportError:
        return None
    if len(d) < 30 or d[0] & 0xf0 != 0xc0 or d[1:5] != b"\x00\x00\x00\x01":
        return None
    i = 5
    dcid = d[i + 1:i + 1 + d[i]]
    i += 1 + d[i]
    i += 1 + d[i]
    tok, i = _varint(d, i)
    i += tok
    lung, i = _varint(d, i)
    pn_off = i
    sale = bytes.fromhex("38762cf7f55934b34d179ae6a4c80cadccbb7f0a")
    iniziale = hmac.new(sale, dcid, hashlib.sha256).digest()
    client = _hkdf_label(iniziale, b"client in", 32)
    chiave, iv, hp = _hkdf_label(client, b"quic key", 16), _hkdf_label(client, b"quic iv", 12), _hkdf_label(client, b"quic hp", 16)
    campione = d[pn_off + 4:pn_off + 20]
    maschera = Cipher(algorithms.AES(hp), modes.ECB()).encryptor().update(campione)
    primo = d[0] ^ (maschera[0] & 0x0f)
    pn_l = (primo & 3) + 1
    pn = bytes(x ^ y for x, y in zip(d[pn_off:pn_off + pn_l], maschera[1:1 + pn_l]))
    intest = bytes([primo]) + d[1:pn_off] + pn
    nonce = bytes(x ^ y for x, y in zip(iv, b"\x00" * (12 - pn_l) + pn))
    try:
        chiaro = AESGCM(chiave).decrypt(nonce, d[pn_off + pn_l:pn_off + lung], intest)
    except Exception:
        return None
    pezzi, j = {}, 0
    while j < len(chiaro):
        t = chiaro[j]
        if t in (0x00, 0x01):
            j += 1
        elif t == 0x06:
            o, j = _varint(chiaro, j + 1)
            l, j = _varint(chiaro, j)
            pezzi[o] = chiaro[j:j + l]
            j += l
        else:
            break
    return pezzi


def leggi_client_hello(datagrammi):
    """Dal primo collegamento: nome del server (SNI) e protocolli (ALPN) del ClientHello."""
    pezzi = {}
    for d in datagrammi:
        p = leggi_initial(d)
        if p:
            pezzi.update(p)
    flusso, o = b"", 0
    while o in pezzi:
        flusso += pezzi[o]
        o += len(pezzi[o])
    if len(flusso) < 43 or flusso[0] != 1:
        return None
    i = 4 + 2 + 32
    i += 1 + flusso[i]
    i += 2 + struct.unpack("!H", flusso[i:i + 2])[0]
    i += 1 + flusso[i]
    fine = i + 2 + struct.unpack("!H", flusso[i:i + 2])[0]
    i += 2
    sni, alpn = None, []
    while i + 4 <= min(fine, len(flusso)):
        t, l = struct.unpack("!HH", flusso[i:i + 4])
        v = flusso[i + 4:i + 4 + l]
        if t == 0 and len(v) > 5:
            sni = v[5:5 + struct.unpack("!H", v[3:5])[0]].decode(errors="replace")
        elif t == 16:
            k = 2
            while k < len(v):
                alpn.append(v[k + 1:k + 1 + v[k]].decode(errors="replace"))
                k += 1 + v[k]
        i += 4 + l
    return {"sni": sni, "alpn": alpn}


def analizza(out, contenuto):
    h = out / "shadow.data" / "hosts"
    pk = {n: pacchetti(next((h / n).glob("eth0*.pcap"))) for n in ["utente", "sito"] + ["nodo%d" % i for i in range(1, 5)]}
    ip_nome = {NODI[i]: "nodo%d" % (i + 1) for i in range(4)}
    r = {}
    u = pk["utente"]
    rete = [p for p in u if p[1] in NODI or p[2] in NODI]
    r["udp"] = sum(1 for p in rete if p[3] == 17)
    r["tcp"] = sum(1 for p in rete if p[3] == 6)
    r["porte"] = sorted({p[5] for p in rete if p[2] in NODI} | {p[4] for p in rete if p[1] in NODI})
    r["parla_con"] = sorted({ip_nome[p[2]] for p in rete if p[2] in NODI})
    guardia = r["parla_con"][0] if len(r["parla_con"]) == 1 else None
    # QUIC riconoscibile? intestazioni lunghe v1 e lettura del primo pacchetto
    usciti = [p[6] for p in rete if p[1] == UTENTE]
    r["quic_v1"] = sum(1 for d in usciti if len(d) > 5 and d[0] & 0xc0 == 0xc0 and d[1:5] == b"\x00\x00\x00\x01")
    r["hello"] = leggi_client_hello(usciti[:6])
    primi = collections.Counter(d[0] for d in usciti if d)
    attesi = len(usciti) / 256
    r["chi2_primo_byte"] = sum((primi.get(k, 0) - attesi) ** 2 / attesi for k in range(256)) if usciti else 0
    # l'uscita: il nodo che parla TCP con il sito
    uscita = next((ip_nome[p[1]] for p in pk["sito"] if p[3] == 6 and p[1] in NODI), None)
    r["guardia"], r["uscita"] = guardia, uscita
    # scaricamento: finestra in cui il sito spedisce
    t_sito = [p[0] for p in pk["sito"] if p[1] == SITO and p[3] == 6 and len(p[6]) > 0]
    t0, t1 = (min(t_sito), max(t_sito) + 1.0) if t_sito else (T_SCARICA, T_SCARICA + 5)
    r["byte_nodi"] = {}
    for n in ["nodo%d" % i for i in range(1, 5)]:
        r["byte_nodi"][n] = sum(len(p[6]) for p in pk[n] if t0 <= p[0] <= t1 and p[3] == 17)
    r["intermedi"] = [n for n in r["byte_nodi"] if n not in (guardia, uscita)]
    # cipolla: pezzi del file cercati in chiaro su ogni macchina
    rnd = random.Random(7)
    campioni = [contenuto[o:o + 24] for o in (rnd.randrange(0, len(contenuto) - 24) for _ in range(300))]
    r["in_chiaro"] = {}
    for n, lista in pk.items():
        tutto_udp = b"".join(p[6] for p in lista if p[3] == 17)
        tutto_tcp = b"".join(p[6] for p in lista if p[3] == 6)
        r["in_chiaro"][n] = (sum(c in tutto_udp for c in campioni), sum(c in tutto_tcp for c in campioni))
    # rumore di fondo: pacchetti utente↔entrata mentre l'utente è fermo
    fermo = [p for p in rete if FERMO[0] <= p[0] <= FERMO[1]]
    r["fermo_pacchetti_s"] = len(fermo) / (FERMO[1] - FERMO[0])
    # forma: lunghezze dei datagrammi durante lo scaricamento
    giu = [len(p[6]) for p in rete if p[2] == UTENTE and t0 <= p[0] <= t1]
    r["lunghezze_giu"] = collections.Counter(giu).most_common(4)
    r["n_giu"] = len(giu)
    r["scaricato_ok"] = (out / "scaricato.bin").exists() and (out / "scaricato.bin").read_bytes() == contenuto
    return r


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("cartella")
    ap.add_argument("--camuffamento", action="store_true")
    ap.add_argument("--rumore", type=float, default=0.0)
    ap.add_argument("--http3", default="", metavar="NOME", help="forma HTTP/3 con questo nome del server")
    ap.add_argument("--porta", type=int, default=4600)
    a = ap.parse_args()
    out = Path(a.cartella).resolve()
    contenuto = prepara(out, a)
    subprocess.run([str(SHADOW), "--progress", "false", "shadow.yaml"], cwd=out,
                   stdout=open(out / "shadow.log", "w"), stderr=subprocess.STDOUT)
    r = analizza(out, contenuto)
    print("== %s · rumore %s/s · porta %d ==" % ("forma HTTP/3 (%s)" % a.http3 if a.http3 else "camuffamento " + ("SÌ" if a.camuffamento else "no"), a.rumore, a.porta))
    print("scaricamento di %d KB arrivato intero: %s" % (FILE_KB, r["scaricato_ok"]))
    print("1. utente↔rete: UDP %d pacchetti, TCP %d · porte dei nodi %s" % (r["udp"], r["tcp"], r["porte"]))
    print("2. intestazioni QUIC v1 leggibili in uscita: %d · primo byte, chi² su 256 valori: %.0f (rumore puro ≈ 255)"
          % (r["quic_v1"], r["chi2_primo_byte"]))
    print("   primo pacchetto letto da un osservatore: %s" % (r["hello"] or "illeggibile"))
    print("3. l'utente parla con: %s (entrata: %s, uscita: %s)" % (r["parla_con"], r["guardia"], r["uscita"]))
    print("4. byte UDP per nodo durante lo scaricamento: %s · intermedi %s"
          % (", ".join("%s %d KB" % (n, b // 1024) for n, b in r["byte_nodi"].items()), r["intermedi"]))
    print("5. pezzi del file (su 300) trovati in chiaro, UDP/TCP: %s"
          % ", ".join("%s %d/%d" % (n, x, y) for n, (x, y) in r["in_chiaro"].items()))
    print("6. utente fermo: %.1f pacchetti/s fra utente ed entrata" % r["fermo_pacchetti_s"])
    print("7. lunghezze più frequenti dei datagrammi in discesa (su %d): %s" % (r["n_giu"], r["lunghezze_giu"]))


if __name__ == "__main__":
    sys.exit(main())
