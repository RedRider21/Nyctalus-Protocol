#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# CAPACITÀ (scaletta punto 3b): USCITE DIVERSE. Con `--exits N` (o NYCTALUS_USCITE) il client
# apre N fasci conflux dalla stessa guardia, ognuno con la sua uscita. Ogni sito resta sulla sua
# uscita (il sito non vede l'utente da più indirizzi insieme); un sito nuovo va al fascio più
# libero, così scaricamenti paralleli da siti diversi non si contendono la stessa uscita.
# Tutto in locale (127.0.0.1); la diagnostica del client dice quale uscita ha preso ogni flusso.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · uscite diverse per siti diversi (--exits)"
assicura_binario
B=22920
K=$(python3 -c "import os;print(os.urandom(32).hex())")
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
righe=""
for i in 0 1 2 3 4; do
  p=$((B+i)); "$BIN" node --listen 127.0.0.1:$p --data "$TMP/n$p" --obfuscation "$K" > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
done
for i in 0 1 2 3 4; do
  p=$((B+i)); L=$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+")
  [ -z "$L" ] && { fail "nodo $p non partito"; exit 1; }
  righe="$righe${righe:+;}$L"
done
DIR="$TMP/dir.txt"
"$BIN" directory --authority-secret "$SEC" --entries "$righe" > "$DIR" 2>/dev/null
mkdir -p "$TMP/www"; echo "ciao dal sito" > "$TMP/www/index.html"; head -c 3000000 /dev/urandom > "$TMP/www/grande.bin"
python3 -m http.server $((B+20)) --bind 127.0.0.1 --directory "$TMP/www" > /dev/null 2>&1 & PIDS+=($!)

export NYCTALUS_GUARDIE="$TMP/guardie"
NYCTALUS_DIAGNOSI=1 "$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$K" --exits 2 \
    --socks 127.0.0.1:$((B+10)) --control 127.0.0.1:$((B+11)) --hops 3 > "$TMP/c.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/c.log" "SOCKS5 proxy" 25 > /dev/null
grep -q "2 exits" "$TMP/c.log" && pass "il client apre 2 fasci, ognuno con la sua uscita" || fail "fasci: $(head -8 "$TMP/c.log")"
U0=$(grep -oE "exit 0, circuit 0: .*" "$TMP/c.log" | awk -F' → ' '{print $NF}')
U1=$(grep -oE "exit 1, circuit 0: .*" "$TMP/c.log" | awk -F' → ' '{print $NF}')
G0=$(grep -oE "exit 0, circuit 0: [0-9.:]+" "$TMP/c.log" | awk '{print $NF}'); G1=$(grep -oE "exit 1, circuit 0: [0-9.:]+" "$TMP/c.log" | awk '{print $NF}')
[ -n "$U0" ] && [ "$U0" != "$U1" ] && [ "$G0" = "$G1" ] && [ "$U0" != "$G0" ] && [ "$U1" != "$G0" ] \
  && pass "uscite diverse ($U0, $U1), stessa guardia ($G0), nessuna uscita uguale alla guardia" || fail "percorsi: $(grep circuit "$TMP/c.log")"

# 1) un primo sito (127.0.0.1) con uno scaricamento lento, che resta aperto
curl -s -m 60 --limit-rate 100k --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/grande.bin -o "$TMP/lento.bin" & LENTO=$!
sleep 3
# 2) un altro sito (localhost) mentre il primo scarica: va all'ALTRA uscita
R=$(curl -s -m 20 --socks5-hostname 127.0.0.1:$((B+10)) http://localhost:$((B+20))/ 2>&1)
[ "$R" = "ciao dal sito" ] && pass "si naviga su un secondo sito mentre il primo scarica" || fail "secondo sito: $R"
# 3) di nuovo il primo sito: STESSA uscita del primo flusso
R=$(curl -s -m 20 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R" = "ciao dal sito" ] || fail "terzo flusso: $R"
sleep 1
F=$(grep -oE "flusso [0-9]+ → uscita [0-9]+" "$TMP/c.log")
F0=$(echo "$F" | awk '$2==0{print $5}'); F1=$(echo "$F" | awk '$2==1{print $5}'); F2=$(echo "$F" | awk '$2==2{print $5}')
[ -n "$F0" ] && [ -n "$F1" ] && [ "$F0" != "$F1" ] && pass "il sito nuovo va all'uscita libera (flusso 0 → uscita $F0, flusso 1 → uscita $F1)" || fail "scelta: $F"
[ -n "$F2" ] && [ "$F2" = "$F0" ] && pass "lo stesso sito torna sempre dalla stessa uscita (flusso 2 → uscita $F2)" || fail "stesso sito: $F"
wait "$LENTO" 2>/dev/null
cmp -s "$TMP/lento.bin" "$TMP/www/grande.bin" && pass "lo scaricamento lungo (3 MB) arriva integro" || fail "file grande diverso ($(stat -c %s "$TMP/lento.bin" 2>/dev/null) byte)"

# 4) di serie una sola uscita, come prima
"$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$K" \
    --socks 127.0.0.1:$((B+12)) --control 127.0.0.1:$((B+13)) --hops 3 > "$TMP/c1.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/c1.log" "SOCKS5 proxy" 25 > /dev/null
grep -q "shared guard and exit" "$TMP/c1.log" && ! grep -q "exit 1," "$TMP/c1.log" && pass "senza --exits: un'uscita sola (comportamento di sempre)" || fail "di serie: $(head -6 "$TMP/c1.log")"
R=$(curl -s -m 20 --socks5-hostname 127.0.0.1:$((B+12)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R" = "ciao dal sito" ] && pass "di serie si naviga come prima" || fail "di serie: $R"
# 5) valori fuori scala rifiutati
"$BIN" circuit --directory "$DIR" --authority "$PUB" --exits 9 > "$TMP/c9.log" 2>&1
grep -q "a number 1-4" "$TMP/c9.log" && pass "--exits 9: rifiutato (da 1 a 4)" || fail "--exits 9: $(tail -2 "$TMP/c9.log")"
[ "$KO" -eq 0 ]
