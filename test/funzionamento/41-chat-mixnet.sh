#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# CHAT DA MIXNET (scaletta punto 4): i flussi verso un `.nyct` chiesti con il nome utente SOCKS
# «mix» (le buste della modalità riservata) passano da circuiti in cui ogni nodo TRATTIENE le
# celle per un ritardo casuale scelto dal mittente (esponenziale, come Loopix) e scritto nello
# strato Sphinx di quel nodo: buste di utenti diversi escono dai nodi rimescolate nel tempo. Il
# flusso resta integro (ordine conservato). Gli altri flussi restano immediati. Tutto in locale.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · chat da mixnet (ritardi nei nodi)"
assicura_binario
B=23020
K=$(python3 -c "import os;print(os.urandom(32).hex())")
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
righe=""
for i in 0 1 2 3 4; do
  p=$((B+i)); NYCTALUS_DIAGNOSI=1 "$BIN" node --listen 127.0.0.1:$p --data "$TMP/n$p" --obfuscation "$K" > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
done
for i in 0 1 2 3 4; do
  p=$((B+i)); L=$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+")
  [ -z "$L" ] && { fail "nodo $p non partito"; exit 1; }
  righe="$righe${righe:+;}$L"
done
DIR="$TMP/dir.txt"
"$BIN" directory --authority-secret "$SEC" --entries "$righe" > "$DIR" 2>/dev/null
mkdir -p "$TMP/www"; echo "busta di prova" > "$TMP/www/index.html"; head -c 200000 /dev/urandom | base64 > "$TMP/www/lunga.txt"
ADDR=$("$BIN" address --identity "$TMP/servizio" | grep -oE '[a-z2-7]{56}\.nyct' | tail -1)
"$BIN" site --dir "$TMP/www" --identity "$TMP/servizio" --directory "$DIR" --authority "$PUB" --obfuscation "$K" --hops 3 > "$TMP/s.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/s.log" "online" 40 > /dev/null || { fail "sito .nyct non online: $(tail -3 "$TMP/s.log")"; exit 1; }
export NYCTALUS_GUARDIE="$TMP/guardie" NYCTALUS_MIX_MS=1500
"$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$K" --socks 127.0.0.1:$((B+10)) --control 127.0.0.1:$((B+11)) --hops 3 > "$TMP/c.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/c.log" "SOCKS5 proxy" 25 > /dev/null

# 1) flusso normale verso il .nyct: immediato
T0=$(date +%s.%N)
R=$(curl -s -m 60 --socks5-hostname 127.0.0.1:$((B+10)) "http://$ADDR/" 2>&1)
T_NORMALE=$(python3 -c "import sys;print(round($(date +%s.%N)-$T0,2))")
[ "$R" = "busta di prova" ] && pass "flusso normale verso il .nyct: arriva (${T_NORMALE} s)" || fail "normale: $R"
! grep -q "circuito mix" "$TMP"/n*.log && pass "senza «mix» nessun nodo trattiene le celle" || fail "un nodo ha trattenuto un flusso normale"

# 2) lo stesso con il nome utente SOCKS «mix»: i nodi trattengono le celle
T0=$(date +%s.%N)
R=$(curl -s -m 120 --socks5-hostname 127.0.0.1:$((B+10)) --proxy-user mix:x "http://$ADDR/" 2>&1)
T_MIX=$(python3 -c "import sys;print(round($(date +%s.%N)-$T0,2))")
[ "$R" = "busta di prova" ] && pass "flusso «mix»: la busta arriva integra (${T_MIX} s)" || fail "mix: $R · $(tail -3 "$TMP/c.log")"
N=$(grep -h "circuito mix" "$TMP"/n*.log | wc -l)
[ "$N" -ge 2 ] && pass "nei nodi: $N salti hanno trattenuto il circuito (ritardi: $(grep -ohE 'trattenuta [0-9]+ ms' "$TMP"/n*.log | awk '{print $2}' | paste -sd, -) ms)" || fail "nessun nodo ha applicato il ritardo"
python3 -c "import sys;sys.exit(0 if $T_MIX > $T_NORMALE + 1.0 else 1)" && pass "il ritardo si vede: ${T_MIX} s contro ${T_NORMALE} s (media 1,5 s per salto)" || fail "mix non più lento: ${T_MIX} vs ${T_NORMALE}"
# ritardi diversi a ogni busta (scelti a caso dal mittente)
curl -s -m 120 --socks5-hostname 127.0.0.1:$((B+10)) --proxy-user mix:x "http://$ADDR/" > /dev/null 2>&1
D=$(grep -ohE 'trattenuta [0-9]+ ms' "$TMP"/n*.log | awk '{print $2}' | sort -u | wc -l)
[ "$D" -ge 3 ] && pass "ritardi diversi a ogni salto e a ogni busta ($D valori)" || fail "ritardi tutti uguali"

# 3) un contenuto lungo resta integro e in ordine attraverso le code dei nodi
curl -s -m 180 --socks5-hostname 127.0.0.1:$((B+10)) --proxy-user mix:x "http://$ADDR/lunga.txt" -o "$TMP/lunga.txt"
cmp -s "$TMP/lunga.txt" "$TMP/www/lunga.txt" && pass "270 KB attraverso le code dei nodi: integri e nell'ordine giusto" || fail "contenuto lungo diverso ($(stat -c %s "$TMP/lunga.txt" 2>/dev/null) byte)"

# 4) la chat: le buste della modalità riservata usano «mix», quelle normali no (nel sorgente)
grep -q 'consegna_busta(self.socks, &casella, addr, &busta, true)' "$ROOT/motore/nyctalus/src/chat.rs" \
  && grep -q 'mix.then_some("mix")' "$ROOT/motore/nyctalus/src/chat.rs" \
  && pass "chat: le buste della modalità riservata (vere e finte) partono come «mix»" || fail "chat senza mix"
[ "$KO" -eq 0 ]
