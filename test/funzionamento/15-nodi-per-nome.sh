#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Nodi annunciati con un NOME invece che con l'IP (come chi ha l'IP dinamico e usa No-IP,
# DuckDNS…): l'elenco firmato contiene «nome:porta», i client lo risolvono, e i nodi intermedi
# trovano il salto successivo per chiave nell'elenco e risolvono il nome al momento. Qui il nome
# è «localhost», che il sistema risolve come farebbe con un nome dinamico.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · nodi con un nome (IP dinamico)"
assicura_binario
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
DIR="$TMP/dir.txt"; : > "$DIR"; righe=""
for p in 20701 20702 20703 20704; do
  mkdir -p "$TMP/n$p"
  "$BIN" node --listen 127.0.0.1:$p --data "$TMP/n$p" --announce localhost:$p --directory "$DIR" --authority "$PUB" > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
done
for p in 20701 20702 20703 20704; do
  L=$(attendi_regex "$TMP/n$p.log" "localhost:$p,[0-9a-f]+,[0-9a-f]+" 15)
  [ -z "$L" ] && { fail "nodo $p: la riga non usa il nome ($(tail -2 "$TMP/n$p.log"))"; exit 1; }
  righe="$righe${righe:+;}$L"
done
pass "i nodi si annunciano con il nome (--announce localhost:PORTA)"
"$BIN" directory --authority-secret "$SEC" --entries "$righe" > "$DIR" 2>/dev/null
grep -c "^v localhost:" "$DIR" | grep -q 4 && pass "l'elenco firmato contiene i nomi (non gli IP)" || fail "elenco: $(head -3 "$DIR")"
"$BIN" nodes --directory "$DIR" --authority "$PUB" --probe 2>&1 | grep -c "online" | grep -q 4 \
  && pass "nyctalus nodes --probe: i 4 nodi col nome risultano online" || fail "sonda: $("$BIN" nodes --directory "$DIR" --authority "$PUB" --probe 2>&1 | tail -4)"
mkdir -p "$TMP/sito"; echo "<h1>arrivato per nome</h1>" > "$TMP/sito/index.html"
"$BIN" site --dir "$TMP/sito" --identity "$TMP/sito.id" --directory "$DIR" --authority "$PUB" --hops 3 > "$TMP/site.log" 2>&1 & PIDS+=($!)
ADDR=$(attendi_regex "$TMP/site.log" '[a-z2-7]{56}\.nyct' 30)
sleep 3
timeout 90 "$BIN" browse "$ADDR" --directory "$DIR" --authority "$PUB" --hops 3 2>"$TMP/b.err" | grep -q "arrivato per nome" \
  && pass "sito .nyct visitato con circuiti a 3 salti fra nodi annunciati per nome" || fail "visita: $(tail -3 "$TMP/b.err")"
# un nodo il cui nome non si risolve viene saltato, la rete funziona con gli altri
"$BIN" directory --authority-secret "$SEC" --entries "$righe;nodo-che-non-esiste.invalid:20799,$(printf '1%.0s' $(seq 64)),$(printf '2%.0s' $(seq 64))" > "$TMP/dir2.txt" 2>/dev/null
"$BIN" nodes --directory "$TMP/dir2.txt" --authority "$PUB" --probe > "$TMP/n2.txt" 2>&1
grep -c "online" "$TMP/n2.txt" | grep -q 4 && pass "un nome che non si risolve viene saltato (gli altri nodi restano usabili)" || fail "nome irrisolvibile: $(tail -3 "$TMP/n2.txt")"
[ "$KO" -eq 0 ]
