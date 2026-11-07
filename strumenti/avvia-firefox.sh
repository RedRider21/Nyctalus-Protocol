#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Nyctalus Firefox (PIANO-ANONIMATO, punto J, fase 1): il Firefox già installato, con un
# PROFILO SEPARATO E BLINDATO (sul modello di Tor Browser) che manda tutto attraverso la rete
# Nyctalus. Il Firefox di tutti i giorni non viene toccato.
#   avvia-firefox.sh [URL]        apre Firefox (accende la rete se serve, la spegne alla fine)
#   avvia-firefox.sh --prepara P  scrive solo il profilo per la porta P e stampa la cartella
set -u
QUI="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
if [ -d "$QUI/firefox" ]; then FDIR="$QUI/firefox"; else FDIR="$QUI/../pc/firefox"; fi
FDIR="$(cd "$FDIR" && pwd)"
if   [ -x "$QUI/bin/nyctalus" ];               then BIN="$QUI/bin/nyctalus"
elif [ -x "$QUI/../target/release/nyctalus" ]; then BIN="$QUI/../target/release/nyctalus"
else BIN="$(command -v nyctalus || true)"; fi
PROFILO="${NYCTALUS_FIREFOX_PROFILO:-$HOME/.nyctalus-firefox/profilo}"

avviso() {   # nel terminale, o come notifica se aperto dal menu
  echo "✗ $*" >&2
  [ -t 2 ] || { command -v notify-send >/dev/null && notify-send -i dialog-warning "Nyctalus Firefox" "$*"; }
}

prepara() {  # porta → profilo pronto in $PROFILO
  mkdir -p "$PROFILO" && chmod 700 "$(dirname "$PROFILO")" "$PROFILO"
  sed -e "s|@SOCKS_PORT@|$1|" -e "s|@PAGINA@|file://$FDIR/inizio.html|" "$FDIR/user.js" > "$PROFILO/user.js"
}

if [ "${1:-}" = "--prepara" ]; then
  [[ "${2:-}" =~ ^[0-9]+$ ]] || { echo "uso: avvia-firefox.sh --prepara PORTA"; exit 2; }
  prepara "$2"; echo "$PROFILO"; exit 0
fi

FF="${NYCTALUS_FIREFOX:-$(command -v firefox-esr || command -v firefox || true)}"
[ -n "$FF" ] || { avviso "Firefox non è installato: sudo apt install firefox-esr (o firefox)"; exit 1; }

porta_occupata() { ss -ltn 2>/dev/null | grep -q ":$1\b"; }
CFG="$HOME/.nyctalus-browser/config.json"
leggi_cfg() { python3 -c "import json,sys
try: print((json.load(open(sys.argv[1])).get(sys.argv[2]) or '').strip())
except Exception: print('')" "$CFG" "$1" 2>/dev/null; }

MIO=""; WORK=""
fine() { [ -n "$MIO" ] && kill "$MIO" 2>/dev/null; [ -n "$WORK" ] && rm -rf "$WORK"; }
trap fine EXIT INT TERM

if porta_occupata 1080; then
  SOCKS=1080                                   # rete condivisa già accesa (chat o browser)
elif porta_occupata 1086; then
  SOCKS=1086                                   # rete di prova locale della chat
else
  URL="$(leggi_cfg directory_url)"; AUTH="$(leggi_cfg authority)"
  if [ -z "$URL" ] || [ -z "$AUTH" ] || [ -z "$BIN" ]; then
    avviso "Nessuna rete Nyctalus: apri prima la chat, oppure collegati a una rete con nyctalus-rete importa CODICE"
    exit 1
  fi
  WORK="$(mktemp -d /tmp/nyctalus-firefox.XXXXXX)"
  if ! python3 -c "import urllib.request,sys;open(sys.argv[2],'wb').write(urllib.request.urlopen(sys.argv[1],timeout=25).read())" "$URL" "$WORK/dir.txt" 2>/dev/null; then
    avviso "Non riesco a scaricare l'elenco dei nodi della rete ($URL)"; exit 1
  fi
  PROT=()
  CAM="$(leggi_cfg camuffamento)"; RUM="$(leggi_cfg rumore)"
  [[ "$CAM" =~ ^[0-9a-f]{64}$ ]] && PROT+=(--obfuscation "$CAM")
  [[ "$RUM" =~ ^[0-9]+(\.[0-9]+)?$ ]] && [ -n "${PROT[*]:-}" ] && PROT+=(--cover "$RUM")
  "$BIN" start --directory "$WORK/dir.txt" --authority "$AUTH" --socks 127.0.0.1:1080 ${PROT[@]+"${PROT[@]}"} > "$WORK/rete.log" 2>&1 & MIO=$!
  for _ in $(seq 1 60); do porta_occupata 1080 && break; sleep 0.25; done
  porta_occupata 1080 || { avviso "La rete Nyctalus non parte: $(tail -2 "$WORK/rete.log" | tr '\n' ' ')"; exit 1; }
  SOCKS=1080
fi

prepara "$SOCKS"
"$FF" --no-remote --new-instance --profile "$PROFILO" --class NyctalusFirefox ${1:+"$1"}
