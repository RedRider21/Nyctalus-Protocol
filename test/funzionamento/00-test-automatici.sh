#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Tutti i test automatici del motore (cargo test): crittografia, celle, conflux,
# rendezvous, chat (stretta interna, nomi dei file, segreto…).
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · test automatici del motore (cargo test)"
if ( cd "$ROOT" && cargo test --quiet > "$TMP/cargo.log" 2>&1 ); then
  N=$(grep -oE "test result: ok\. [0-9]+" "$TMP/cargo.log" | grep -oE "[0-9]+$" | paste -sd+ | bc)
  pass "tutti i test automatici superati ($N)"
else
  fail "cargo test ha segnalato errori:"; grep -E "FAILED|panicked" "$TMP/cargo.log" | head -10
fi
[ "$KO" -eq 0 ]
