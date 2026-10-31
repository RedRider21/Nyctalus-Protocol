#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Chiamata dalla chat GTK del PC (microfono e altoparlante DI PROVA, mai quelli veri): 📞,
# risposta, voce Opus nei due sensi attraverso la rete di prova, barra con durata, muto,
# volume, riaggancia.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · chiamata dalla chat del PC"
[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] || { salta "nessun display grafico"; exit 0; }
python3 -c 'import gi; gi.require_version("Gst","1.0"); from gi.repository import Gst; Gst.init(None); assert Gst.ElementFactory.find("opusenc")' 2>/dev/null \
  || { salta "GStreamer con Opus non installato"; exit 0; }
assicura_binario
rete_di_prova 21100
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:21180 --hops 3 --control 127.0.0.1:21190 > "$TMP/c.log" 2>&1 & PIDS+=($!)
sleep 2
SA=alicesegreto0123456789abcdef; SB=bobsegreto0123456789abcdef00
identita alice 21111 21121 21180 "$SA"; ALICE=$ADDR
identita bob 21112 21122 21180 "$SB"; BOB=$ADDR
sleep 4
HOME="$TMP/home" NYCTALUS_CHAT_PORT=21121 NYCTALUS_CHAT_TOKEN="$SA" PYTHONPATH="$ROOT/pc/chat:$ROOT/pc/browser" \
  NYCTALUS_MIC_SRC="audiotestsrc is-live=true wave=sine freq=330" NYCTALUS_ALTOPARLANTE="fakesink" \
  timeout 90 python3 "$STRUMENTI/prova_chiamata_pc.py" 21122 "$SB" "$ALICE" "$BOB" "${NYCT_FOTO:-$TMP}/chiamata-pc.png" > "$TMP/r.txt" 2>"$TMP/e.txt" \
  || echo "KO prova interrotta: $(grep -v Warning "$TMP/e.txt" | tail -2)" >> "$TMP/r.txt"
while read -r esito resto; do
  case "$esito" in OK) pass "$resto" ;; KO) fail "$resto" ;; esac
done < "$TMP/r.txt"
[ "$KO" -eq 0 ]
