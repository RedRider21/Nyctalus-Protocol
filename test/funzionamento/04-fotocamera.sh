#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Fotocamera e anteprima d'invio della chat del PC, con SORGENTI DI PROVA di
# GStreamer al posto di webcam e microfono (la webcam vera non viene mai accesa).
# Servono un display grafico e GStreamer.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · fotocamera e anteprima d'invio (sorgenti di prova)"
[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] || { salta "nessun display grafico"; exit 0; }
python3 -c 'import gi; gi.require_version("Gst","1.0")' 2>/dev/null || { salta "GStreamer per Python non installato"; exit 0; }
XDG_CACHE_HOME="$TMP/cache" PYTHONPATH="$ROOT/pc/chat:$ROOT/pc/browser" timeout 60 \
  python3 "$STRUMENTI/prova_fotocamera.py" > "$TMP/r.txt" 2>/dev/null
FOTO=$(grep -oE '^foto .*' "$TMP/r.txt" | cut -d' ' -f2-); VIDEO=$(grep -oE '^video .*' "$TMP/r.txt" | cut -d' ' -f2-)
[ -s "$FOTO" ] && file "$FOTO" | grep -q JPEG && pass "foto scattata (JPEG)" || fail "foto non scattata"
if [ -s "$VIDEO" ]; then
  INFO=$(ffprobe -v error -show_entries stream=codec_name:format=duration -of csv=p=0 "$VIDEO" 2>/dev/null | tr '\n' ' ')
  echo "$INFO" | grep -q h264 && pass "video registrato (H.264)" || fail "video senza traccia H.264: $INFO"
  echo "$INFO" | grep -q aac && pass "audio registrato (AAC)" || fail "video senza audio: $INFO"
  D=$(echo "$INFO" | grep -oE '[0-9]+\.[0-9]+' | tail -1)
  python3 -c "import sys; sys.exit(0 if 2.0 < float('$D') < 6.0 else 1)" && pass "durata coerente (${D}s) e file chiuso correttamente" || fail "durata anomala: $D"
else
  fail "video non registrato"
fi
grep -q "^anteprima ok" "$TMP/r.txt" && pass "finestra d'anteprima con didascalia e opzione EXIF" || fail "anteprima d'invio"
[ "$KO" -eq 0 ]
