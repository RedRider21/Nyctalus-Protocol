#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""OSSERVATORE DELLA FORMA «VIDEOCHIAMATA» (PIANO-ANONIMATO, invisibilità punto 2): fa la parte
di chi guarda il traffico (un filtro DPI), senza bisogno di tshark. Sta in mezzo come relè UDP:
ogni porta PUBBLICA inoltra alla porta VERA del nodo, e ogni datagramma, nei due sensi, viene
classificato come lo farebbe un analizzatore di rete:

  - STUN (RFC 5389): primi due bit 00, «magic cookie» 0x2112A442, lunghezza giusta, FINGERPRINT
    (CRC32 ^ 0x5354554e) corretto;
  - RTP/SRTP (RFC 3550): versione 2, tipo di contenuto dinamico (96-127), SSRC stabile per flusso,
    numeri di sequenza crescenti;
  - altro: tutto il resto (es. QUIC in chiaro o rumore) → la forma NON regge.

uso: osserva_webrtc.py --coppie PUBBLICA:VERA[,…] --rapporto FILE.json [--secondi N]
Il rapporto si riscrive ogni secondo (e alla fine).
"""
import argparse
import json
import select
import socket
import struct
import time
import zlib

MAGIA = 0x2112A442


def classifica(d):
    if len(d) >= 20 and d[0] >> 6 == 0 and struct.unpack("!I", d[4:8])[0] == MAGIA:
        lung = struct.unpack("!H", d[2:4])[0]
        if lung + 20 != len(d) or lung % 4:
            return "stun-rotto", None
        # FINGERPRINT in fondo
        if len(d) >= 28 and struct.unpack("!H", d[-8:-6])[0] == 0x8028:
            if struct.unpack("!I", d[-4:])[0] != (zlib.crc32(d[:-8]) ^ 0x5354554E) & 0xFFFFFFFF:
                return "stun-rotto", None
        tipo = struct.unpack("!H", d[:2])[0]
        return {0x0001: "stun-richiesta", 0x0101: "stun-risposta"}.get(tipo, "stun-altro"), None
    if len(d) >= 12 and d[0] >> 6 == 2 and 96 <= (d[1] & 0x7F) <= 127:
        seq, ts, ssrc = struct.unpack("!HII", d[2:12])
        return "srtp", (ssrc, seq)
    return "altro", None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--coppie", required=True)
    ap.add_argument("--rapporto", required=True)
    ap.add_argument("--secondi", type=float, default=0)
    a = ap.parse_args()
    pubblici = {}          # socket pubblico → indirizzo vero del nodo
    for c in a.coppie.split(","):
        pub, vera = c.split(":")
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.bind(("127.0.0.1", int(pub)))
        pubblici[s] = ("127.0.0.1", int(vera))
    verso_nodo = {}        # (socket pubblico, mittente) → socket verso il nodo
    ritorno = {}           # socket verso il nodo → (socket pubblico, mittente)
    conti = {}
    flussi = {}            # ssrc → ultimo seq
    sequenze_rotte = 0
    dimensioni = []
    inizio = time.time()
    ultimo = 0

    def conta(d):
        nonlocal sequenze_rotte
        k, info = classifica(d)
        conti[k] = conti.get(k, 0) + 1
        dimensioni.append(len(d))
        if info:
            ssrc, seq = info
            if ssrc in flussi and ((seq - flussi[ssrc]) & 0xFFFF) > 1000:
                sequenze_rotte += 1
            flussi[ssrc] = seq

    def scrivi():
        tot = sum(conti.values())
        r = {"pacchetti": tot, "classi": conti, "flussi_srtp": len(flussi), "sequenze_rotte": sequenze_rotte,
             "dimensione_media": round(sum(dimensioni) / len(dimensioni), 1) if dimensioni else 0,
             "riconosciuti_percento": round(100 * (tot - conti.get("altro", 0) - conti.get("stun-rotto", 0)) / tot, 2) if tot else 0}
        with open(a.rapporto + ".tmp", "w") as f:
            json.dump(r, f, indent=1)
        import os
        os.replace(a.rapporto + ".tmp", a.rapporto)

    while not a.secondi or time.time() - inizio < a.secondi:
        pronti, _, _ = select.select(list(pubblici) + list(ritorno), [], [], 0.5)
        for s in pronti:
            try:
                d, da = s.recvfrom(65535)
            except OSError:
                continue
            conta(d)
            if s in pubblici:
                chiave = (s, da)
                if chiave not in verso_nodo:
                    u = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                    u.bind(("127.0.0.1", 0))
                    verso_nodo[chiave] = u
                    ritorno[u] = chiave
                verso_nodo[chiave].sendto(d, pubblici[s])
            else:
                pub, mittente = ritorno[s]
                pub.sendto(d, mittente)
        if time.time() - ultimo >= 1:
            scrivi()
            ultimo = time.time()
    scrivi()


if __name__ == "__main__":
    main()
