#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# La porta di controllo della chat (che legge e manda i messaggi) accetta solo
# chi presenta il segreto: su Android ogni app può aprire le porte di 127.0.0.1.
. "$(dirname "$0")/../lib.sh"
titolo "Sicurezza · segreto della porta di controllo"
assicura_binario
SEG="segretoprova0123456789abcdef"
NYCTALUS_CHAT_TOKEN="$SEG" "$BIN" chat --data "$TMP/c" --backend 127.0.0.1:19182 --control 127.0.0.1:19191 \
    --socks 127.0.0.1:1 > "$TMP/c.log" 2>&1 & PIDS+=($!)
[ -n "$(attendi_regex "$TMP/c.log" '[a-z2-7]{56}\.nyct' 10)" ] || { fail "il motore non parte"; exit 1; }
R=$(controllo 19191 - 0.8 WHOAMI LIST)
if echo "$R" | grep -q "^ME "; then fail "senza segreto il motore risponde (grave)"; else pass "senza segreto: nessuna risposta utile"; fi
echo "$R" | grep -q "autenticazione richiesta" && pass "senza segreto: «autenticazione richiesta»" || fail "manca il rifiuto esplicito"
R=$(controllo 19191 segretosbagliato0123456789ab 0.8 WHOAMI)
echo "$R" | grep -q "^ME " && fail "segreto SBAGLIATO accettato (grave)" || pass "segreto sbagliato → rifiutato"
R=$(controllo 19191 "$SEG" 0.8 WHOAMI)
echo "$R" | grep -q "^ME " && pass "segreto giusto → accettato" || fail "segreto giusto rifiutato"
verifica "segreto troppo corto rifiutato all'avvio" bash -c "! NYCTALUS_CHAT_TOKEN=corto '$BIN' chat --data '$TMP/d' --backend 127.0.0.1:19183 --control 127.0.0.1:19192 --socks 127.0.0.1:1"
[ "$KO" -eq 0 ]
