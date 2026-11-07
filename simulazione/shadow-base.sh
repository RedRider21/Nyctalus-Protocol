#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# PIANO-ANONIMATO, punto K: Nyctalus VERO dentro Shadow (Internet simulata, orologio simulato).
# 4 nodi (gestori e sottoreti diversi) + 1 utente + 1 sito; ogni collegamento ha RITARDO ms di latenza; l'utente scarica tre volte
# un file da 2 MB attraverso la rete (3 salti) e se ne misurano tempi e velocità.
#   shadow-base.sh CARTELLA [RITARDO_MS] [OPZIONI_UTENTE…]   (es. --cover 5)
set -eu
QUI="$(cd "$(dirname "$0")" && pwd)"; ROOT="$(cd "$QUI/.." && pwd)"
BIN="$ROOT/target/release/nyctalus"
SHADOW="${SHADOW:-$HOME/.local/share/nyctalus-sim/installato/bin/shadow}"
OUT="$(mkdir -p "$1" && cd "$1" && pwd)"; RITARDO="${2:-30}"; shift 2 || shift $#
EXTRA=("$@")
rm -rf "$OUT"/*; mkdir -p "$OUT/sito"
head -c 2097152 /dev/urandom > "$OUT/sito/file.bin"

# identità dei nodi (create fuori da Shadow, come fa l'installatore) e elenco firmato
"$BIN" authority > "$OUT/autorita.txt"
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$OUT/autorita.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$OUT/autorita.txt" | awk '{print $2}')
RIGHE=""
for i in 1 2 3 4; do
  mkdir -p "$OUT/nodo$i"
  "$BIN" node --listen 127.0.0.1:0 --data "$OUT/nodo$i" --announce "11.$i.0.1:4600" > /dev/null 2>&1 & P=$!
  for _ in $(seq 1 50); do [ -s "$OUT/nodo$i/riga-elenco.txt" ] && break; sleep 0.1; done
  kill $P 2>/dev/null; wait $P 2>/dev/null || true
  RIGHE="$RIGHE;$(cat "$OUT/nodo$i/riga-elenco.txt"),gestore$i"
done
"$BIN" directory --authority-secret "$SEC" --entries "${RIGHE#;}" > "$OUT/directory.txt"

lista() { local s="" a; for a in "$@"; do s="$s, \"$a\""; done; echo "[${s#, }]"; }
{
cat <<YAML
general:
  stop_time: 150s
  model_unblocked_syscall_latency: true
network:
  graph:
    type: gml
    inline: |
      graph [
        node [
          id 0
          host_bandwidth_down "50 Mbit"
          host_bandwidth_up "20 Mbit"
        ]
        edge [
          source 0
          target 0
          latency "$RITARDO ms"
          packet_loss 0.0
        ]
      ]
hosts:
YAML
for i in 1 2 3 4; do cat <<YAML
  nodo$i:
    network_node_id: 0
    ip_addr: 11.$i.0.1
    processes:
    - path: $BIN
      args: $(lista node --listen 0.0.0.0:4600 --data "$OUT/nodo$i" --announce "11.$i.0.1:4600")
      start_time: 1s
      expected_final_state: running
YAML
done
cat <<YAML
  sito:
    network_node_id: 0
    ip_addr: 12.0.0.20
    processes:
    - path: /usr/bin/python3
      args: $(lista -m http.server 80 --bind 12.0.0.20 --directory "$OUT/sito")
      start_time: 1s
      expected_final_state: running
  utente:
    network_node_id: 0
    ip_addr: 13.0.0.30
    processes:
    - path: $BIN
      args: $(lista start --directory "$OUT/directory.txt" --authority "$PUB" --socks 127.0.0.1:1080 --circuits 2 ${EXTRA[@]+"${EXTRA[@]}"})
      environment: { NYCTALUS_GUARDIE: "$OUT/guardie" }
      start_time: 5s
      expected_final_state: running
YAML
for t in 30 60 90; do cat <<YAML
    - path: /usr/bin/curl
      args: $(lista -s -o /dev/null -w "scaricato %{size_download} byte in %{time_total} s, primo byte dopo %{time_starttransfer} s\n" --socks5-hostname 127.0.0.1:1080 http://12.0.0.20/file.bin)
      start_time: ${t}s
      expected_final_state: { exited: 0 }
YAML
done
} > "$OUT/shadow.yaml"
cd "$OUT" && "$SHADOW" --progress false shadow.yaml > shadow.log 2>&1 || echo "(shadow uscito con errore: vedi $OUT/shadow.log)"
cat "$OUT"/shadow.data/hosts/utente/curl.*.stdout 2>/dev/null
