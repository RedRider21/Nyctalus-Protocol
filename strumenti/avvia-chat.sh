#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Avvia la chat .nyct per l'uso reale: motore + servizio .nyct + finestra.
# Per non farti aspettare, la FINESTRA si apre subito (il motore parte in un
# istante e mostra il tuo Contact ID); la rete sale in SOTTOFONDO: finché non è
# pronta puoi già vedere la finestra, ma per inviare/ricevere serve qualche
# secondo (o la rete pubblica configurata).
#
#  - rete PUBBLICA (directory_url + authority in ~/.nyctalus-browser/config.json):
#    è così che raggiungi le altre persone;
#  - senza configurazione: piccola rete di prova LOCALE (la finestra si apre
#    comunque, ma non raggiungi altre persone — vedi DEPLOY.md).
#
# Chiudendo la finestra (o con Ctrl-C) si ferma e pulisce tutto.
set -u
QUI="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"   # anche se lanciato da un collegamento

if   [ -x "$QUI/bin/nyctalus" ];               then BIN="$QUI/bin/nyctalus"
elif [ -x "$QUI/../target/release/nyctalus" ]; then BIN="$QUI/../target/release/nyctalus"
else BIN="$(command -v nyctalus || true)"; fi
[ -n "$BIN" ] || { echo "✗ binario 'nyctalus' non trovato"; exit 1; }

if [ -d "$QUI/chat" ]; then PYBASE="$QUI"; else PYBASE="$QUI/../pc"; fi   # pacchetto | progetto
export PYTHONPATH="$PYBASE/chat:$PYBASE/browser${PYTHONPATH:+:$PYTHONPATH}"

python3 -c 'import gi; gi.require_version("Gtk","3.0")' 2>/dev/null || {
  echo "✗ PyGObject/GTK3 non disponibile — sudo apt install python3-gi gir1.2-gtk-3.0"; exit 1; }

DATI="$HOME/.nyctalus-chat"; mkdir -p "$DATI"
CFG="$HOME/.nyctalus-browser/config.json"
BACKEND="127.0.0.1:1082"; CONTROL="127.0.0.1:1091"
WORK="$(mktemp -d /tmp/nyctalus-chat.XXXXXX)"; MIEI=()
cleanup() { for p in "${MIEI[@]:-}"; do kill "$p" 2>/dev/null; done; rm -rf "$WORK"; }
trap cleanup EXIT INT TERM

leggi_cfg() { python3 -c "import json,sys
try: print((json.load(open(sys.argv[1])).get(sys.argv[2]) or '').strip())
except Exception: print('')" "$CFG" "$1" 2>/dev/null; }
porta_occupata() { ss -ltn 2>/dev/null | grep -q ":$1\b"; }

URL="$(leggi_cfg directory_url)"; AUTH="$(leggi_cfg authority)"
if [ -n "$URL" ] && [ -n "$AUTH" ]; then SOCKS="127.0.0.1:1080"; else SOCKS="127.0.0.1:1086"; fi

# Segreto della porta di controllo: solo la nostra finestra potrà usarla
# (passa per variabile d'ambiente, così non compare nell'elenco dei processi).
NYCTALUS_CHAT_TOKEN="$(python3 -c 'import secrets;print(secrets.token_hex(16))')"
export NYCTALUS_CHAT_TOKEN

# --- 1) motore (parte subito) e FINESTRA (subito) ----------------------------
echo "» avvio il motore della chat"
"$BIN" chat --data "$DATI" --backend "$BACKEND" --control "$CONTROL" --socks "$SOCKS" > "$WORK/chat.log" 2>&1 & MIEI+=($!)
ADDR=""; for _ in $(seq 1 40); do ADDR=$(grep -oE '[a-z2-7]{56}\.nyct' "$WORK/chat.log" | head -1); [ -n "$ADDR" ] && break; sleep 0.1; done
[ -n "$ADDR" ] && echo "  il tuo Contact ID:  $ADDR"

echo "» apro la finestra (la rete finisce di salire in sottofondo)"
NYCTALUS_CHAT_PORT=1091 python3 -m nyctalus_chat > "$WORK/ui.log" 2>&1 & UIPID=$!

# --- 2) rete + servizio .nyct, IN SOTTOFONDO ---------------------------------
avvia_rete() {
  if [ -n "$URL" ] && [ -n "$AUTH" ]; then
    echo "» (sottofondo) uso la rete pubblica configurata"
    if command -v curl >/dev/null 2>&1; then curl -fsS "$URL" -o "$WORK/dir.txt" || { echo "✗ download elenco fallito"; return; }
    else python3 -c "import urllib.request,sys;open(sys.argv[2],'wb').write(urllib.request.urlopen(sys.argv[1],timeout=25).read())" "$URL" "$WORK/dir.txt" || { echo "✗ download elenco fallito"; return; }; fi
    if ! porta_occupata 1080; then
      "$BIN" start --directory "$WORK/dir.txt" --authority "$AUTH" --socks 127.0.0.1:1080 > "$WORK/daemon.log" 2>&1 & MIEI+=($!)
    fi
    DIRFILE="$WORK/dir.txt"; AUTHORITY="$AUTH"
  else
    echo "» (sottofondo) nessuna rete pubblica: avvio una rete di prova LOCALE"
    cd "$WORK" || return
    "$BIN" authority > auth.txt 2>&1
    local SEC; SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' auth.txt | awk '{print $2}')
    AUTHORITY=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' auth.txt | awk '{print $2}')
    local NODI=(5840 5841 5842 5843) LINES="" p L
    for p in "${NODI[@]}"; do porta_occupata "$p" && { echo "✗ porta $p occupata"; return; }; "$BIN" node --listen 127.0.0.1:"$p" > "n$p.log" 2>&1 & MIEI+=($!); done
    for p in "${NODI[@]}"; do L=""; for _ in $(seq 1 80); do L=$(grep -oE "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+" "n$p.log" | head -1); [ -n "$L" ] && break; sleep 0.2; done; LINES="$LINES${LINES:+;}$L"; done
    "$BIN" directory --authority-secret "$SEC" --entries "$LINES" > dir.txt 2>dir.err
    grep -q '^firma ' dir.txt || { echo "✗ firma elenco fallita"; return; }
    "$BIN" circuit --directory dir.txt --authority "$AUTHORITY" --socks 127.0.0.1:1086 --hops 3 --control 127.0.0.1:1596 > daemon.log 2>&1 & MIEI+=($!)
    DIRFILE="$WORK/dir.txt"
  fi
  # pubblica il nostro servizio .nyct (entrata)
  for _ in $(seq 1 30); do porta_occupata "${SOCKS##*:}" && break; sleep 0.2; done
  "$BIN" site --backend "$BACKEND" --identity "$DATI/identita" --directory "$DIRFILE" --authority "$AUTHORITY" --hops 3 > "$WORK/site.log" 2>&1 & MIEI+=($!)
  echo "» (sottofondo) rete pronta — puoi scrivere"
}
# NB: in primo piano nella shell principale (la finestra è GIÀ aperta sopra),
# così i PID dei nodi/circuito/servizio finiscono in MIEI e il cleanup li chiude.
avvia_rete

# --- 3) resta vivo finché la finestra è aperta -------------------------------
wait "$UIPID"
