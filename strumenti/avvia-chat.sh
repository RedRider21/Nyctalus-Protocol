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
export PYTHONPATH="$PYBASE/chat:$PYBASE/file:$PYBASE/browser${PYTHONPATH:+:$PYTHONPATH}"
# quale finestra aprire sopra il motore: la chat (predefinita) o il programma File
INTERFACCIA="${NYCTALUS_INTERFACCIA:-nyctalus_chat}"
# «nessuna»: solo il motore, senza finestre (nyctalus-cli avvia: server, SSH, Raspberry)
case "$INTERFACCIA" in nyctalus_chat|nyctalus_file|nessuna) ;; *) INTERFACCIA=nyctalus_chat ;; esac

[ "$INTERFACCIA" = nessuna ] || python3 -c 'import gi; gi.require_version("Gtk","3.0")' 2>/dev/null || {
  echo "✗ PyGObject/GTK3 non disponibile — sudo apt install python3-gi gir1.2-gtk-3.0"; exit 1; }

DATI="$HOME/.nyctalus-chat"; mkdir -p "$DATI"
COLLEGAMENTO="$DATI/controllo"            # «porta segreto» del motore acceso (privato, 600)
FINESTRE="$DATI/finestre"; mkdir -p "$FINESTRE"   # le finestre aperte sopra questo motore

# Motore già acceso (es. la chat è aperta e ora si apre File, o viceversa): ci si aggancia,
# senza accenderne un secondo. Il motore resta acceso finché c'è almeno una finestra.
if ss -ltn 2>/dev/null | grep -q ":1091\b" && [ -s "$COLLEGAMENTO" ]; then
  if [ "$INTERFACCIA" = nessuna ]; then
    echo "» il motore di Nyctalus è già acceso: usa pure nyctalus-cli"; exit 0
  fi
  read -r PORTA SEGRETO < "$COLLEGAMENTO"
  NYCTALUS_CHAT_PORT="$PORTA" NYCTALUS_CHAT_TOKEN="$SEGRETO" python3 -m "$INTERFACCIA" & UIPID=$!
  echo "$UIPID" > "$FINESTRE/$UIPID"
  wait "$UIPID"; rm -f "$FINESTRE/$UIPID"
  exit 0
fi
CFG="$HOME/.nyctalus-browser/config.json"
BACKEND="127.0.0.1:1082"; CONTROL="127.0.0.1:1091"
WORK="$(mktemp -d /tmp/nyctalus-chat.XXXXXX)"; MIEI=()
cleanup() { for p in "${MIEI[@]:-}"; do kill "$p" 2>/dev/null; done; rm -rf "$WORK"; rm -f "$COLLEGAMENTO"; }
trap cleanup EXIT INT TERM

leggi_cfg() { python3 -c "import json,sys
try: print((json.load(open(sys.argv[1])).get(sys.argv[2]) or '').strip())
except Exception: print('')" "$CFG" "$1" 2>/dev/null; }
porta_occupata() { ss -ltn 2>/dev/null | grep -q ":$1\b"; }

URL="$(leggi_cfg directory_url)"; AUTH="$(leggi_cfg authority)"
# camuffamento e rumore di fondo della rete (dal codice di rete; reti senza: niente)
PROT=(); PROT_SITO=()
CAM="$(leggi_cfg camuffamento)"; RUM="$(leggi_cfg rumore)"
[[ "$CAM" =~ ^[0-9a-f]{64}$ ]] && PROT+=(--obfuscation "$CAM") && PROT_SITO+=(--obfuscation "$CAM")
[[ "$RUM" =~ ^[0-9]+(\.[0-9]+)?$ ]] && [ -n "${PROT[*]:-}" ] && PROT+=(--cover "$RUM")
if [ -n "${PROT[*]:-}" ] && [ "$(leggi_cfg forma)" = h3 ]; then     # forma HTTP/3: sembra un sito
  NOMEH3="$(leggi_cfg nome_h3)"; [[ "$NOMEH3" =~ ^[A-Za-z0-9.-]{1,253}$ ]] || NOMEH3=""
  PROT+=(--mimic ${NOMEH3:+--mimic-name "$NOMEH3"}); PROT_SITO+=(--mimic ${NOMEH3:+--mimic-name "$NOMEH3"})
elif [ -n "${PROT[*]:-}" ] && [ "$(leggi_cfg forma)" = webrtc ]; then   # forma videochiamata
  PROT+=(--webrtc); PROT_SITO+=(--webrtc)
fi
# Impostazioni avanzate (dalle Impostazioni grafiche): manopole del motore per variabile d'ambiente
FKB="$(leggi_cfg finestra_kb)"; [[ "$FKB" =~ ^[0-9]+$ ]] && [ "$FKB" -ge 16 ] && [ "$FKB" -le 4096 ] && export NYCTALUS_FINESTRA_KB="$FKB"
CONG="$(leggi_cfg congestione)"; [[ "$CONG" =~ ^(cubic|newreno)$ ]] && export NYCTALUS_CONGESTIONE="$CONG"
case "$(leggi_cfg tcp)" in sempre) export NYCTALUS_TCP=1 ;; mai) export NYCTALUS_TCP=0 ;; esac
[ "$(leggi_cfg riuso)" = no ] && export NYCTALUS_RIUSO=0
USC="$(leggi_cfg uscite)"; [[ "$USC" =~ ^[1-4]$ ]] && export NYCTALUS_USCITE="$USC"
COST="$(leggi_cfg costante)"; case "$COST" in uso) export NYCTALUS_COSTANTE=60 ;; sempre) export NYCTALUS_COSTANTE=60 NYCTALUS_COSTANTE_SEMPRE=1 ;; esac
# ponti nascosti della rete (righe segrete): al motore per variabile d'ambiente, non sulla riga
# di comando, che tutti gli utenti del computer vedono con «ps»
export NYCTALUS_PONTI="$(python3 -c "import json,sys
try:
    c=json.load(open(sys.argv[1])); print(';'.join((c.get('ponti') or {}).get(c.get('authority',''),[])))
except Exception: print('')" "$CFG" 2>/dev/null)"
if [ -n "$URL" ] && [ -n "$AUTH" ]; then SOCKS="127.0.0.1:1080"; else SOCKS="127.0.0.1:1086"; NYCTALUS_PONTI=""; fi

# Segreto della porta di controllo: solo la nostra finestra potrà usarla
# (passa per variabile d'ambiente, così non compare nell'elenco dei processi).
NYCTALUS_CHAT_TOKEN="$(python3 -c 'import secrets;print(secrets.token_hex(16))')"
export NYCTALUS_CHAT_TOKEN
( umask 077; echo "1091 $NYCTALUS_CHAT_TOKEN" > "$COLLEGAMENTO" )   # per le altre finestre (File)

# --- 1) motore (parte subito) e FINESTRA (subito) ----------------------------
echo "» avvio il motore della chat"
# rubrica e casella della rete (dal codice di rete): la posta per chi non è in linea
POSTA=()
if [ -n "$URL" ] && [ -n "$AUTH" ]; then
  for k in rubrica casella; do
    v="$(leggi_cfg "$k")"; [[ "$v" =~ ^[a-z2-7]{56}\.nyct$ ]] && POSTA+=("--$k" "$v")
  done
fi
"$BIN" chat --data "$DATI" --backend "$BACKEND" --control "$CONTROL" --socks "$SOCKS" ${POSTA[@]+"${POSTA[@]}"} > "$WORK/chat.log" 2>&1 & MIEI+=($!)
ADDR=""; for _ in $(seq 1 40); do ADDR=$(grep -oE '[a-z2-7]{56}\.nyct' "$WORK/chat.log" | head -1); [ -n "$ADDR" ] && break; sleep 0.1; done
[ -n "$ADDR" ] && echo "  il tuo Contact ID:  $ADDR"

if [ "$INTERFACCIA" = nessuna ]; then
  MOTORE="${MIEI[0]}"
  echo "» motore acceso senza finestre: usa nyctalus-cli da un altro terminale (Ctrl-C qui lo spegne)"
else
  echo "» apro la finestra (la rete finisce di salire in sottofondo)"
  NYCTALUS_CHAT_PORT=1091 python3 -m "$INTERFACCIA" > "$WORK/ui.log" 2>&1 & UIPID=$!
  echo "$UIPID" > "$FINESTRE/$UIPID"
fi

# --- 2) rete + servizio .nyct, IN SOTTOFONDO ---------------------------------
avvia_rete() {
  if [ -n "$URL" ] && [ -n "$AUTH" ]; then
    echo "» (sottofondo) uso la rete pubblica configurata"
    if command -v curl >/dev/null 2>&1; then curl -fsS "$URL" -o "$WORK/dir.txt" || { echo "✗ download elenco fallito"; return; }
    else python3 -c "import urllib.request,sys;open(sys.argv[2],'wb').write(urllib.request.urlopen(sys.argv[1],timeout=25).read())" "$URL" "$WORK/dir.txt" || { echo "✗ download elenco fallito"; return; }; fi
    if ! porta_occupata 1080; then
      "$BIN" start --directory "$WORK/dir.txt" --authority "$AUTH" --socks 127.0.0.1:1080 ${PROT[@]+"${PROT[@]}"} > "$WORK/daemon.log" 2>&1 & MIEI+=($!)
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
  # l'identità la dà il motore della chat (i dati possono essere cifrati: niente file in chiaro)
  "$BIN" site --backend "$BACKEND" --identity-chat "$CONTROL" --directory "$DIRFILE" --authority "$AUTHORITY" --hops 3 ${PROT_SITO[@]+"${PROT_SITO[@]}"} > "$WORK/site.log" 2>&1 & MIEI+=($!)
  echo "» (sottofondo) rete pronta — puoi scrivere"
}
# NB: in primo piano nella shell principale (la finestra è GIÀ aperta sopra),
# così i PID dei nodi/circuito/servizio finiscono in MIEI e il cleanup li chiude.
avvia_rete

# --- 3) resta vivo finché la finestra è aperta -------------------------------
if [ "$INTERFACCIA" = nessuna ]; then
  echo "» pronto. Il tuo Contact ID: ${ADDR:-?}"
  wait "$MOTORE"; exit 0
fi
wait "$UIPID"; rm -f "$FINESTRE/$UIPID"
# chiusa la finestra che ha acceso il motore: si aspettano le altre ancora aperte (es. File
# con un trasferimento in corso), poi si spegne tutto
while :; do
  vive=0
  for f in "$FINESTRE"/*; do
    [ -e "$f" ] || continue
    if kill -0 "$(basename "$f")" 2>/dev/null; then vive=1; else rm -f "$f"; fi
  done
  [ "$vive" = 1 ] || break
  sleep 2
done
