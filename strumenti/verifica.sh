#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Verifiche automatiche di sicurezza (vedi SICUREZZA.md). Controlla proprietà
# concrete testabili in locale e riporta PASS/FAIL. NON sostituisce un audit
# indipendente: le conclusioni forti sull'anonimato richiedono misure su rete
# reale e revisione di crittografi terzi (vedi SICUREZZA.md §3).
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/nyctalus"
W="$(mktemp -d /tmp/nyctalus-verifica.XXXXXX)"
PIDS=()
OK=0; KO=0

pass() { printf '  \033[1;32mPASS\033[0m  %s\n' "$*"; OK=$((OK+1)); }
fail() { printf '  \033[1;31mFAIL\033[0m  %s\n' "$*"; KO=$((KO+1)); }
cleanup() { for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null; done; rm -rf "$W"; }
trap cleanup EXIT INT TERM

[ -x "$BIN" ] || { echo "Compilo il binario…"; ( cd "$ROOT" && cargo build --release -p nyctalus ) || exit 1; }
cd "$W" || exit 1

echo "== 1. Correttezza del protocollo (cargo test) =="
if ( cd "$ROOT" && cargo test --quiet >/tmp/nyctalus-test.log 2>&1 ); then
  n=$(grep -oE "test result: ok\. [0-9]+" /tmp/nyctalus-test.log | grep -oE "[0-9]+" | paste -sd+ | bc 2>/dev/null)
  pass "tutti i test automatici superati (${n:-?} test)"
else
  fail "cargo test ha segnalato errori (vedi /tmp/nyctalus-test.log)"
fi

echo "== 2. Autorità, directory firmata =="
"$BIN" authority > auth.txt 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' auth.txt | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' auth.txt | awk '{print $2}')
"$BIN" authority > auth2.txt 2>&1
PUB2=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' auth2.txt | awk '{print $2}')
# un nodo per avere una riga d'elenco valida
"$BIN" node --listen 127.0.0.1:4950 > n.log 2>&1 & PIDS+=($!)
for _ in $(seq 1 40); do RIGA=$(grep -oE "127\.0\.0\.1:4950,[0-9a-f]+,[0-9a-f]+" n.log | head -1); [ -n "$RIGA" ] && break; sleep 0.25; done
"$BIN" directory --authority-secret "$SEC" --entries "$RIGA" > dir.txt 2>/dev/null
if "$BIN" nodes --directory dir.txt --authority "$PUB" >/dev/null 2>&1; then
  pass "una directory firmata correttamente viene accettata"
else
  fail "una directory valida è stata rifiutata"
fi

echo "== 3. Directory manomessa → rifiutata =="
cp dir.txt dir_bad.txt
# cambia un carattere esadecimale nella prima riga 'v ...'
perl -0pi -e 's/^(v \S+ [0-9a-f])([0-9a-f])/$1 . (($2 eq "a")?"b":"a")/me' dir_bad.txt
if "$BIN" nodes --directory dir_bad.txt --authority "$PUB" >/dev/null 2>&1; then
  fail "una directory MANOMESSA è stata accettata (grave)"
else
  pass "una directory manomessa viene rifiutata (firma invalida)"
fi

echo "== 4. Autorità sbagliata → rifiutata =="
if "$BIN" nodes --directory dir.txt --authority "$PUB2" >/dev/null 2>&1; then
  fail "una directory firmata da un'ALTRA autorità è stata accettata (grave)"
else
  pass "una directory firmata da un'altra autorità viene rifiutata"
fi

echo "== 5. Camuffamento del trasporto obbligatorio =="
KEY="00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
"$BIN" node --listen 127.0.0.1:4951 --obfuscation "$KEY" > no.log 2>&1 & PIDS+=($!)
for _ in $(seq 1 40); do R2=$(grep -oE "127\.0\.0\.1:4951,[0-9a-f]+,[0-9a-f]+" no.log | head -1); [ -n "$R2" ] && break; sleep 0.25; done
"$BIN" directory --authority-secret "$SEC" --entries "$R2" > diro.txt 2>/dev/null
# con la chiave giusta: raggiungibile
ON=$(timeout 8 "$BIN" nodes --directory diro.txt --authority "$PUB" --probe --obfuscation "$KEY" --timeout 3 2>/dev/null | grep -c "online")
# senza chiave (traffico = rumore per il nodo): NON raggiungibile
OFF=$(timeout 8 "$BIN" nodes --directory diro.txt --authority "$PUB" --probe --timeout 3 2>/dev/null | grep -c "online")
if [ "$ON" -ge 1 ] && [ "$OFF" -eq 0 ]; then
  pass "con la chiave di rete il nodo è raggiungibile, senza no (offuscamento attivo)"
else
  fail "offuscamento: atteso online con chiave / offline senza (ON=$ON OFF=$OFF)"
fi

echo
echo "Risultato: $OK PASS, $KO FAIL."
echo "Nota: queste prove verificano che le difese ci sono e funzionano."
echo "L'anonimato forte richiede rete reale + audit indipendente — vedi SICUREZZA.md."
[ "$KO" -eq 0 ]
