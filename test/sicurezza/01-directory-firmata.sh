#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# L'elenco dei nodi è firmato: valido → accettato; manomesso o firmato da
# un'altra autorità → rifiutato.
. "$(dirname "$0")/../lib.sh"
titolo "Sicurezza · elenco dei nodi firmato"
assicura_binario
cd "$TMP" || exit 1
"$BIN" authority > a1.txt 2>&1; "$BIN" authority > a2.txt 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' a1.txt | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' a1.txt | awk '{print $2}')
PUB2=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' a2.txt | awk '{print $2}')
"$BIN" node --listen 127.0.0.1:19050 > n.log 2>&1 & PIDS+=($!)
R=$(attendi_regex n.log "127\.0\.0\.1:19050,[0-9a-f]+,[0-9a-f]+")
"$BIN" directory --authority-secret "$SEC" --entries "$R" > dir.txt 2>/dev/null
verifica "elenco firmato correttamente → accettato" "$BIN" nodes --directory dir.txt --authority "$PUB"
cp dir.txt manomesso.txt
perl -0pi -e 's/^(v \S+ [0-9a-f])([0-9a-f])/$1 . (($2 eq "a")?"b":"a")/me' manomesso.txt
if "$BIN" nodes --directory manomesso.txt --authority "$PUB" >/dev/null 2>&1; then fail "elenco MANOMESSO accettato (grave)"; else pass "elenco manomesso → rifiutato"; fi
if "$BIN" nodes --directory dir.txt --authority "$PUB2" >/dev/null 2>&1; then fail "elenco di un'ALTRA autorità accettato (grave)"; else pass "elenco firmato da un'altra autorità → rifiutato"; fi
[ "$KO" -eq 0 ]
