#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# La web-app del telefono dentro Chrome (a misura di telefono): elenco, cache
# IndexedDB, apertura di una chat, menu della pressione lunga, invio di un testo
# e di una foto a se stessi attraverso la rete vera. Serve google-chrome.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · web-app del telefono (in Chrome)"
command -v google-chrome >/dev/null || { salta "google-chrome non installato"; exit 0; }
assicura_binario
H="$TMP/home"; mkdir -p "$H/.nyctalus-mobile" "$H/.nyctalus-chat/cronologia"
TOK=segreto0123456789abcdef0123456789; printf %s "$TOK" > "$H/.nyctalus-mobile/token"
cat > "$H/.nyctalus-mobile/config.json" <<'EOF'
{"porte": {"http": 19733, "controllo": 19591, "backend": 19582, "socks": 19580, "socks_locale": 19586,
           "monitor": 19596, "nodi": [19540, 19541, 19542, 19543]}}
EOF
BOB=mdeq6zcq5g2xwl4swp6n2wmve6clwolpffok32pojjostzjdec7lsaa.nyct
hx() { printf %s "$1" | xxd -p | tr -d '\n'; }
printf '%s %s\n' "$BOB" "$(hx Bob)" > "$H/.nyctalus-chat/contatti"
ORA=$(date +%s)
printf 'in %d %s\nout %d %s\nin %d %s\n' $((ORA-600)) "$(hx 'Ciao, tutto bene?')" $((ORA-500)) "$(hx 'Sì, e tu?')" \
  $((ORA-60)) "$(hx 'Ci vediamo stasera?')" > "$H/.nyctalus-chat/cronologia/$BOB.log"
HOME="$H" NYCTALUS_BIN="$BIN" python3 "$ROOT/mobile/orchestratore/nyctalus_mobile.py" > "$TMP/o.log" 2>&1 & PIDS+=($!)
[ -n "$(attendi_regex "$TMP/o.log" 'servizi pronti' 40)" ] || { fail "servizi non avviati"; exit 1; }
U="http://127.0.0.1:19733"
cat > "$TMP/passi.json" <<EOF
[["vai","$U/?t=$TOK"],["attendi",3],
 ["js","'RIGHE ' + document.querySelectorAll('.riga').length"],
 ["js","'ID ' + (S.me.endsWith('.nyct') ? 'ok' : 'manca')"],
 ["vai","$U/"],["attendi",0.4],
 ["js","'CACHE ' + document.querySelectorAll('.riga').length"],
 ["attendi",2.5],
 ["js","apriChat('$BOB'); 'aperta'"],["attendi",3],
 ["js","'BOLLE ' + document.querySelectorAll('#messaggi .msg').length"],
 ["js","const m=document.getElementById('messaggi'); 'FONDO ' + (m.scrollHeight - m.scrollTop - m.clientHeight < 5)"],
 ["js","document.querySelector('#messaggi .msg').dispatchEvent(new MouseEvent('contextmenu',{bubbles:true})); 'MENU ' + document.querySelectorAll('#foglio .voce').length"],
 ["js","chiudiFoglio(); manda('ADD '+S.me+' '+inHex('Io')); aggiornaContatto(S.me,'Io'); apriChat(S.me); 'io'"],["attendi",1.5],
 ["js","manda('SEND '+S.me+' '+inHex('prova dalla web-app')); 'inviato'"],
 ["js","(async()=>{const c=document.createElement('canvas');c.width=320;c.height=200;const g=c.getContext('2d');g.fillStyle='#3a7bd5';g.fillRect(0,0,320,200);const b=await new Promise(o=>c.toBlob(o,'image/jpeg',0.9));await carica(S.me,new File([b],'foto-web.jpg',{type:'image/jpeg'}),true);return 'CARICATA'})()"],
 ["attendi",14],
 ["js","'IO ' + [...document.querySelectorAll('#messaggi .msg')].map(m=>m.className.split(' ')[1]+':'+(m.querySelector('img')?'FOTO':'TESTO')).join(',')"]]
EOF
timeout 120 python3 "$STRUMENTI/cdp.py" "$TMP/chrome" "$TMP/passi.json" > "$TMP/r.txt" 2>&1
r() { grep -oE "^js → $1 .*" "$TMP/r.txt" | head -1 | cut -d' ' -f4-; }
[ "$(r RIGHE)" = "1" ] && pass "elenco delle chat visualizzato" || fail "elenco: $(r RIGHE)"
[ "$(r ID)" = "ok" ] && pass "Contact ID ricevuto dal motore" || fail "Contact ID mancante"
[ "$(r CACHE)" = "1" ] && pass "dopo 0,4 s l'elenco c'è già (cache IndexedDB)" || fail "cache IndexedDB: $(r CACHE)"
[ "$(r BOLLE)" = "3" ] && pass "conversazione con i 3 messaggi della cronologia" || fail "bolle: $(r BOLLE)"
[ "$(r FONDO)" = "true" ] && pass "la chat si apre in fondo" || fail "chat non in fondo"
[ "${R_MENU:=$(r MENU)}" -ge 3 ] 2>/dev/null && pass "pressione lunga → menu del messaggio ($R_MENU voci)" || fail "menu della pressione lunga"
grep -q "CARICATA" "$TMP/r.txt" && pass "foto caricata dal browser (senza EXIF)" || fail "caricamento della foto"
IO=$(r IO)
for atteso in "out:TESTO" "in:TESTO" "out:FOTO" "in:FOTO"; do
  echo "$IO" | grep -q "$atteso" && pass "a se stessi attraverso la rete: $atteso" || fail "manca $atteso ($IO)"
done
[ "$KO" -eq 0 ]
