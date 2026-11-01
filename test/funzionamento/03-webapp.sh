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
 ["js","(()=>{window._inviate=[]; const v=S.ws.send.bind(S.ws); S.ws.send=(r)=>{ if(/^(CALL|AUD|VID)/.test(r)) window._inviate.push(r); else v(r); }; evento('CALLIN $BOB abc123'); const s=Chiamate._stato(); return 'SQUILLA '+Chiamate.disponibile()+','+(s.c&&s.c.stato)+','+!document.querySelector('.ch-arrivo').classList.contains('nascosto')})()"],
 ["foto","${NYCT_FOTO:-$TMP}/chiamata-arrivo.png"],
 ["js","document.querySelector('.ch-arrivo .verde').click(); 'risposto'"],["attendi",3],
 ["js","(()=>{const s=Chiamate._stato(); const aud=window._inviate.filter(r=>r.startsWith('AUD $BOB ')); window._aud=aud; return 'RISPOSTA '+window._inviate.includes('CALLOK $BOB abc123')+','+(s.c&&s.c.stato)+','+(aud.length>50)})()"],
 ["js","(()=>{window._aud.forEach(r=>{const p=r.split(' '); Chiamate._ricevi(+p[2],p[3]);}); return 'x'})()"],["attendi",1],
 ["js","'SUONATI ' + ((Chiamate._stato().a||{}).suonati||0)"],
 ["js","'USCITA ' + (Chiamate._stato().a||{}).uscita"],
 ["foto","${NYCT_FOTO:-$TMP}/chiamata-attiva.png"],
 ["js","(()=>{document.querySelector('.ch-tasti .ch-muto').click(); const m=Chiamate._stato().muto; const n=window._inviate.length; window._n=n; return 'MUTO '+m})()"],["attendi",1],
 ["js","'MUTI ' + (window._inviate.length - window._n <= 3)"],
 ["js","(()=>{document.querySelector('.ch-riduci').click(); const r=!document.querySelector('.ch-mini').classList.contains('nascosto') && document.querySelector('.chiamata').classList.contains('nascosto'); document.querySelector('.ch-mini').click(); return 'RIDUCI '+r})()"],
 ["js","(()=>{window._v0=window._inviate.length; document.querySelector('.ch-tasti .ch-video').click(); return 'video acceso'})()"],["attendi",3],
 ["js","(()=>{const n=window._inviate.slice(window._v0); const vid=n.filter(r=>r.startsWith('VID $BOB ')); window._vid=vid; const s=Chiamate._stato(); const loc=document.querySelector('.ch-vid.locale'); return 'VIDEO '+n.includes('VIDSTATO $BOB 1')+','+(vid.length>=20)+','+(vid.length&&vid[0].split(' ')[3]==='1')+','+document.querySelector('.chiamata').classList.contains('video')+','+loc.classList.contains('pieno')+','+document.querySelector('.ch-video .et').textContent+','+!document.querySelector('.ch-gira').classList.contains('nascosto')})()"],
 ["js","(()=>{const m=Math.max(...window._vid.map(r=>r.length)); return 'PESO '+(m<120000)+' '+window._vid.length})()"],
 ["js","(()=>{evento('VIDSTATO $BOB 1'); window._vid.forEach(r=>evento(r)); return 'eco'})()"],["attendi",1.5],
 ["js","(()=>{const s=Chiamate._stato(); const t=document.querySelector('.ch-vid.remoto canvas:last-child'); return 'REMOTO '+s.remCam+','+document.querySelector('.ch-vid.remoto').classList.contains('pieno')+','+document.querySelector('.ch-vid.locale').classList.contains('pip')+','+(t.width>0&&t.height>0)+','+(s.v.ricevute>=10)})()"],
 ["foto","${NYCT_FOTO:-$TMP}/videochiamata.png"],
 ["js","(()=>{Chiamate._scambia(); return 'SCAMBIO '+document.querySelector('.ch-vid.locale').classList.contains('pieno')+','+document.querySelector('.ch-vid.remoto').classList.contains('pip')})()"],["attendi",0.5],
 ["js","(()=>{const b=document.querySelector('.ch-vid.pip'); const r=b.getBoundingClientRect(); const x=r.left+r.width/2, y=r.top+r.height/2; const ev=(t,X,Y)=>b.dispatchEvent(new PointerEvent(t,{bubbles:true,pointerId:7,clientX:X,clientY:Y,isPrimary:true})); ev('pointerdown',x,y); ev('pointermove',x-60,y+80); ev('pointermove',40,innerHeight-200); ev('pointerup',40,innerHeight-200); b.dispatchEvent(new WheelEvent('wheel',{deltaY:-100,bubbles:true,cancelable:true})); const s=Chiamate._stato(); return 'TRASCINA '+s.pip.angolo+','+(s.pip.scala>1)})()"],["attendi",0.6],
 ["foto","${NYCT_FOTO:-$TMP}/videochiamata-scambio.png"],
 ["js","(()=>{document.querySelector('.ch-scena').click(); const q=document.querySelector('.chiamata').classList.contains('quieto'); document.querySelector('.ch-scena').click(); return 'QUIETO '+q+','+!document.querySelector('.chiamata').classList.contains('quieto')})()"],
 ["js","(()=>{window._k0=window._inviate.length; setTimeout(()=>evento('VIDKEY $BOB'),400); return 'k'})()"],["attendi",1.2],
 ["js","(()=>{const n=window._inviate.slice(window._k0).filter(r=>r.startsWith('VID $BOB ')); const i=n.findIndex((r,j)=>j>=3&&r.split(' ')[3]==='1'); return 'CHIAVE '+(i>=0)})()"],
 ["js","(()=>{window._s0=window._inviate.length; document.querySelector('.ch-tasti .ch-video').click(); return 'video spento'})()"],["attendi",1],
 ["js","(()=>{const n=window._inviate.slice(window._s0); const dopo=n.slice(n.indexOf('VIDSTATO $BOB 0')+1).filter(r=>r.startsWith('VID $BOB ')); return 'SPENTO '+n.includes('VIDSTATO $BOB 0')+','+(dopo.length===0)+','+document.querySelector('.ch-video .et').textContent+','+document.querySelector('.ch-vid.locale').classList.contains('nascosto')+','+(Chiamate._stato().v.flusso===null)})()"],
 ["js","(()=>{evento('VIDSTATO $BOB 0'); return 'VOCE '+!document.querySelector('.chiamata').classList.contains('video')})()"],
 ["js","(()=>{document.querySelector('.ch-tasti .rosso').click(); return 'FINE '+window._inviate.includes('CALLEND $BOB abc123')+','+(Chiamate._stato().c===null)+','+document.querySelector('.chiamata').classList.contains('nascosto')})()"],
 ["js","(()=>{evento('CALLIN $BOB vid777 v'); const s=Chiamate._stato(); return 'VARRIVO '+(s.c&&s.c.video)+','+document.querySelector('.ch-stato').textContent+','+!document.querySelector('.ch-solovoce').classList.contains('nascosto')})()"],
 ["foto","${NYCT_FOTO:-$TMP}/videochiamata-arrivo.png"],
 ["js","document.querySelector('.ch-arrivo .ch-si').click(); 'risposto video'"],["attendi",3],
 ["js","(()=>{const s=Chiamate._stato(); return 'VRISPOSTA '+window._inviate.includes('CALLOK $BOB vid777')+','+!!(s.v&&s.v.flusso)+','+window._inviate.includes('VIDSTATO $BOB 1')})()"],
 ["js","(()=>{document.querySelector('.ch-tasti .ch-fine').click(); const s=Chiamate._stato(); return 'VFINE '+(s.c===null)+','+(s.v===null)+','+document.querySelector('.ch-vid.locale').classList.contains('nascosto')})()"],
 ["js","(()=>{const ts=9999999999; bollaTesto('out','prova spunte',ts); const r=[...document.querySelectorAll('#messaggi .msg.out')].pop(); if(!r) return 'SPUNTE nessuna'; const sp=()=>r.querySelector('.spunta'); const v=[]; evento('SP '+S.attivo+' '+ts+' 1'); v.push(sp().textContent); evento('CAS '+S.attivo+' '+ts); v.push(sp().textContent); evento('SP '+S.attivo+' '+ts+' 2'); v.push(sp().textContent); evento('SP '+S.attivo+' '+ts+' 3'); v.push(sp().textContent+(sp().classList.contains('letto')?'blu':'')); return 'SPUNTE '+v.join(',')})()"],
 ["js","(()=>{const n0=document.querySelectorAll('#messaggi .avviso').length; for(let i=0;i<5;i++) evento('ERR motore non raggiungibile'); const n1=document.querySelectorAll('#messaggi .avviso').length; for(let i=0;i<3;i++) evento('ERR SOCKS: connessione rifiutata'); const n2=document.querySelectorAll('#messaggi .avviso').length; const bar=!document.getElementById('stato-rete').classList.contains('nascosto'); S.motoreGiu=false; return 'SFILZA '+(n1-n0)+','+(n2-n1)+','+bar})()"],
 ["js","impostazioni(); 'impostazioni'"],["finche","document.querySelector('.stato-rete-imp') && document.querySelector('.stato-rete-imp').dataset.fase !== 'avvio'",10],
 ["js","(()=>{const b=[...document.querySelectorAll('#dialogo .pulsanti button')]; const a=b.find(x=>x.textContent.includes('Avvia')), f=b.find(x=>x.textContent.includes('Ferma')); return 'RETE ' + document.querySelector('.stato-rete-imp').dataset.fase + ',' + a.disabled + ',' + f.disabled})()"],
 ["foto","${NYCT_FOTO:-$TMP}/impostazioni.png"],
 ["vai","$U/?avvio=1#rubrica"],["attendi",2],
 ["js","(()=>{const f=document.getElementById('foglio'), r=f.getBoundingClientRect(); return 'SOLO ' + document.body.classList.contains('solo-rubrica') + ',' + !f.classList.contains('nascosto') + ',' + (r.top===0 && r.height>=innerHeight-1) + ',' + !!f.querySelector('.indietro-rubrica')})()"],
 ["js","(()=>{const b=document.querySelector('#foglio .rubrica'); return 'SENZA ' + (b.textContent.includes('non ha una rubrica') && !!b.querySelector('.riprova-rubrica'))})()"],
 ["js","(()=>{S.rubrica='rubricaprova.nyct'; S.rubricaDati={op:'elenco',iscritto:false,utenti:[{a:'$BOB',n:'Bob',on:true,v:0},{a:'altro.nyct',n:'Carla',on:false,v:Math.floor(Date.now()/1000)-3600}]}; disegnaRubrica(); return 'UTENTI ' + document.querySelectorAll('#foglio .rubrica-utente').length})()"],
 ["foto","${NYCT_FOTO:-$TMP}/rubrica.png"],
 ["js","(()=>{document.querySelector('#foglio .rubrica-utente').click(); return 'SCRIVI ' + document.body.classList.contains('solo-rubrica') + ',' + document.getElementById('foglio').classList.contains('nascosto') + ',' + (S.attivo==='$BOB')})()"]]
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
[ "$(r SQUILLA)" = "true,squilla,true" ] && pass "chiamata in arrivo: schermata con Rifiuta/Rispondi" || fail "chiamata in arrivo: $(r SQUILLA)"
[ "$(r RISPOSTA)" = "true,attiva,true" ] && pass "Rispondi: CALLOK e voce dal microfono (Opus 20 ms, microfono FINTO)" || fail "risposta: $(r RISPOSTA)"
[ "${R_S:=$(r SUONATI)}" -ge 30 ] 2>/dev/null && pass "la voce ricevuta si decodifica e suona ($R_S pacchetti)" || fail "voce ricevuta: $(r SUONATI)"
[ "$(r USCITA)" = "telefonata" ] && pass "la voce esce come telefonata (WebRTC interno): eco cancellata, auricolare/vivavoce veri" || fail "uscita della voce: $(r USCITA)"
[ "$(r MUTO)" = "true" ] && [ "$(r MUTI)" = "true" ] && pass "🎤 muto: non parte più voce" || fail "muto: $(r MUTO) $(r MUTI)"
[ "$(r RIDUCI)" = "true" ] && pass "⌄ riduci: barretta in cima, la chiamata continua" || fail "riduci: $(r RIDUCI)"
[ "$(r VIDEO)" = "true,true,true,true,true,Solo voce,true" ] && pass "📹 durante la chiamata: VIDSTATO 1, immagini VP8 (la prima completa), ti vedi a tutto schermo, il tasto diventa 📞 «Solo voce», compare «Gira»" || fail "video acceso: $(r VIDEO)"
r PESO | grep -q "^true" && pass "immagini sotto il limite del motore ($(r PESO | cut -d' ' -f2) inviate in 3 s, telecamera FINTA)" || fail "immagini troppo grandi: $(r PESO)"
[ "$(r REMOTO)" = "true,true,true,true,true" ] && pass "video dell'altro decodificato a tutto schermo, il tuo nel riquadro" || fail "video ricevuto: $(r REMOTO)"
[ "$(r SCAMBIO)" = "true,true" ] && pass "tocco sul riquadro: scambio fra te e l'altro" || fail "scambio: $(r SCAMBIO)"
[ "$(r TRASCINA)" = "bs,true" ] && pass "riquadro trascinato si aggancia all'angolo (in basso a sinistra) e si ingrandisce" || fail "trascina/ridimensiona: $(r TRASCINA)"
[ "$(r QUIETO)" = "true,true" ] && pass "tocco sul video: comandi nascosti e di nuovo visibili" || fail "comandi nascosti: $(r QUIETO)"
[ "$(r CHIAVE)" = "true" ] && pass "VIDKEY: l'altro chiede e riceve subito un'immagine completa" || fail "immagine completa su richiesta"
[ "$(r SPENTO)" = "true,true,Video,true,true" ] && pass "📞 «Solo voce»: VIDSTATO 0, niente più immagini, telecamera spenta" || fail "video spento: $(r SPENTO)"
[ "$(r VOCE)" = "true" ] && pass "l'altro spegne la telecamera: si torna alla schermata della voce" || fail "ritorno alla voce: $(r VOCE)"
[ "$(r VARRIVO)" = "true,videochiamata in arrivo,true" ] && pass "videochiamata in arrivo: Rifiuta / Solo voce / Rispondi" || fail "videochiamata in arrivo: $(r VARRIVO)"
[ "$(r VRISPOSTA)" = "true,true,true" ] && pass "Rispondi alla videochiamata: CALLOK, telecamera accesa, VIDSTATO 1" || fail "risposta video: $(r VRISPOSTA)"
[ "$(r VFINE)" = "true,true,true" ] && pass "fine videochiamata: telecamera rilasciata" || fail "fine video: $(r VFINE)"
[ "$(r FINE)" = "true,true,true" ] && pass "✆ riaggancia: CALLEND e schermata chiusa" || fail "fine: $(r FINE)"
[ "$(r SPUNTE)" = "✓,📮✓,✓✓,✓✓blu" ] && pass "spunte come Telegram: ✓ spedito, 📮✓ casella, ✓✓ consegnato, ✓✓ blu letto" || fail "spunte: $(r SPUNTE)"
[ "$(r SFILZA)" = "0,1,true" ] && pass "errori ripetuti: un solo avviso (motore giù → barra in alto, non 30 righe in chat)" || fail "sfilza di errori: $(r SFILZA)"
[ "$(r RETE)" = "pronto,true,false" ] && pass "impostazioni: rete «attiva», «Avvia» spento e «Ferma» acceso" || fail "pulsanti della rete: $(r RETE)"
[ "$(r SOLO)" = "true,true,true,true" ] && pass "riquadro «Rubrica» (#rubrica): la sola rubrica a tutto schermo, con «←»" || fail "sola rubrica: $(r SOLO)"
[ "$(r SENZA)" = "true" ] && pass "rete senza rubrica: lo dice e propone «Riprova» (niente scritta ferma)" || fail "rubrica su rete senza rubrica: $(r SENZA)"
[ "$(r UTENTI)" = "2" ] && pass "rubrica: elenco degli iscritti disegnato" || fail "rubrica: $(r UTENTI)"
[ "$(r SCRIVI)" = "false,true,true" ] && pass "dalla rubrica, scegliendo qualcuno si esce dalla rubrica e si apre la conversazione" || fail "scrivi dalla rubrica: $(r SCRIVI)"
[ "$KO" -eq 0 ]
