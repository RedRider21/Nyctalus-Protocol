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
# una galleria finta (al posto di DCIM del telefono): foto e un video
G="$TMP/galleria/Camera"; mkdir -p "$G"
for c in 1e88e5 e53935 43a047 fdd835 8e24aa; do
  python3 -c "import zlib,struct,sys;w=h=96;c=bytes.fromhex(sys.argv[1]);raw=b''.join(b'\x00'+c*w for _ in range(h))
def k(t,d):return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d))
open(sys.argv[2],'wb').write(b'\x89PNG\r\n\x1a\n'+k(b'IHDR',struct.pack('>IIBBBBB',w,h,8,2,0,0,0))+k(b'IDAT',zlib.compress(raw))+k(b'IEND',b''))" "$c" "$G/IMG_$c.png"
  sleep 0.05
done
command -v ffmpeg >/dev/null && ffmpeg -v error -f lavfi -i testsrc2=size=320x240:rate=10:duration=2 -pix_fmt yuv420p "$G/VID_prova.mp4"
# tante foto più vecchie (come un telefono vero): la griglia non deve schiacciare le caselle
V="$TMP/galleria/Vecchie"; mkdir -p "$V"
for k in $(seq -w 1 60); do cp "$G/IMG_1e88e5.png" "$V/OLD_$k.png"; touch -d "2 days ago" "$V/OLD_$k.png"; done
NYCTALUS_GALLERIA="$TMP/galleria" NYCTALUS_GALLERIA_SOLO=1 HOME="$H" NYCTALUS_BIN="$BIN" python3 "$ROOT/mobile/orchestratore/nyctalus_mobile.py" > "$TMP/o.log" 2>&1 & PIDS+=($!)
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
 ["js","(()=>{const m=document.getElementById('messaggi'); return 'FONDO ' + (m.scrollHeight - m.scrollTop - m.clientHeight < 5)})()"],
 ["js","document.querySelector('#messaggi .msg').dispatchEvent(new MouseEvent('contextmenu',{bubbles:true})); 'MENU ' + document.querySelectorAll('#foglio .voce').length"],
 ["js","chiudiFoglio(); manda('ADD '+S.me+' '+inHex('Io')); aggiornaContatto(S.me,'Io'); apriChat(S.me); 'io'"],["attendi",1.5],
 ["js","manda('SEND '+S.me+' '+inHex('prova dalla web-app')); 'inviato'"],
 ["js","(async()=>{const c=document.createElement('canvas');c.width=320;c.height=200;const g=c.getContext('2d');g.fillStyle='#3a7bd5';g.fillRect(0,0,320,200);const b=await new Promise(o=>c.toBlob(o,'image/jpeg',0.9));await carica(S.me,new File([b],'foto-web.jpg',{type:'image/jpeg'}),true);return 'CARICATA'})()"],
 ["attendi",14],
 ["js","'IO ' + [...document.querySelectorAll('#messaggi .msg')].map(m=>m.className.split(' ')[1]+':'+(m.querySelector('img')?'FOTO':'TESTO')).join(',')"],
 ["js","S.exif=false; apriGalleria(); 'galleria'"],["attendi",2],
 ["js","'TILE ' + document.querySelectorAll('#foglio .tile[data-i]').length"],
 ["js","(()=>{const t=[...document.querySelectorAll('#foglio .tile')]; const brutte=t.filter(x=>x.offsetWidth<40||Math.abs(x.offsetHeight-x.offsetWidth)>2); return 'QUADRATE ' + (t.length-brutte.length) + '/' + t.length + (brutte.length?(' es. '+brutte[0].offsetWidth+'x'+brutte[0].offsetHeight):'')})()"],
 ["js","(()=>{const t=document.querySelectorAll('#foglio .tile[data-i] img'); return 'MINI ' + [...t].filter(i=>i.complete&&i.naturalWidth>0).length})()"],
 ["js","(()=>{const t=document.querySelectorAll('#foglio .tile[data-i]'); t[1].click(); t[3].click(); return 'NUMERI ' + [...document.querySelectorAll('#foglio .tile.scelta .num')].map(n=>n.textContent).join(',')})()"],
 ["js","'BOTTONE ' + document.querySelector('#foglio .invia-gal').textContent"],
 ["foto","${NYCT_FOTO:-$TMP}/galleria.png"],
 ["js","(()=>{window._prima=document.querySelectorAll('#messaggi .msg img').length; document.querySelector('#foglio .invia-gal').click(); return 'inviate'})()"],
 ["finche","document.querySelectorAll('#messaggi .msg img').length - window._prima >= 4",30],
 ["js","'NUOVE ' + (document.querySelectorAll('#messaggi .msg img').length - window._prima)"],
 ["foto","${NYCT_FOTO:-$TMP}/chat-dopo-galleria.png"],
 ["js","(()=>{apriEmoji(); const b=document.querySelectorAll('.griglia-emoji button'); const n=b.length; b[0].click(); const ok=document.getElementById('testo').value.length>0; chiudiFoglio(); document.getElementById('testo').value=''; return 'EMOJI '+n+' '+ok})()"],
 ["js","(()=>{evento('ST '+S.attivo+' 0'); return 'VISTO '+document.getElementById('testa-stato').textContent})()"],
 ["js","impostazioni(); 'impostazioni'"],["finche","document.querySelector('.stato-rete-imp') && document.querySelector('.stato-rete-imp').dataset.fase !== 'avvio'",10],
 ["js","(()=>{const b=[...document.querySelectorAll('#dialogo .pulsanti button')]; const a=b.find(x=>x.textContent.includes('Avvia')), f=b.find(x=>x.textContent.includes('Ferma')); return 'RETE ' + document.querySelector('.stato-rete-imp').dataset.fase + ',' + a.disabled + ',' + f.disabled})()"],
 ["foto","${NYCT_FOTO:-$TMP}/impostazioni.png"]]
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
Q=$(r QUADRATE); [ -n "$Q" ] && [ "${Q%%/*}" = "$(echo "$Q" | cut -d/ -f2 | cut -d' ' -f1)" ] && pass "caselle della galleria tutte quadrate anche con molte foto ($Q)" || fail "caselle schiacciate: $Q"
[ "$(r TILE)" -ge 6 ] && pass "selettore: griglia con le foto e i video recenti ($(r TILE))" || fail "griglia della galleria: $(r TILE)"
[ "$(r MINI)" -ge 5 ] && pass "miniature caricate dal telefono" || fail "miniature: $(r MINI)"
[ "$(r NUMERI)" = "1,2" ] && pass "selezione numerata come Telegram (1, 2)" || fail "numerazione: $(r NUMERI)"
[ "$(r BOTTONE)" = "Invia (2)" ] && pass "pulsante «Invia (2)»" || fail "pulsante: $(r BOTTONE)"
[ "$(r NUOVE)" -ge 4 ] && pass "le 2 foto della galleria inviate e ricevute" || fail "foto dalla galleria: $(r NUOVE)"
r VISTO | grep -q "^ultimo accesso oggi alle" && pass "telefono: «ultimo accesso» quando il contatto esce (anche con motore vecchio)" || fail "ultimo accesso sul telefono: $(r VISTO)"
E=$(r EMOJI); [ "${E%% *}" -ge 50 ] 2>/dev/null && [ "${E##* }" = true ] && pass "pannello emoticon: $(echo "$E" | cut -d' ' -f1) emoticon, si inseriscono nel testo" || fail "emoticon: $E"
[ "$(r RETE)" = "pronto,true,false" ] && pass "impostazioni: rete «attiva», «Avvia» spento e «Ferma» acceso" || fail "pulsanti della rete: $(r RETE)"
[ "$KO" -eq 0 ]
