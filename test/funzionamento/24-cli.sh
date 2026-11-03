#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# nyctalus-cli: chat e file dal terminale fra Alice e Bob sulla rete di prova (circuiti a 3
# salti), solo attraverso il comando; poi «nyctalus-cli avvia» (motore senza finestre) in una
# rete isolata (user namespace, HOME finto): niente tocca il motore vero dell'utente.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · nyctalus-cli (chat e file dal terminale)"
assicura_binario
CLI="$ROOT/pc/cli/nyctalus-cli"
rete_di_prova 21700
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:21780 --hops 3 --control 127.0.0.1:21790 > "$TMP/ca.log" 2>&1 & PIDS+=($!)
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:21781 --hops 3 --control 127.0.0.1:21791 > "$TMP/cb.log" 2>&1 & PIDS+=($!)
sleep 2
SA=alicesegreto0123456789abcdef; SB=bobsegreto0123456789abcdef00
identita alice 21711 21721 21780 "$SA"; ALICE=$ADDR
identita bob 21712 21722 21781 "$SB"; BOB=$ADDR
[ -n "$ALICE" ] && [ -n "$BOB" ] || { fail "identità non partite"; exit 1; }
sleep 5
timeout 900 python3 "$STRUMENTI/prova_cli.py" "$CLI" 21721 "$SA" 21722 "$SB" "$ALICE" "$BOB" "$TMP" > "$TMP/r.txt" 2>&1 \
  || echo "KO prova interrotta: $(tail -1 "$TMP/r.txt")" >> "$TMP/r.txt"
while read -r esito resto; do
  case "$esito" in OK) pass "$resto" ;; KO) fail "$resto" ;; esac
done < "$TMP/r.txt"

# «avvia»: motore senza finestre, in una rete isolata (porte 1091… del namespace, non quelle vere)
if unshare -rn true 2>/dev/null; then
  mkdir -p "$TMP/casa-avvia"
  HOME="$TMP/casa-avvia" timeout 120 unshare -rn bash -c '
    ip link set lo up
    python3 "'"$CLI"'" avvia > "'"$TMP"'/avvia.log" 2>&1 & P=$!
    for _ in $(seq 1 60); do grep -q "pronto" "'"$TMP"'/avvia.log" && break; sleep 0.5; done
    echo "IO $(python3 "'"$CLI"'" io 2>&1)"
    echo "PERM $(stat -c %a "$HOME/.nyctalus-chat/controllo" 2>&1)"
    echo "DUE $(python3 "'"$CLI"'" avvia 2>&1 | tail -1)"
    kill -TERM $P; wait $P   # (in sottofondo bash ignora SIGINT: il Ctrl-C vero arriva a tutto il gruppo)
    echo "DOPO $(python3 "'"$CLI"'" io 2>&1)"
    echo "FILE $( [ -e "$HOME/.nyctalus-chat/controllo" ] && echo resta || echo tolto)"
  ' > "$TMP/avvia.txt" 2>&1
  grep -qE '^IO [a-z2-7]{56}\.nyct$' "$TMP/avvia.txt" && pass "«nyctalus-cli avvia»: motore acceso senza finestre (né GTK), «io» risponde" || fail "avvia: $(cat "$TMP/avvia.txt" "$TMP/avvia.log" | tail -5)"
  grep -q '^PERM 600$' "$TMP/avvia.txt" && pass "il segreto del motore sta in un file privato (600)" || fail "permessi: $(grep PERM "$TMP/avvia.txt")"
  grep -q '^DUE .*già acceso' "$TMP/avvia.txt" && pass "un secondo «avvia» non accende un altro motore" || fail "secondo avvia: $(grep DUE "$TMP/avvia.txt")"
  grep -q '^DOPO .*non è acceso' "$TMP/avvia.txt" && grep -q '^FILE tolto' "$TMP/avvia.txt" \
    && pass "spegnendo «avvia» si spegne il motore e si cancella il segreto" || fail "spegnimento: $(grep -E 'DOPO|FILE' "$TMP/avvia.txt")"
  # ⌨ Terminale del mini menu (dentro la finestra di terminale): accende il motore, aiuto,
  # comandi, e chiudendo il terminale spegne il motore che ha acceso
  mkdir -p "$TMP/casa-term"
  HOME="$TMP/casa-term" timeout 120 unshare -rn bash -c '
    ip link set lo up
    printf "io\nexit\n" | bash "'"$ROOT"'/strumenti/avvia-terminale.sh" --dentro 2>/dev/null
    sleep 1
    echo "PORTA $(ss -ltn | grep -c ":1091\b")"
  ' > "$TMP/term.txt" 2>&1
  grep -q "nyctalus-cli — Nyctalus dal terminale" "$TMP/term.txt" && grep -qE "Il tuo Contact ID: [a-z2-7]{56}\.nyct" "$TMP/term.txt" \
    && pass "⌨ Terminale del PC: accende il motore senza finestre, mostra i comandi e il tuo Contact ID" || fail "terminale PC: $(head -5 "$TMP/term.txt")"
  [ "$(grep -oE '[a-z2-7]{56}\.nyct' "$TMP/term.txt" | wc -l)" -ge 2 ] \
    && pass "⌨ Terminale del PC: i comandi si scrivono così come sono, senza «nyctalus-cli» («io» risponde)" || fail "terminale PC, comando: $(tail -4 "$TMP/term.txt")"
  grep -q "^PORTA 0$" "$TMP/term.txt" && pass "⌨ Terminale del PC: chiudendolo si spegne il motore che aveva acceso" || fail "terminale PC, spegnimento: $(grep PORTA "$TMP/term.txt")"
else
  salta "«avvia»: user namespace non disponibili"
fi
[ "$KO" -eq 0 ]
