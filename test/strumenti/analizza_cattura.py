# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Legge una cattura .pcap (come Wireshark) e guarda il traffico con gli occhi di chi sta
IN MEZZO alla rete. Il traffico «in rete» è quello UDP fra gli apparecchi e i nodi; il TCP
su 127.0.0.1 verso le porte dei servizi locali (app ↔ motore) non esce mai dall'apparecchio.
Controlli: parole spia, indirizzi .nyct, contatti diretti fra apparecchi, DNS/STUN/mDNS,
TCP inatteso, aspetto cifrato (entropia), QUIC, dimensioni e ritmo dei pacchetti.
Stampa «OK …» / «KO …» / «INFO …».
uso: analizza_cattura.py CATTURA.pcap FASI PORTE_NODI PORTE_LOCALI_TCP IND_A IND_B"""
import collections
import math
import struct
import sys

cattura, fasi_f, nodi, locali, ind_a, ind_b = sys.argv[1:7]
NODI = {int(x) for x in nodi.split(",")}
LOCALI = {int(x) for x in locali.split(",")}
esito = lambda ok, d: print(("OK " if ok else "KO ") + d, flush=True)
info = lambda d: print("INFO " + d, flush=True)


def pacchetti(percorso):
    """(tempo, proto, porta_src, porta_dst, carico) da un pcap (Ethernet o Linux cooked)."""
    with open(percorso, "rb") as f:
        testa = f.read(24)
        magia = struct.unpack("<I", testa[:4])[0]
        e = "<" if magia in (0xA1B2C3D4, 0xA1B23C4D) else ">"
        nano = magia in (0xA1B23C4D, 0x4D3CB2A1)
        tipo = struct.unpack(e + "I", testa[20:24])[0]
        while True:
            h = f.read(16)
            if len(h) < 16:
                return
            s, us, incl, _orig = struct.unpack(e + "IIII", h)
            dati = f.read(incl)
            t = s + us / (1e9 if nano else 1e6)
            if tipo == 1:                    # Ethernet
                eth, off = struct.unpack("!H", dati[12:14])[0], 14
            elif tipo == 113:                # Linux cooked (SLL)
                eth, off = struct.unpack("!H", dati[14:16])[0], 16
            elif tipo == 276:                # Linux cooked v2
                eth, off = struct.unpack("!H", dati[0:2])[0], 20
            else:
                continue
            if eth == 0x0800:
                ihl = (dati[off] & 0x0F) * 4
                proto = dati[off + 9]
                l4 = off + ihl
            elif eth == 0x86DD:
                proto, l4 = dati[off + 6], off + 40
            else:
                continue
            if proto == 17:
                ps, pd = struct.unpack("!HH", dati[l4:l4 + 4])
                yield t, "udp", ps, pd, dati[l4 + 8:]
            elif proto == 6:
                ps, pd = struct.unpack("!HH", dati[l4:l4 + 4])
                doff = (dati[l4 + 12] >> 4) * 4
                yield t, "tcp", ps, pd, dati[l4 + doff:]


fasi, spie = {}, {}
for riga in open(fasi_f):
    p = riga.split()
    if p[0] == "spia":
        spie[p[1]] = p[2].encode()
    else:
        fasi.setdefault(p[0], {})[p[1]] = float(p[2])

tutti = list(pacchetti(cattura))
udp = [x for x in tutti if x[1] == "udp"]
tcp = [x for x in tutti if x[1] == "tcp"]
info("cattura: %d pacchetti (%d UDP «in rete», %d TCP locali)" % (len(tutti), len(udp), len(tcp)))
esito(len(udp) > 500, "la cattura ha visto il traffico della rete (%d pacchetti UDP)" % len(udp))

# --- 1) parole spia e indirizzi: in rete MAI; nel traffico locale sì (prova che la ricerca funziona)
cerca = dict(spie)
cerca.update({"indirizzo di Alice": ind_a.encode(), "indirizzo di Bob": ind_b.encode()})
for nome, s in cerca.items():
    forme = [s, s.hex().encode(), s.hex().upper().encode()]
    in_rete = sum(1 for x in udp if any(f in x[4] for f in forme))
    esito(in_rete == 0, "in rete non compare «%s» (né in chiaro né in esadecimale)" % nome
          + ("" if in_rete == 0 else ": trovato in %d pacchetti" % in_rete))
for nome, s in spie.items():
    if nome == "file":          # l'app passa al motore il PERCORSO, non il contenuto: controllo nello scenario
        continue
    locale = sum(1 for x in tcp if s in x[4] or s.hex().encode() in x[4])
    esito(locale > 0, "controllo: «%s» SI trova nel traffico locale app↔motore (%d), quindi la ricerca funziona" % (nome, locale))

# --- 2) chi parla con chi: ogni pacchetto UDP deve avere un NODO a un capo
diretti = [x for x in udp if x[2] not in NODI and x[3] not in NODI]
esito(not diretti, "nessun collegamento diretto fra apparecchi: ogni pacchetto va a/da un nodo della rete"
      + ("" if not diretti else " (%d diretti, es. %d→%d)" % (len(diretti), diretti[0][2], diretti[0][3])))
estremi = collections.Counter()
for x in udp:
    estremi[x[2] if x[2] not in NODI else x[3]] += 1
info("apparecchi visti in rete (porte proprie): %d; nodi: %d" % (len(estremi), len(NODI)))

# --- 3) niente fughe: DNS, STUN/WebRTC, mDNS/SSDP, TCP verso fuori
SOSPETTE = {53: "DNS", 853: "DNS su TLS", 5353: "mDNS", 1900: "SSDP", 3478: "STUN/WebRTC", 19302: "STUN di Google",
            5349: "TURN", 123: "NTP"}
for porta, nome in SOSPETTE.items():
    n = sum(1 for x in tutti if porta in (x[2], x[3]))
    esito(n == 0, "nessun pacchetto %s (porta %d)" % (nome, porta) + ("" if n == 0 else ": %d" % n))
# (le connessioni rifiutate senza dati sono i controlli «porta libera?» della prova stessa)
fuori = [x for x in tcp if x[2] not in LOCALI and x[3] not in LOCALI and x[4]]
esito(not fuori, "il TCP è solo locale (app ↔ motore/servizi su 127.0.0.1), nessuna connessione TCP in rete"
      + ("" if not fuori else ": %d (es. %d→%d)" % (len(fuori), fuori[0][2], fuori[0][3])))

# --- 4) aspetto: cifrato e QUIC
def entropia(b):
    if not b:
        return 0.0
    c = collections.Counter(b)
    return -sum(v / len(b) * math.log2(v / len(b)) for v in c.values())
grandi = [x[4] for x in udp if len(x[4]) >= 600]
campione = b"".join(grandi[:400])
H = entropia(campione)
esito(H > 7.9, "i pacchetti grandi sembrano rumore casuale (entropia %.3f bit/byte su %d KB; 8 = casuale perfetto)" % (H, len(campione) // 1024))
lunghi = [x for x in udp if x[4] and x[4][0] & 0x80]
versioni = collections.Counter(struct.unpack("!I", x[4][1:5])[0] for x in lunghi if len(x[4]) > 5)
# intestazione corta: primo bit 0. Il secondo («bit fisso») qui è casuale: quinn lo «ingrassa»
# (RFC 9287) apposta, perché nessuno si basi su di esso per riconoscere o bloccare il traffico
corti = [x for x in udp if x[4] and not x[4][0] & 0x80]
esito(versioni.get(1, 0) > 0 and len(corti) + len(lunghi) == len(udp),
      "ogni pacchetto ha la forma QUIC v1 (come HTTP/3, cifrato TLS 1.3): %d d'apertura, %d di dati" % (len(lunghi), len(corti)))
bit = sum(1 for x in corti if x[4][0] & 0x40)
info("bit fisso QUIC acceso nel %.0f%% dei pacchetti di dati: casuale di proposito (RFC 9287, «grease»)" % (100.0 * bit / max(1, len(corti))))

# --- 5) cosa si capisce comunque: dimensioni e ritmo (analisi onesta)
dims = collections.Counter(len(x[4]) for x in udp)
comuni = dims.most_common(3)
info("dimensioni dei pacchetti in rete: %d diverse; le più frequenti %s" % (len(dims), ", ".join("%d B ×%d" % c for c in comuni)))
for nome in ("riposo", "testo", "file", "chiamata", "dopo"):
    f = fasi.get(nome, {})
    if "inizio" not in f or "fine" not in f:
        continue
    dentro = [x for x in udp if f["inizio"] <= x[0] <= f["fine"]]
    durata = f["fine"] - f["inizio"]
    byte = sum(len(x[4]) for x in dentro)
    info("fase %-8s %5.1f s: %6.1f pacchetti/s, %7.1f kbit/s (somma di TUTTI i salti della rete di prova)" % (nome, durata, len(dentro) / durata, byte * 8 / durata / 1000))
r = fasi.get("riposo", {}); c = fasi.get("chiamata", {})
if r and c:
    pr = sum(1 for x in udp if r["inizio"] <= x[0] <= r["fine"]) / (r["fine"] - r["inizio"])
    pc = sum(1 for x in udp if c["inizio"] <= x[0] <= c["fine"]) / (c["fine"] - c["inizio"])
    info("ritmo: in chiamata %.0f× rispetto al riposo — chi osserva la linea di un apparecchio capisce che "
         "sta passando molto traffico (non cosa, né con chi)" % (pc / max(pr, 0.1)))
