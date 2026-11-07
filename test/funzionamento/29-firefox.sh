#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Nyctalus Firefox (PIANO-ANONIMATO, punto J, fase 1): profilo separato e blindato per il Firefox
# già installato. Con Firefox vero in modalità invisibile (headless) e una rete di prova: la pagina
# arriva attraverso la rete, dentro la pagina niente WebRTC, ora UTC, lingua inglese, niente WebGL;
# a rete spenta non esce NIENTE (mai diretto); senza rete il comando lo dice. Tutto in 127.0.0.1.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · Nyctalus Firefox (profilo blindato)"
FF=$(command -v firefox-esr || command -v firefox || true)
[ -n "$FF" ] || { salta "Firefox non installato"; exit 0; }
assicura_binario
A="$ROOT/strumenti/avvia-firefox.sh"
P="$TMP/ff/profilo"
NYCTALUS_FIREFOX_PROFILO="$P" bash "$A" --prepara 21980 > /dev/null
grep -q 'network.proxy.socks_port", 21980' "$P/user.js" && grep -q 'socks_remote_dns", true' "$P/user.js" \
  && grep -q 'failover_direct", false' "$P/user.js" && grep -q 'resistFingerprinting", true' "$P/user.js" \
  && grep -q 'peerconnection.enabled", false' "$P/user.js" && [ "$(stat -c %a "$P")" = 700 ] \
  && pass "profilo separato (cartella 700): proxy Nyctalus anche per i nomi, mai diretto, impronta uniforme, WebRTC spento" || fail "profilo: $(ls -la "$P")"

rete_di_prova 21900
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:21980 --hops 3 --control 127.0.0.1:21990 > "$TMP/circ.log" 2>&1 & CIRC=$!; PIDS+=($CIRC)
python3 "$STRUMENTI/server_impronta.py" 21970 "$TMP/visite.txt" & PIDS+=($!)
for _ in $(seq 1 40); do ss -ltn | grep -q ":21980\b" && break; sleep 0.3; done
sleep 3
apri() { timeout 90 "$FF" --headless --no-remote --new-instance --profile "$P" --screenshot "$TMP/foto-$2.png" "$1" > "$TMP/ff-$2.log" 2>&1; }
for i in 1 2 3; do apri "http://127.0.0.1:21970/pagina" $i; grep -q "^/impronta" "$TMP/visite.txt" 2>/dev/null && break; sleep 3; done
grep -q "^/pagina" "$TMP/visite.txt" && pass "Firefox apre la pagina attraverso la rete Nyctalus (3 salti)" || fail "pagina non arrivata: $(tail -3 "$TMP/ff-1.log")"
IMP=$(grep "^/impronta" "$TMP/visite.txt" | tail -1)
echo "$IMP" | grep -q "rtc=undefined" && pass "dentro la pagina: WebRTC non esiste (non può rivelare l'IP)" || fail "WebRTC: $IMP"
echo "$IMP" | grep -qE "tz=(UTC|Atlantic%2FReykjavik|Etc%2FUTC)" && pass "fuso orario uniforme (UTC), non quello del PC" || fail "fuso orario: $IMP"
echo "$IMP" | grep -q "lang=en-US" && pass "lingua uniforme (en-US)" || fail "lingua: $IMP"
echo "$IMP" | grep -q "webgl=false" && pass "WebGL spento (niente impronta della scheda video)" || fail "WebGL: $IMP"

# rete spenta: Firefox non deve uscire diretto
kill "$CIRC" 2>/dev/null; sleep 1
apri "http://127.0.0.1:21970/a-rete-spenta" spenta
! grep -q "a-rete-spenta" "$TMP/visite.txt" && pass "a rete spenta non esce NIENTE: nessun collegamento diretto (fail-closed)" || fail "è uscito diretto senza la rete!"

# nessuna rete configurata né accesa: lo dice e non apre Firefox (in uno spazio di rete isolato)
if unshare -rn true 2>/dev/null; then
  mkdir -p "$TMP/vuota"
  OUT=$(HOME="$TMP/vuota" NYCTALUS_FIREFOX=/bin/false unshare -rn bash -c "ip link set lo up; bash '$A'" 2>&1 < /dev/null)
  echo "$OUT" | grep -q "Nessuna rete Nyctalus" && pass "senza rete: «apri prima la chat, o importa il codice», e Firefox non parte" || fail "senza rete: $OUT"
else
  salta "spazi di rete isolati non disponibili"
fi
[ "$KO" -eq 0 ]
