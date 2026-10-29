#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# L'orchestratore del telefono, sul PC: avvia rete e chat, fa da ponte
# WebSocket verso il motore, riceve e serve i file (anche a pezzi, Range), e
# fermandolo non lascia processi orfani.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · orchestratore del telefono"
assicura_binario
H="$TMP/home"; mkdir -p "$H/.nyctalus-mobile"
TOK=segreto0123456789abcdef0123456789; printf %s "$TOK" > "$H/.nyctalus-mobile/token"
cat > "$H/.nyctalus-mobile/config.json" <<'EOF'
{"porte": {"http": 19732, "controllo": 19491, "backend": 19482, "socks": 19480, "socks_locale": 19486,
           "monitor": 19496, "nodi": [19440, 19441, 19442, 19443]}}
EOF
HOME="$H" NYCTALUS_BIN="$BIN" python3 "$ROOT/mobile/orchestratore/nyctalus_mobile.py" > "$TMP/o.log" 2>&1 &
OPID=$!; PIDS+=($OPID)
[ -n "$(attendi_regex "$TMP/o.log" 'servizi pronti' 40)" ] && pass "servizi avviati (rete di prova + chat + servizio .nyct)" || { fail "servizi non avviati: $(tail -2 "$TMP/o.log")"; exit 1; }
python3 "$STRUMENTI/prova_orchestratore.py" 19732 "$TOK" > "$TMP/r.txt" 2>&1
while read -r esito resto; do          # niente pipe: i conteggi restano in questa shell
  case "$esito" in OK) pass "$resto" ;; KO) fail "$resto" ;; esac
done < "$TMP/r.txt"
kill "$OPID"; sleep 1.5
N=$(pgrep -fc -- "--data $H/.nyctalus-chat" || true)
[ "$N" = "0" ] && pass "fermato: nessun processo orfano" || fail "processi rimasti dopo l'arresto: $N"
[ "$KO" -eq 0 ]
