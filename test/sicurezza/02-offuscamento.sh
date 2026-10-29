#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Con la chiave di rete il trasporto è camuffato: senza la chiave il nodo non
# risponde (il traffico è rumore per lui).
. "$(dirname "$0")/../lib.sh"
titolo "Sicurezza · camuffamento del trasporto"
assicura_binario
cd "$TMP" || exit 1
KEY="00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
"$BIN" authority > a.txt 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' a.txt | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' a.txt | awk '{print $2}')
"$BIN" node --listen 127.0.0.1:19051 --obfuscation "$KEY" > n.log 2>&1 & PIDS+=($!)
R=$(attendi_regex n.log "127\.0\.0\.1:19051,[0-9a-f]+,[0-9a-f]+")
"$BIN" directory --authority-secret "$SEC" --entries "$R" > dir.txt 2>/dev/null
ON=$(timeout 10 "$BIN" nodes --directory dir.txt --authority "$PUB" --probe --obfuscation "$KEY" --timeout 3 2>/dev/null | grep -c online)
OFF=$(timeout 10 "$BIN" nodes --directory dir.txt --authority "$PUB" --probe --timeout 3 2>/dev/null | grep -c online)
if [ "$ON" -ge 1 ]; then pass "con la chiave di rete il nodo risponde"; else fail "con la chiave il nodo NON risponde"; fi
if [ "$OFF" -eq 0 ]; then pass "senza la chiave il nodo resta muto (offuscamento attivo)"; else fail "senza chiave il nodo risponde (offuscamento NON attivo)"; fi
[ "$KO" -eq 0 ]
