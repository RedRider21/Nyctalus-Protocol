#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# «Info» del messaggio nella chat del PC: quando un nostro messaggio è stato messo nella casella,
# consegnato e visualizzato (dalle spunte del motore, che ora portano l'ora). Finto motore.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · info del messaggio (chat del PC)"
[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] || { salta "nessun display grafico"; exit 0; }
mkdir -p "$TMP/home/.config"
HOME="$TMP/home" XDG_CONFIG_HOME="$TMP/home/.config" NYCTALUS_CHAT_PORT=21998 NYCTALUS_CHAT_TOKEN=x NYCTALUS_SENZA_SUONI=1 \
  PYTHONPATH="$ROOT/pc/chat:$ROOT/pc/browser" \
  timeout 90 python3 "$STRUMENTI/prova_info_pc.py" > "$TMP/r.txt" 2>"$TMP/e.txt" \
  || echo "KO prova interrotta (uscita $?): $(grep -v Warning "$TMP/e.txt" | tail -2)" >> "$TMP/r.txt"
while read -r esito resto; do
  case "$esito" in OK) pass "$resto" ;; KO) fail "$resto" ;; esac
done < "$TMP/r.txt"
[ "$KO" -eq 0 ]
