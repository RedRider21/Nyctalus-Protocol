#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# RIPRESA DAI CIRCUITI CADUTI (scaletta punto 5). Uno scaricamento passa da più circuiti (ragnatela
# conflux) con la stessa guardia e la stessa uscita. A metà si SPEGNE un nodo intermedio: le unità
# che erano in viaggio su quel circuito vanno perse. Prima la ricomposizione restava ad aspettarle
# per sempre (scaricamento bloccato); ora ogni capo tiene le unità non ancora confermate, le
# rispedisce sugli altri circuiti e ricostruisce un circuito nuovo. Tutto in locale (127.0.0.1).
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · ripresa dai circuiti caduti"
assicura_binario
B=23320
K=$(python3 -c "import os;print(os.urandom(32).hex())")
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
righe=""; declare -A PIDNODO
for i in 0 1 2 3 4 5 6; do
  p=$((B+i)); "$BIN" node --listen 127.0.0.1:$p --data "$TMP/n$p" --obfuscation "$K" --max-rate 8 > "$TMP/n$p.log" 2>&1 & PIDNODO[$p]=$!; PIDS+=($!)
done
for i in 0 1 2 3 4 5 6; do
  p=$((B+i)); L=$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+")
  [ -z "$L" ] && { fail "nodo $p non partito"; exit 1; }
  righe="$righe${righe:+;}$L"
done
DIR="$TMP/dir.txt"
"$BIN" directory --authority-secret "$SEC" --entries "$righe" > "$DIR" 2>/dev/null
mkdir -p "$TMP/www"; head -c 6000000 /dev/urandom > "$TMP/www/grande.bin"
python3 -m http.server $((B+20)) --bind 127.0.0.1 --directory "$TMP/www" > /dev/null 2>&1 & PIDS+=($!)
export NYCTALUS_GUARDIE="$TMP/guardie"
"$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$K" --circuits 3 --hops 3 \
    --socks 127.0.0.1:$((B+10)) --control 127.0.0.1:$((B+11)) > "$TMP/c.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/c.log" "SOCKS5 proxy" 25 > /dev/null
# un intermedio del circuito 0 (non la guardia né l'uscita, che sono comuni a tutti)
MEZZO=$(grep -oE "circuit 0: [0-9.:]+ → [0-9.:]+" "$TMP/c.log" | awk '{print $NF}')
PM=${MEZZO##*:}
[ -n "$PM" ] && [ -n "${PIDNODO[$PM]:-}" ] || { fail "intermedio del circuito 0 non trovato: $(grep circuit "$TMP/c.log")"; exit 1; }
curl -s -m 120 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/grande.bin -o "$TMP/giu.bin" & CURL=$!
sleep 2
kill -9 "${PIDNODO[$PM]}" 2>/dev/null
pass "a metà scaricamento si spegne di colpo un intermedio ($MEZZO)"
wait $CURL
cmp -s "$TMP/giu.bin" "$TMP/www/grande.bin" && pass "lo scaricamento (6 MB) arriva comunque integro" \
  || fail "scaricamento rotto: $(stat -c %s "$TMP/giu.bin" 2>/dev/null) byte su 6000000"
grep -q "circuit lost" "$TMP/c.log" && pass "il client si accorge del circuito caduto e rispedisce le unità non confermate" || fail "nessun segno della caduta nel registro: $(tail -4 "$TMP/c.log")"
grep -q "replacement circuit" "$TMP/c.log" && pass "e costruisce un circuito nuovo al suo posto" || fail "nessun circuito nuovo: $(tail -4 "$TMP/c.log")"
# dopo la caduta la rete funziona ancora per i flussi nuovi
echo "ancora qui" > "$TMP/www/index.html"
R=$(curl -s -m 30 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R" = "ancora qui" ] && pass "dopo la caduta si naviga ancora (flusso nuovo)" || fail "dopo la caduta: $R"
[ "$KO" -eq 0 ]
