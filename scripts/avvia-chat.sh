#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Avvia la chat .nyct per l'uso reale: motore + servizio .nyct + finestra.
#  - se è configurata la rete PUBBLICA (directory_url + authority in
#    ~/.nyctalus-browser/config.json) la usa: è così che raggiungi le altre
#    persone;
#  - altrimenti avvia una piccola rete di prova LOCALE dedicata, così la
#    finestra si apre comunque e puoi vedere il tuo Contact ID (ma per parlare
#    con qualcuno serve la rete condivisa — vedi DEPLOY.md).
#
# Chiudendo la finestra (o con Ctrl-C) si ferma e pulisce tutto.
set -u
QUI="$(cd "$(dirname "$0")" && pwd)"

# binario: bin/ nel pacchetto, target/release in sviluppo, poi PATH
if   [ -x "$QUI/bin/nyctalus" ];             then BIN="$QUI/bin/nyctalus"
elif [ -x "$QUI/../target/release/nyctalus" ]; then BIN="$QUI/../target/release/nyctalus"
else BIN="$(command -v nyctalus || true)"; fi
[ -n "$BIN" ] || { echo "✗ binario 'nyctalus' non trovato"; exit 1; }

# moduli python: chat/ e browser/ accanto allo script (pacchetto) o un livello su
if [ -d "$QUI/chat" ]; then PYBASE="$QUI"; else PYBASE="$QUI/.."; fi
export PYTHONPATH="$PYBASE/chat:$PYBASE/browser${PYTHONPATH:+:$PYTHONPATH}"

python3 -c 'import gi; gi.require_version("Gtk","3.0")' 2>/dev/null || {
  echo "✗ PyGObject/GTK3 non disponibile — sudo apt install python3-gi gir1.2-gtk-3.0"; exit 1; }

DATI="$HOME/.nyctalus-chat"; mkdir -p "$DATI"
CFG="$HOME/.nyctalus-browser/config.json"
BACKEND="127.0.0.1:1082"; CONTROL="127.0.0.1:1091"
WORK=""; MIEI=()
cleanup() { for p in "${MIEI[@]:-}"; do kill "$p" 2>/dev/null; done; [ -n "$WORK" ] && rm -rf "$WORK"; }
trap cleanup EXIT INT TERM

leggi_cfg() { python3 -c "import json,sys
try: print((json.load(open(sys.argv[1])).get(sys.argv[2]) or '').strip())
except Exception: print('')" "$CFG" "$1" 2>/dev/null; }

URL="$(leggi_cfg directory_url)"; AUTH="$(leggi_cfg authority)"

porta_occupata() { ss -ltn 2>/dev/null | grep -q ":$1\b"; }

if [ -n "$URL" ] && [ -n "$AUTH" ]; then
  # ---- rete PUBBLICA configurata ----
  SOCKS="127.0.0.1:1080"
  WORK="$(mktemp -d /tmp/nyctalus-chat.XXXXXX)"
  echo "» uso la rete pubblica configurata"
  if command -v curl >/dev/null 2>&1; then curl -fsS "$URL" -o "$WORK/dir.txt" || { echo "✗ non riesco a scaricare l'elenco firmato da $URL"; exit 1; }
  else python3 -c "import urllib.request,sys;open(sys.argv[2],'wb').write(urllib.request.urlopen(sys.argv[1],timeout=25).read())" "$URL" "$WORK/dir.txt" || { echo "✗ download elenco fallito"; exit 1; }; fi
  if ! porta_occupata 1080; then
    "$BIN" start --directory "$WORK/dir.txt" --authority "$AUTH" --socks "$SOCKS" > "$WORK/daemon.log" 2>&1 & MIEI+=($!)
    sleep 3
  else
    echo "  (riuso il proxy SOCKS già attivo su 1080)"
  fi
  DIRFILE="$WORK/dir.txt"; AUTHORITY="$AUTH"
else
  # ---- nessuna configurazione: rete di prova LOCALE dedicata ----
  SOCKS="127.0.0.1:1086"   # porta dedicata per non confliggere col browser
  WORK="$(mktemp -d /tmp/nyctalus-chat.XXXXXX)"; cd "$WORK"
  echo "» nessuna rete pubblica configurata: avvio una rete di prova LOCALE"
  echo "  (per parlare con altre persone configura la rete condivisa — vedi DEPLOY.md)"
  "$BIN" authority > auth.txt 2>&1
  SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' auth.txt | awk '{print $2}')
  AUTHORITY=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' auth.txt | awk '{print $2}')
  NODI=(5840 5841 5842 5843); LINES=""
  for p in "${NODI[@]}"; do porta_occupata "$p" && { echo "✗ porta $p occupata"; exit 1; }; "$BIN" node --listen 127.0.0.1:"$p" > "n$p.log" 2>&1 & MIEI+=($!); done
  for p in "${NODI[@]}"; do
    L=""; for _ in $(seq 1 60); do L=$(grep -oE "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+" "n$p.log" | head -1); [ -n "$L" ] && break; sleep 0.25; done
    [ -z "$L" ] && { echo "✗ nodo $p non partito"; cat "n$p.log"; exit 1; }
    LINES="$LINES${LINES:+;}$L"
  done
  "$BIN" directory --authority-secret "$SEC" --entries "$LINES" > dir.txt 2>dir.err
  grep -q '^firma ' dir.txt || { echo "✗ non riesco a firmare l'elenco"; cat dir.err; exit 1; }
  "$BIN" circuit --directory dir.txt --authority "$AUTHORITY" --socks "$SOCKS" --hops 3 --control 127.0.0.1:1596 > daemon.log 2>&1 & MIEI+=($!)
  sleep 3
  DIRFILE="$WORK/dir.txt"
fi

echo "» avvio il motore della chat"
"$BIN" chat --data "$DATI" --backend "$BACKEND" --control "$CONTROL" --socks "$SOCKS" > "$WORK/chat.log" 2>&1 & MIEI+=($!)
# attendi la creazione dell'identità e l'indirizzo
ADDR=""; for _ in $(seq 1 50); do ADDR=$(grep -oE '[a-z2-7]{56}\.nyct' "$WORK/chat.log" | head -1); [ -n "$ADDR" ] && break; sleep 0.2; done
[ -z "$ADDR" ] && { echo "✗ motore chat non partito"; cat "$WORK/chat.log"; exit 1; }
echo "  il tuo Contact ID:  $ADDR"

echo "» pubblico il tuo servizio .nyct"
"$BIN" site --backend "$BACKEND" --identity "$DATI/identita" --directory "$DIRFILE" --authority "$AUTHORITY" --hops 3 > "$WORK/site.log" 2>&1 & MIEI+=($!)
sleep 3

echo "» apro la finestra della chat (chiudila per fermare tutto)"
NYCTALUS_CHAT_PORT=1091 python3 -m nyctalus_chat
