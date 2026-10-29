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

# --- selettore allegati come Telegram, su una galleria finta ---
G="$TMP/galleria"; mkdir -p "$G"
for c in 1e88e5 e53935 43a047 fdd835 8e24aa; do
  python3 -c "import zlib,struct,sys;w=h=96;c=bytes.fromhex(sys.argv[1]);raw=b''.join(b'\x00'+c*w for _ in range(h))
def k(t,d):return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d))
open(sys.argv[2],'wb').write(b'\x89PNG\r\n\x1a\n'+k(b'IHDR',struct.pack('>IIBBBBB',w,h,8,2,0,0,0))+k(b'IDAT',zlib.compress(raw))+k(b'IEND',b''))" "$c" "$G/IMG_$c.png"
  sleep 0.05
done
command -v ffmpeg >/dev/null && ffmpeg -v error -f lavfi -i testsrc2=size=320x240:rate=10:duration=3 -pix_fmt yuv420p "$G/VID_prova.mp4"
NYCTALUS_GALLERIA="$G" NYCTALUS_GALLERIA_SOLO=1 XDG_CACHE_HOME="$TMP/cache" PYTHONPATH="$ROOT/pc/chat:$ROOT/pc/browser" timeout 60 \
  python3 "$STRUMENTI/prova_selettore.py" > "$TMP/s.txt" 2>/dev/null
v() { grep -oE "^$1 .*" "$TMP/s.txt" | cut -d' ' -f2-; }
[ "$(v caselle)" -eq 6 ] && pass "selettore: griglia con foto e video recenti ($(v caselle))" || fail "griglia: $(v caselle)"
[ "$(v miniature)" -ge 6 ] && pass "miniature caricate (anche del video)" || fail "miniature: $(v miniature)"
[ "$(v numeri)" = "1,2" ] && pass "selezione numerata come Telegram (1, 2)" || fail "numerazione: $(v numeri)"
[ "$(v bottone)" = "Invia (2)" ] && [ "$(v barra)" = "True" ] && pass "didascalia + «Invia (2)» al posto delle schede" || fail "barra di invio: $(v bottone) $(v barra)"
[ -n "$(v ordine)" ] && pass "ordine di invio = ordine di tocco ($(v ordine))" || fail "ordine dei file scelti"
[ "$KO" -eq 0 ]
