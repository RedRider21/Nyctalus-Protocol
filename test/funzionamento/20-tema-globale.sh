#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Tema globale del PC: scelto nel mini menu (⚙ → Tema) o nella chat, vale per tutti i
# programmi aperti, al volo. HOME e configurazione temporanee: il tema vero non si tocca.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · tema globale del PC (mini menu ↔ chat)"
[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] || { salta "nessun display grafico"; exit 0; }
mkdir -p "$TMP/home/.config"
HOME="$TMP/home" XDG_CONFIG_HOME="$TMP/home/.config" NYCTALUS_CHAT_PORT=21999 NYCTALUS_CHAT_TOKEN=x NYCTALUS_SENZA_SUONI=1 \
  PYTHONPATH="$ROOT/pc/chat:$ROOT/pc/launcher:$ROOT/pc/browser" \
  timeout 120 python3 "$STRUMENTI/prova_tema_globale.py" > "$TMP/r.txt" 2>"$TMP/e.txt" \
  || echo "KO prova interrotta (uscita $?): $(grep -v Warning "$TMP/e.txt" | tail -2)" >> "$TMP/r.txt"
while read -r esito resto; do
  case "$esito" in OK) pass "$resto" ;; KO) fail "$resto" ;; esac
done < "$TMP/r.txt"
[ "$KO" -eq 0 ]
