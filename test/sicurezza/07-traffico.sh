#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# «Wireshark in mezzo»: tutta la rete di prova (4 nodi + due apparecchi, Alice e Bob, ognuno
# col suo client) gira in uno SPAZIO DI RETE ISOLATO, dove dumpcap registra ogni pacchetto.
# Alice e Bob si scambiano messaggio, file e videochiamata con PAROLE SPIA dentro; poi la
# cattura (dumpcap, il motore di cattura di Wireshark) si analizza con gli occhi di chi sta in
# mezzo (strumenti/analizza_cattura.py).
# Niente permessi di amministratore (spazio utente + rete propria), la rete vera non si tocca.
# NYCT_CATTURA=percorso.pcap conserva la cattura per aprirla in Wireshark.
if [ -z "${NYCT_NETNS:-}" ]; then
  command -v unshare >/dev/null && command -v dumpcap >/dev/null || {
    . "$(dirname "$0")/../lib.sh"; titolo "Sicurezza · traffico in rete (cattura dei pacchetti)"; salta "servono unshare e dumpcap (pacchetto wireshark-common)"; exit 0; }
  unshare -rn true 2>/dev/null || {
    . "$(dirname "$0")/../lib.sh"; titolo "Sicurezza · traffico in rete (cattura dei pacchetti)"; salta "spazi di rete utente non permessi dal sistema"; exit 0; }
  exec unshare -rn env NYCT_NETNS=1 bash "$0" "$@"
fi
. "$(dirname "$0")/../lib.sh"
titolo "Sicurezza · traffico in rete (cattura dei pacchetti, «Wireshark in mezzo»)"
ip link set lo up
assicura_binario
CAP="$TMP/cattura.pcap"
dumpcap -q -P -i lo -w "$CAP" > "$TMP/dumpcap.log" 2>&1 & TCPD=$!
sleep 1.5
rete_di_prova 21300
# due apparecchi distinti, ognuno con il proprio client della rete (circuiti propri)
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:21380 --hops 3 --control 127.0.0.1:21390 > "$TMP/ca.log" 2>&1 & PIDS+=($!)
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:21381 --hops 3 --control 127.0.0.1:21391 > "$TMP/cb.log" 2>&1 & PIDS+=($!)
sleep 2
SA=alicesegreto0123456789abcdef; SB=bobsegreto0123456789abcdef00
identita alice 21311 21321 21380 "$SA"; ALICE=$ADDR
identita bob 21312 21322 21381 "$SB"; BOB=$ADDR
[ -n "$ALICE" ] && [ -n "$BOB" ] || { fail "identità non partite"; exit 1; }
sleep 5
python3 "$STRUMENTI/prova_traffico.py" 21321 "$SA" 21322 "$SB" "$ALICE" "$BOB" "$TMP" "$TMP/fasi.txt" > "$TMP/r.txt" 2>&1 \
  || echo "KO scenario interrotto: $(tail -1 "$TMP/r.txt")" >> "$TMP/r.txt"
sleep 1
kill "$TCPD" 2>/dev/null; wait "$TCPD" 2>/dev/null
# porte dei servizi locali (app ↔ motore, SOCKS, servizi .nyct): TCP che non lascia l'apparecchio
LOCALI=21311,21312,21321,21322,21380,21381,21390,21391
python3 "$STRUMENTI/analizza_cattura.py" "$CAP" "$TMP/fasi.txt" 21300,21301,21302,21303 "$LOCALI" "$ALICE" "$BOB" >> "$TMP/r.txt" 2>&1 \
  || echo "KO analisi interrotta: $(tail -1 "$TMP/r.txt")" >> "$TMP/r.txt"
[ -n "${NYCT_CATTURA:-}" ] && cp "$CAP" "$NYCT_CATTURA" 2>/dev/null
while read -r esito resto; do
  case "$esito" in OK) pass "$resto" ;; KO) fail "$resto" ;; INFO) echo "  ℹ  $resto" ;; esac
done < "$TMP/r.txt"
[ "$KO" -eq 0 ]
