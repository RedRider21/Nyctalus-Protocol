#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""IMPRONTA DELLA STRETTA DI MANO TLS (PIANO-ANONIMATO, invisibilità punto 2): chi filtra la rete
riconosce i programmi dal loro ClientHello (il primo messaggio TLS, in chiaro). Questo strumento
ascolta su una porta TCP, registra il ClientHello di chi si collega e ne calcola l'impronta **JA4**
(lo standard usato dai filtri: versione, cifrari ed estensioni ordinati, ALPN) più l'elenco
leggibile. Poi chiude: serve solo a confrontare Nyctalus con un browser vero.

uso: impronta_tls.py PORTA [--volte N]   → per ogni collegamento stampa «JA4 …» e i dettagli
"""
import argparse
import hashlib
import socket
import struct

GREASE = {0x0a0a + 0x1010 * i for i in range(16)}
NOMI_EST = {0: "server_name", 5: "status_request", 10: "supported_groups", 11: "ec_point_formats",
            13: "signature_algorithms", 16: "alpn", 17: "status_request_v2", 18: "sct", 21: "padding",
            23: "extended_master_secret", 27: "compress_certificate", 35: "session_ticket", 41: "pre_shared_key",
            43: "supported_versions", 45: "psk_key_exchange_modes", 49: "post_handshake_auth", 51: "key_share",
            17513: "application_settings(ALPS)", 17613: "application_settings_new", 65037: "encrypted_client_hello",
            65281: "renegotiation_info"}


def leggi_hello(c):
    dati = b""
    while len(dati) < 5:
        b = c.recv(65536)
        if not b:
            return None
        dati += b
    lung = struct.unpack("!H", dati[3:5])[0]
    while len(dati) < 5 + lung:
        b = c.recv(65536)
        if not b:
            break
        dati += b
    return dati[5:5 + lung]


def ja4(h):
    """(stringa JA4, dettagli) dal corpo dell'handshake ClientHello."""
    assert h[0] == 1, "non è un ClientHello"
    i = 4 + 2 + 32
    i += 1 + h[i]
    nc = struct.unpack("!H", h[i:i + 2])[0]
    cifrari = [struct.unpack("!H", h[i + 2 + k:i + 4 + k])[0] for k in range(0, nc, 2)]
    i += 2 + nc
    i += 1 + h[i]
    fine = i + 2 + struct.unpack("!H", h[i:i + 2])[0]
    i += 2
    est, alpn, sni, versioni, firme = [], [], False, [], []
    while i + 4 <= fine:
        t, l = struct.unpack("!HH", h[i:i + 4])
        v = h[i + 4:i + 4 + l]
        est.append(t)
        if t == 0:
            sni = True
        elif t == 16:
            k = 2
            while k < len(v):
                alpn.append(v[k + 1:k + 1 + v[k]].decode(errors="replace"))
                k += 1 + v[k]
        elif t == 43:
            versioni = [struct.unpack("!H", v[1 + k:3 + k])[0] for k in range(0, v[0], 2)]
        elif t == 13:
            firme = [struct.unpack("!H", v[2 + k:4 + k])[0] for k in range(0, struct.unpack("!H", v[:2])[0], 2)]
        i += 4 + l
    vere_v = [x for x in versioni if x not in GREASE]
    ver = {0x0304: "13", 0x0303: "12"}.get(max(vere_v) if vere_v else 0x0303, "00")
    c_ok = [x for x in cifrari if x not in GREASE]
    e_ok = [x for x in est if x not in GREASE]
    a = alpn[0] if alpn else ""
    alpn_c = (a[0] + a[-1]) if a else "00"
    parte_a = "t%s%s%02d%02d%s" % (ver, "d" if sni else "i", min(len(c_ok), 99), min(len(e_ok), 99), alpn_c)
    h_c = hashlib.sha256(",".join("%04x" % x for x in sorted(c_ok)).encode()).hexdigest()[:12]
    e_senza = sorted(x for x in e_ok if x not in (0, 16))
    testo_e = ",".join("%04x" % x for x in e_senza) + ("_" + ",".join("%04x" % x for x in firme) if firme else "")
    h_e = hashlib.sha256(testo_e.encode()).hexdigest()[:12]
    dettagli = {"cifrari": ["%04x" % x for x in c_ok], "estensioni": [NOMI_EST.get(x, str(x)) for x in e_ok],
                "grease": any(x in GREASE for x in cifrari + est), "alpn": alpn, "versioni": ["%04x" % x for x in vere_v]}
    return "%s_%s_%s" % (parte_a, h_c, h_e), dettagli


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("porta", type=int)
    ap.add_argument("--volte", type=int, default=1)
    a = ap.parse_args()
    s = socket.socket()
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.bind(("127.0.0.1", a.porta))
    s.listen()
    for _ in range(a.volte):
        c, _ = s.accept()
        c.settimeout(10)
        h = leggi_hello(c)
        c.close()
        if not h:
            continue
        j, d = ja4(h)
        print("JA4 %s" % j)
        print("    versioni %s · cifrari (%d) %s" % (d["versioni"], len(d["cifrari"]), " ".join(d["cifrari"])))
        print("    estensioni (%d) %s" % (len(d["estensioni"]), ", ".join(d["estensioni"])))
        print("    GREASE %s · ALPN %s" % ("sì" if d["grease"] else "no", d["alpn"]), flush=True)


if __name__ == "__main__":
    main()
