#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Demo LOCALE della chat .nyct: avvia una piccola rete di prova (4 nodi) e DUE
# identità ("Alice" e "Bob") complete (motore + servizio .nyct), poi apre DUE
# finestre di chat. Aggiungi l'una l'indirizzo dell'altra (il bottone « + ») e
# scrivetevi: tutto resta sulla tua macchina. Chiudendo (o con Ctrl-C) si ferma
# e pulisce tutto.
#
#   ./strumenti/demo-chat.sh
#
# Richiede: il binario `nyctalus` (lo compila se manca) + python3 + PyGObject
# (GTK3). È solo una rete di prova locale: NON dà anonimato reale.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/nyctalus"
WORK="$(mktemp -d /tmp/nyctalus-chatdemo.XXXXXX)"
PIDS=()
say() { printf '\033[1;36m» %s\033[0m\n' "$*"; }
err() { printf '\033[1;31m✗ %s\033[0m\n' "$*" >&2; }
cleanup() { say "chiusura, pulizia…"; for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null; done; rm -rf "$WORK"; }
trap cleanup EXIT INT TERM

[ -x "$BIN" ] || { say "compilo nyctalus (release)…"; ( cd "$ROOT" && cargo build --release -p nyctalus ) || { err "compilazione fallita"; exit 1; }; }
if ! python3 -c 'import gi; gi.require_version("Gtk","3.0")' 2>/dev/null; then
  err "PyGObject/GTK3 non disponibile."; echo "   sudo apt install python3-gi gir1.2-gtk-3.0"; exit 1
fi
if [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then err "nessun display grafico."; exit 1; fi
for porta in 1080 1082 1083 1091 1092; do
  ss -ltn 2>/dev/null | grep -q ":$porta\b" && { err "la porta $porta è occupata: chiudi il processo e riprova."; exit 1; }
done
cd "$WORK"

attendi_addr() { local f="$1" a=""; for _ in $(seq 1 50); do a=$(grep -oE '[a-z2-7]{56}\.nyct' "$f" | head -1); [ -n "$a" ] && break; sleep 0.2; done; echo "$a"; }

say "creo l'autorità e 4 nodi di prova…"
"$BIN" authority > auth.txt 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' auth.txt | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' auth.txt | awk '{print $2}')
[ -z "$SEC" ] && { err "autorità KO"; cat auth.txt; exit 1; }
NODI=(5830 5831 5832 5833); LINES=""
for p in "${NODI[@]}"; do "$BIN" node --listen 127.0.0.1:"$p" > "n$p.log" 2>&1 & PIDS+=($!); done
for p in "${NODI[@]}"; do
  L=""; for _ in $(seq 1 60); do L=$(grep -oE "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+" "n$p.log" | head -1); [ -n "$L" ] && break; sleep 0.25; done
  [ -z "$L" ] && { err "nodo $p KO"; cat "n$p.log"; exit 1; }
  LINES="$LINES${LINES:+;}$L"
done
"$BIN" directory --authority-secret "$SEC" --entries "$LINES" > dir.txt 2>dir.err
grep -q '^firma ' dir.txt || { err "directory KO"; cat dir.err; exit 1; }

say "avvio il proxy SOCKS condiviso (circuit, 3 salti)…"
"$BIN" circuit --directory dir.txt --authority "$PUB" --socks 127.0.0.1:1080 --hops 3 --control 127.0.0.1:1591 > daemon.log 2>&1 & PIDS+=($!)
sleep 3

avvia_identita() { # nome backend control  (NIENTE sostituzione di comando con processi in background!)
  local nome="$1" be="$2" ctl="$3"
  mkdir -p "$WORK/$nome"
  "$BIN" chat --data "$WORK/$nome" --backend 127.0.0.1:"$be" --control 127.0.0.1:"$ctl" --socks 127.0.0.1:1080 > "$nome.chat.log" 2>&1 & PIDS+=($!)
}

say "avvio Alice e Bob (motore chat)…"
avvia_identita alice 1082 1091
avvia_identita bob   1083 1092
ALICE=$(attendi_addr alice.chat.log); BOB=$(attendi_addr bob.chat.log)
[ -z "$ALICE" ] && { err "Alice KO"; cat alice.chat.log; exit 1; }
[ -z "$BOB" ]   && { err "Bob KO";   cat bob.chat.log;   exit 1; }

say "pubblico i due servizi .nyct…"
"$BIN" site --backend 127.0.0.1:1082 --identity "$WORK/alice/identita" --directory dir.txt --authority "$PUB" --hops 3 > alice.site.log 2>&1 & PIDS+=($!)
"$BIN" site --backend 127.0.0.1:1083 --identity "$WORK/bob/identita"   --directory dir.txt --authority "$PUB" --hops 3 > bob.site.log   2>&1 & PIDS+=($!)
sleep 4

echo
say "Pronto. Si aprono DUE finestre di chat:"
echo "   Alice:  $ALICE"
echo "   Bob:    $BOB"
echo "   In ciascuna premi « + », incolla l'indirizzo dell'ALTRA e scrivetevi."
echo "   (rete di prova LOCALE: nessun anonimato reale)"
echo

export PYTHONPATH="$ROOT/pc/chat:$ROOT/pc/browser"
NYCTALUS_CHAT_PORT=1091 python3 -m nyctalus_chat > "$WORK/ui-alice.log" 2>&1 & PIDS+=($!)
NYCTALUS_CHAT_PORT=1092 python3 -m nyctalus_chat > "$WORK/ui-bob.log"   2>&1 & PIDS+=($!)

# aspetta che una delle due finestre venga chiusa
wait -n 2>/dev/null || sleep infinity
