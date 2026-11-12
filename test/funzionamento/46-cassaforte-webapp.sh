#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# CASSAFORTE nell'APP DEL TELEFONO (web-app, in Chrome): con i dati cifrati il servizio parte lo
# stesso, la chat chiede la passphrase, una sbagliata viene rifiutata, quella giusta apre la chat
# con i suoi contatti; nelle Impostazioni (scheda 💬 Chat) la protezione risulta accesa.
# HOME finta con dati di prova (mai i dati veri).
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · cassaforte nell'app del telefono"
command -v google-chrome >/dev/null || { salta "google-chrome non installato"; exit 0; }
assicura_binario
H="$TMP/home"; mkdir -p "$H/.nyctalus-mobile" "$H/.nyctalus-chat/cronologia"
TOK=segreto0123456789abcdef0123456789; printf %s "$TOK" > "$H/.nyctalus-mobile/token"
cat > "$H/.nyctalus-mobile/config.json" <<'EOF'
{"porte": {"http": 19833, "controllo": 19691, "backend": 19682, "socks": 19680, "socks_locale": 19686,
           "monitor": 19696, "nodi": [19640, 19641, 19642, 19643]}}
EOF
BOB=mdeq6zcq5g2xwl4swp6n2wmve6clwolpffok32pojjostzjdec7lsaa.nyct
hx() { printf %s "$1" | xxd -p | tr -d '\n'; }
printf '%s %s\n' "$BOB" "$(hx Bob)" > "$H/.nyctalus-chat/contatti"
printf 'in %d %s\n' "$(date +%s)" "$(hx 'messaggio segreto')" > "$H/.nyctalus-chat/cronologia/$BOB.log"
PP="passphrase del telefono"

# i dati si cifrano una volta con il motore (come farebbe «Proteggi con una passphrase»)
NYCTALUS_CHAT_TOKEN=preparazione0123456789 "$BIN" chat --data "$H/.nyctalus-chat" --backend 127.0.0.1:19750 --control 127.0.0.1:19751 --socks 127.0.0.1:19752 > "$TMP/prep.log" 2>&1 & PREP=$!; PIDS+=($PREP)
attendi_regex "$TMP/prep.log" '[a-z2-7]{56}\.nyct' 10 > /dev/null
NYCT_FINO="^(CIFRATA|ERR)" controllo 19751 preparazione0123456789 30 "CIFRA $(hx "$PP")" | grep -q "^CIFRATA" && kill $PREP && wait $PREP 2>/dev/null
grep -q "messaggio segreto\|$(hx 'messaggio segreto')" "$H/.nyctalus-chat/cronologia/$BOB.log" && fail "dati di prova non cifrati" || pass "dati della chat cifrati sul telefono (prova)"

HOME="$H" NYCTALUS_BIN="$BIN" python3 "$ROOT/mobile/orchestratore/nyctalus_mobile.py" > "$TMP/o.log" 2>&1 & PIDS+=($!)
[ -n "$(attendi_regex "$TMP/o.log" 'servizi pronti' 40)" ] && grep -q "la chat aspetta la passphrase" "$TMP/o.log" \
  && pass "il servizio del telefono parte anche con i dati cifrati (la chat aspetta la passphrase)" || fail "servizi: $(tail -3 "$TMP/o.log")"
U="http://127.0.0.1:19833"
cat > "$TMP/passi.json" <<EOF
[["vai","$U/?t=$TOK"],["attendi",3],
 ["js","'DIALOGO ' + (!document.getElementById('dialogo').classList.contains('nascosto')) + ',' + document.querySelector('#dialogo-scatola h3').textContent + ',' + !!document.querySelector('#dialogo-scatola input[type=password]')"],
 ["js","(()=>{const c=document.querySelector('#dialogo-scatola input[type=password]'); c.value='non è lei'; document.querySelector('#dialogo-scatola .pulsanti button').click(); return 'mandata'})()"],
 ["attendi",3.5],
 ["js","'SBAGLIATA ' + (!document.getElementById('dialogo').classList.contains('nascosto')) + ',' + document.querySelector('#dialogo-scatola .nota-rete').textContent"],
 ["foto","$TMP/sblocco.png"],
 ["js","(()=>{const c=document.querySelector('#dialogo-scatola input[type=password]'); c.value='$PP'; document.querySelector('#dialogo-scatola .pulsanti button').click(); return 'mandata'})()"],
 ["finche","S.me && S.me.endsWith('.nyct')",30],
 ["attendi",2],
 ["js","'APERTA ' + S.me.endsWith('.nyct') + ',' + document.querySelectorAll('.riga').length + ',' + document.getElementById('dialogo').classList.contains('nascosto')"],
 ["js","impostazioni(); [...document.querySelectorAll('.tabber-ling')].find(x=>x.textContent.includes('Chat')).click(); 'imp'"],["attendi",1.5],
 ["js","'PROTEZIONE ' + document.querySelector('.stato-cassaforte').textContent.startsWith('🔐 Accesa') + ',' + [...document.querySelectorAll('.tabber-pan:not(.nascosto) .pulsanti button')].map(b=>b.textContent).join('/')"]
]
EOF
timeout 300 python3 "$STRUMENTI/cdp.py" "$TMP/chrome" "$TMP/passi.json" > "$TMP/r.txt" 2>&1
r() { grep -oE "^js → $1 .*" "$TMP/r.txt" | head -1 | cut -d' ' -f4-; }
[ "$(r DIALOGO)" = "true,🔐 Dati protetti,true" ] && pass "all'apertura la chat chiede la passphrase (campo nascosto)" || fail "schermata di sblocco: $(r DIALOGO)"
r SBAGLIATA | grep -q "^true,✗ Passphrase sbagliata" && pass "passphrase sbagliata: «Passphrase sbagliata, riprova»" || fail "sbagliata: $(r SBAGLIATA)"
[ "$(r APERTA)" = "true,1,true" ] && pass "passphrase giusta: la chat si apre con il suo Contact ID e i contatti" || fail "dopo lo sblocco: $(r APERTA)"
r PROTEZIONE | grep -q "^true,Cambia passphrase/Togli la protezione" && pass "Impostazioni › 💬 Chat: protezione «Accesa», con «Cambia passphrase» e «Togli la protezione»" || fail "impostazioni: $(r PROTEZIONE)"
[ "$KO" -eq 0 ]
