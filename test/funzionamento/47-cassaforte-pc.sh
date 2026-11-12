#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# CASSAFORTE nella CHAT DEL PC: con i dati cifrati la chat chiede la passphrase all'avvio (sbagliata
# → la richiede; giusta → si ricollega e mostra contatti e indirizzo) e dal menu «🔐 Protezione dei
# dati…» si cambia la passphrase. Motore vero, dati di prova in una HOME finta.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · cassaforte nella chat del PC"
[ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] || { salta "nessun display grafico"; exit 0; }
assicura_binario
D="$TMP/chat"; mkdir -p "$D/cronologia" "$TMP/home/.config"; SEG=segretodelpc0123456789abcdef; C=23551
BOB=mdeq6zcq5g2xwl4swp6n2wmve6clwolpffok32pojjostzjdec7lsaa.nyct; PP="passphrase del PC di prova"
hx() { printf %s "$1" | xxd -p | tr -d '\n'; }
printf '%s %s\n' "$BOB" "$(hx Bob)" > "$D/contatti"
printf 'in %d %s\n' "$(date +%s)" "$(hx 'messaggio segreto')" > "$D/cronologia/$BOB.log"
motore() { NYCTALUS_CHAT_TOKEN="$SEG" "$BIN" chat --data "$D" --backend 127.0.0.1:23550 --control 127.0.0.1:$C --socks 127.0.0.1:23552 > "$TMP/chat$1.log" 2>&1 & MOT=$!; PIDS+=($MOT); }
motore 1
attendi_regex "$TMP/chat1.log" '[a-z2-7]{56}\.nyct' 10 > /dev/null
NYCT_FINO="^(CIFRATA|ERR)" controllo $C "$SEG" 30 "CIFRA $(hx "$PP")" | grep -q "^CIFRATA" || { fail "dati di prova non cifrati"; exit 1; }
kill $MOT; wait $MOT 2>/dev/null; sleep 0.5
motore 2
attendi_regex "$TMP/chat2.log" "aspetto la passphrase" 10 > /dev/null && pass "motore riavviato chiuso (dati di prova cifrati)" || fail "motore non chiuso"
HOME="$TMP/home" XDG_CONFIG_HOME="$TMP/home/.config" NYCTALUS_CHAT_PORT=$C NYCTALUS_CHAT_TOKEN="$SEG" NYCTALUS_SENZA_SUONI=1 \
  PYTHONPATH="$ROOT/pc/chat:$ROOT/pc/browser" \
  timeout 90 python3 "$STRUMENTI/prova_cassaforte_pc.py" "$PP" "$BOB" > "$TMP/r.txt" 2>"$TMP/e.txt" \
  || echo "KO prova interrotta (uscita $?): $(grep -v Warning "$TMP/e.txt" | tail -2)" >> "$TMP/r.txt"
while read -r esito resto; do
  case "$esito" in OK) pass "$resto" ;; KO) fail "$resto" ;; esac
done < "$TMP/r.txt"
# la passphrase è davvero cambiata: la vecchia non apre più, la nuova sì
kill $MOT; wait $MOT 2>/dev/null; sleep 0.5
motore 3
attendi_regex "$TMP/chat3.log" "aspetto la passphrase" 10 > /dev/null
R1=$(NYCT_FINO="^ERR|SBLOCCATA" controllo $C "$SEG" 15 "SBLOCCA $(hx "$PP")")
R2=$(NYCT_FINO="^ERR|SBLOCCATA" controllo $C "$SEG" 30 "SBLOCCA $(hx 'passphrase cambiata dal PC')")
echo "$R1" | grep -q "^ERR" && echo "$R2" | grep -q SBLOCCATA && pass "dopo il cambio la vecchia passphrase non apre più, la nuova sì" || fail "cambio: $R1 / $R2"
[ "$KO" -eq 0 ]
