#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Rifà gli screenshot del README (documenti/immagini/) su DATI DIMOSTRATIVI: HOME finta,
# identità usa e getta, contatti inventati, foto disegnate. Mai i dati veri, mai la
# webcam. Servono un display grafico, ImageMagick (import/convert), wmctrl, Chrome/Chromium.
#   PC:       mini menu, chat, selettore allegati, browser
#   telefono: elenco chat, conversazione, galleria allegati, impostazioni (web-app)
# La schermata di benvenuto e il browser dell'app Android si fotografano dal telefono
# (qui non c'è un emulatore): mettili in documenti/immagini/ come tel-app.png e tel-browser.png.
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/nyctalus"
OUT="$ROOT/documenti/immagini"
H="$(mktemp -d /tmp/nyctalus-screenshot.XXXXXX)"; L="$H/grezze"; mkdir -p "$OUT" "$L"
PIDS=()
pulisci() { for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null || true; done; case "$H" in /tmp/nyctalus-screenshot.*) rm -rf "$H" ;; esac; }
trap pulisci EXIT
[ -x "$BIN" ] || ( cd "$ROOT" && cargo build --release -p nyctalus )
say() { printf '\033[1;36m» %s\033[0m\n' "$*"; }

say "Dati dimostrativi"
python3 "$ROOT/strumenti/screenshot/dati_dimostrativi.py" "$H" "$BIN" > /dev/null

say "PC: chat e selettore allegati"
TOK=$(python3 -c 'import secrets;print(secrets.token_hex(16))')
NYCTALUS_CHAT_TOKEN=$TOK "$BIN" chat --data "$H/.nyctalus-chat" --backend 127.0.0.1:19951 --control 127.0.0.1:19952 \
  --socks 127.0.0.1:19953 > "$H/motore.log" 2>&1 & PIDS+=($!)
sleep 1
HOME="$H" NYCTALUS_CHAT_PORT=19952 NYCTALUS_CHAT_TOKEN=$TOK NYCTALUS_GALLERIA="$H/galleria" NYCTALUS_GALLERIA_SOLO=1 \
  XDG_CACHE_HOME="$H/cache" PYTHONPATH="$ROOT/pc/chat:$ROOT/pc/browser" timeout 90 \
  python3 "$ROOT/strumenti/screenshot/foto_chat_pc.py" "$L" 2>/dev/null

cattura() { # pid file secondi
  local id=""; for _ in $(seq 1 40); do id=$(wmctrl -lp | awk -v p="$1" '$3==p{print $1}' | head -1); [ -n "$id" ] && break; sleep 0.5; done
  sleep "$3"; [ -n "$id" ] && import -window "$id" "$L/$2"; }
say "PC: mini menu e browser"
HOME="$H" PYTHONPATH="$ROOT/pc/launcher:$ROOT/pc/browser" python3 -m nyctalus_launcher > /dev/null 2>&1 & P=$!; PIDS+=($P)
cattura $P pc-launcher.png 2
HOME="$H" PYTHONPATH="$ROOT/pc/browser" python3 -m nyctalus_browser https://redrider21.github.io/Nyctalus-Protocol/ > /dev/null 2>&1 & P=$!; PIDS+=($P)
cattura $P pc-browser.png 8

say "Telefono: web-app"
mkdir -p "$H/.nyctalus-mobile"
echo '{"porte": {"http": 19961, "controllo": 19962, "backend": 19963, "socks": 19964, "socks_locale": 19965, "monitor": 19966, "nodi": [19967,19968,19969,19970], "socks_browser": 19971}}' > "$H/.nyctalus-mobile/config.json"
printf %s demo0123456789abcdef0123456789ab > "$H/.nyctalus-mobile/token"
NYCTALUS_GALLERIA="$H/galleria" NYCTALUS_GALLERIA_SOLO=1 HOME="$H" NYCTALUS_BIN="$BIN" \
  python3 "$ROOT/mobile/orchestratore/nyctalus_mobile.py" > "$H/o.log" 2>&1 & PIDS+=($!)
for _ in $(seq 1 80); do grep -q "servizi pronti" "$H/o.log" && break; sleep 0.5; done
A=$(python3 -c "import json,sys;print(dict(json.load(open(sys.argv[1]))['contatti'])['Alice'])" "$H/demo.json")
cat > "$H/passi.json" <<EOF
[["vai","http://127.0.0.1:19961/?t=demo0123456789abcdef0123456789ab"],["attendi",5],
 ["foto","$L/tel-elenco.png"],
 ["js","apriChat('$A'); 'ok'"],["attendi",3],
 ["js","(()=>{const m=document.getElementById('messaggi'); m.scrollTop=m.scrollHeight; return 'ok'})()"],["attendi",1],
 ["foto","$L/tel-chat.png"],
 ["js","apriGalleria(); 'ok'"],["attendi",4],
 ["js","(()=>{const t=document.querySelectorAll('#foglio .tile[data-i]'); t[1].click(); t[4].click(); return 'ok'})()"],["attendi",1],
 ["foto","$L/tel-allegati.png"],
 ["js","chiudiFoglio(); history.back(); 'ok'"],["attendi",1],
 ["js","impostazioni(); 'ok'"],["attendi",2],
 ["foto","$L/tel-impostazioni.png"]]
EOF
timeout 150 python3 "$ROOT/test/strumenti/cdp.py" "$H/chrome" "$H/passi.json" > /dev/null

say "Ridimensiono in documenti/immagini/"
for f in "$L"/tel-*.png; do convert "$f" -resize 400x -strip "$OUT/$(basename "$f")"; done
for f in "$L"/pc-chat.png "$L"/pc-browser.png; do convert "$f" -resize 900x\> -strip "$OUT/$(basename "$f")"; done
convert "$L/pc-allegati.png" -resize 560x\> -strip "$OUT/pc-allegati.png"
convert "$L/pc-launcher.png" -strip "$OUT/pc-launcher.png"
ls -la "$OUT"
