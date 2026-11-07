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
# programma File: l'archivio privato (file ricevuti) e le cartelle «del telefono» finte
mkdir -p "$H/.nyctalus-chat/allegati/prova"
head -c 2097152 /dev/urandom > "$H/.nyctalus-chat/allegati/prova/distro.iso"
head -c 4096 /dev/urandom > "$H/.nyctalus-chat/allegati/prova/nota.pdf"
NYCTALUS_SFOGLIA="$TMP/galleria" NYCTALUS_SFOGLIA_SOLO=1 \
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
 ["js","(()=>{const ts=9999999999; infoMessaggio(ts); const r=[...document.querySelectorAll('#dialogo .info-riga')].map(x=>x.querySelector('.cosa').textContent+':'+(x.classList.contains('no')?'no':(x.querySelector('.quando').textContent.startsWith('oggi alle')?'oggi':'si'))); document.getElementById('dialogo').classList.add('nascosto'); return 'INFO '+r.join(',')})()"],
 ["js","(()=>{const ts=10000000005; bollaTesto('out','info dal motore',ts,'grezza-di-prova'); evento('SP '+S.attivo+' '+ts+' 2 1700000000'); infoMessaggio(ts); const r=[...document.querySelectorAll('#dialogo .info-riga')].map(x=>x.querySelector('.cosa').textContent+':'+x.querySelector('.quando').textContent); document.getElementById('dialogo').classList.add('nascosto'); const m=[...document.querySelectorAll('#messaggi .msg.out')].pop(); m.dispatchEvent(new MouseEvent('contextmenu',{bubbles:true})); const v=[...document.querySelectorAll('#foglio .voce')].map(x=>x.textContent).join('|'); chiudiFoglio(); return 'INFOMOT '+r.join(',')+' || '+v.includes('Info')})()"],
 ["js","(()=>{const n0=document.querySelectorAll('#messaggi .avviso').length; for(let i=0;i<5;i++) evento('ERR motore non raggiungibile'); const n1=document.querySelectorAll('#messaggi .avviso').length; for(let i=0;i<3;i++) evento('ERR SOCKS: connessione rifiutata'); const n2=document.querySelectorAll('#messaggi .avviso').length; const bar=!document.getElementById('stato-rete').classList.contains('nascosto'); S.motoreGiu=false; return 'SFILZA '+(n1-n0)+','+(n2-n1)+','+bar})()"],
 ["js","(()=>{document.getElementById('btn-menu').click(); const r='DALLACHAT '+!document.getElementById('dialogo').classList.contains('nascosto')+','+S.soloRubrica+','+document.body.classList.contains('solo-rubrica'); document.getElementById('dialogo').classList.add('nascosto'); return r})()"],
 ["js","impostazioni(); 'impostazioni'"],["finche","document.querySelector('.stato-rete-imp') && document.querySelector('.stato-rete-imp').dataset.fase !== 'avvio'",10],
 ["js","(()=>{const b=[...document.querySelectorAll('#dialogo .pulsanti button')]; const a=b.find(x=>x.textContent.includes('Avvia')), f=b.find(x=>x.textContent.includes('Ferma')); return 'RETE ' + document.querySelector('.stato-rete-imp').dataset.fase + ',' + a.disabled + ',' + f.disabled})()"],
 ["foto","${NYCT_FOTO:-$TMP}/impostazioni.png"],
 ["vai","$U/?avvio=1#rubrica"],["attendi",2],
 ["js","(()=>{const f=document.getElementById('foglio'), r=f.getBoundingClientRect(); return 'SOLO ' + document.body.classList.contains('solo-rubrica') + ',' + !f.classList.contains('nascosto') + ',' + (r.top===0 && r.height>=innerHeight-1) + ',' + !!f.querySelector('.indietro-rubrica')})()"],
 ["js","(()=>{const b=document.querySelector('#foglio .rubrica'); return 'SENZA ' + (b.textContent.includes('non ha una rubrica') && !!b.querySelector('.riprova-rubrica'))})()"],
 ["js","(()=>{S.rubrica='rubricaprova.nyct'; S.rubricaDati={op:'elenco',iscritto:false,utenti:[{a:'$BOB',n:'Bob',on:true,v:0},{a:'altro.nyct',n:'Carla',on:false,v:Math.floor(Date.now()/1000)-3600}]}; disegnaRubrica(); return 'UTENTI ' + document.querySelectorAll('#foglio .rubrica-utente').length})()"],
 ["foto","${NYCT_FOTO:-$TMP}/rubrica.png"],
 ["js","(()=>{document.querySelector('#foglio .rubrica-utente').click(); return 'SCRIVI ' + document.body.classList.contains('solo-rubrica') + ',' + document.getElementById('foglio').classList.contains('nascosto') + ',' + (S.attivo==='$BOB')})()"],
 ["vai","$U/?avvio=1#impostazioni"],["attendi",2.5],
 ["js","(()=>{const f=document.getElementById('foglio'), r=f.getBoundingClientRect(), p=f.querySelector('.impostazioni-pagina'); return 'SOLOIMP ' + document.body.classList.contains('solo-rubrica') + ',' + (r.top===0 && r.height>=innerHeight-1) + ',' + !!f.querySelector('.indietro-rubrica') + ',' + (p?p.querySelectorAll('.scelta-tema').length:0) + ',' + (p&&p.querySelector('.codice-id').textContent.endsWith('.nyct')) + ',' + !!(p&&p.querySelector('.stato-rete-imp'))})()"],
 ["foto","${NYCT_FOTO:-$TMP}/impostazioni-schermo.png"],
 ["js","(()=>{const b=[...document.querySelectorAll('#foglio .scelta-tema > button')].find(x=>x.textContent.includes('Menta')); b.click(); const v=getComputedStyle(document.documentElement).getPropertyValue('--bg').trim(); return 'MENTA '+document.documentElement.dataset.tema+','+v})()"],
 ["foto","${NYCT_FOTO:-$TMP}/impostazioni-menta.png"],
 ["js","(()=>{const b=[...document.querySelectorAll('#foglio .scelta-tema > button')].find(x=>x.textContent.includes('Personalizzato')); b.click(); const p=document.querySelector('#foglio .tema-personalizzato'); const c=p.querySelector('input[type=text]'); c.value='#ff8800'; c.dispatchEvent(new Event('input')); [...p.querySelectorAll('.modo button')].find(x=>x.textContent==='Chiaro').click(); const st=getComputedStyle(document.documentElement); const atteso=derivaTema('#ff8800',false); return 'PERS '+document.documentElement.dataset.tema+','+!p.classList.contains('nascosto')+','+(st.getPropertyValue('--bg').trim()===atteso.bg)+','+(st.getPropertyValue('--accento').trim()===atteso.accento)+','+(p.querySelector('input[type=color]').value==='#ff8800')})()"],
 ["foto","${NYCT_FOTO:-$TMP}/impostazioni-personalizzato.png"],
 ["js","(()=>{document.querySelector('#foglio .tema-personalizzato .ripristina').click(); const sel=[...document.querySelector('#foglio .scelta-tema').querySelectorAll(':scope > button.sel')].map(x=>x.textContent.trim()).join('/'); return 'RIPR '+S.tema+','+S.temaColore.c+','+S.temaColore.scuro+','+document.querySelector('#foglio .tema-personalizzato').classList.contains('nascosto')+','+sel+','+(document.documentElement.style.getPropertyValue('--bg')==='')})()"],
 ["js","(()=>{const b=[...document.querySelectorAll('#foglio .scelta-tema > button')].find(x=>x.textContent.includes('Notte')); b.click(); return 'NOTTE '+document.documentElement.style.getPropertyValue('--bg')+'|'+document.querySelector('#foglio .tema-personalizzato').classList.contains('nascosto')})()"],
 ["js","(()=>{const c=document.querySelector('#foglio .interr-riservata'); evento('RISERVATA 1 1 0 0 0'); const acc=c.checked&&!c.disabled; evento('RISERVATA 0 0 0 0 0'); return 'RISIMP '+!!c+','+acc+','+c.disabled})()"],
 ["js","(()=>{document.querySelector('#foglio .indietro-rubrica').click(); return 'ESCIIMP ' + document.body.classList.contains('solo-rubrica') + ',' + document.getElementById('foglio').classList.contains('nascosto')})()"],
 ["vai","$U/?avvio=1#file"],["attendi",3],
 ["js","(()=>{window._xinv=[]; const v=S.ws.send.bind(S.ws); S.ws.send=(r)=>{ if(/^X(OK|NO|SEND|PAUSA|ANNULLA|VIA|PULISCI|CONF)/.test(r)) window._xinv.push(r); else v(r); }; const f=document.getElementById('foglio'), r=f.getBoundingClientRect(); return 'FILE1 '+document.body.classList.contains('solo-rubrica')+','+(r.top===0&&r.height>=innerHeight-1)+','+f.querySelectorAll('.xf-schede button').length+','+FileGrandi._stato().motoreOk+','+!!f.querySelector('.indietro-rubrica')})()"],
 ["js","(()=>{evento('XNEW abcdef0123456789 in $BOB 3145728 video/mp4 '+inHex('vacanze.mp4')+' chiede 0'); const r=document.querySelector('.xf-riga[data-id=abcdef0123456789]'); const b=[...r.querySelectorAll('.xf-azioni button')].map(x=>x.textContent).join('/'); [...r.querySelectorAll('.xf-azioni button')].find(x=>x.textContent==='Accetta').click(); return 'FILE2 '+r.querySelector('.xf-stato').textContent+'|'+b+'|'+window._xinv.includes('XOK abcdef0123456789')})()"],
 ["js","(()=>{evento('XNEW 0123456789abcdef out $BOB 26214400 application/zip '+inHex('archivio.zip')+' corre 5242880'); evento('XPROG 0123456789abcdef 7340032 26214400 1048576'); document.querySelector('.xf-schede [data-v=out]').click(); const r=[...document.querySelectorAll('.xf-riga')].find(x=>x.dataset.id==='0123456789abcdef'); return 'FILE3 '+r.querySelector('.xf-stato').textContent+'|'+r.querySelector('.xf-sotto').textContent})()"],
 ["foto","${NYCT_FOTO:-$TMP}/file-telefono-uscita.png"],
 ["js","(async()=>{await FileGrandi._sfoglia('$BOB',''); await new Promise(r=>setTimeout(r,500)); const radici=[...document.querySelectorAll('#foglio .xf-sfoglia .voce')].map(x=>x.textContent); [...document.querySelectorAll('#foglio .xf-sfoglia .voce')].find(x=>x.textContent.includes('galleria')).click(); await new Promise(r=>setTimeout(r,500)); [...document.querySelectorAll('#foglio .xf-sfoglia .voce')].find(x=>x.textContent.includes('Camera')).click(); await new Promise(r=>setTimeout(r,500)); const f=[...document.querySelectorAll('#foglio .xf-sfoglia .voce')].find(x=>x.textContent.includes('VID_prova')); f.click(); const bot=document.querySelector('#foglio .xf-invia').textContent; document.querySelector('#foglio .xf-invia').click(); const x=window._xinv.find(r=>r.startsWith('XSEND $BOB ')); const perc=x?new TextDecoder().decode(new Uint8Array(x.split(' ')[2].match(/../g).map(h=>parseInt(h,16)))):''; return 'SFOGLIA '+radici.join('/')+'|'+bot+'|'+perc.endsWith('/galleria/Camera/VID_prova.mp4')+'|'+(x||'').split(' ')[3]})()"],
 ["js","(()=>{const f=document.getElementById('foglio'), r=f.getBoundingClientRect(); return 'DOPOINVIO '+document.body.classList.contains('solo-rubrica')+','+(r.top===0)+','+!!f.querySelector('.file-pagina')+','+!!f.querySelector('.indietro-rubrica')+','+FileGrandi._stato().scheda})()"],
 ["js","(()=>{document.querySelector('.xf-schede [data-v=arch]').click(); return 'arch'})()"],["attendi",1.5],
 ["js","(()=>{const r=[...document.querySelectorAll('.xf-lista .xf-riga')].map(x=>x.querySelector('.xf-nome').textContent+':'+x.querySelector('.xf-sotto').textContent); return 'ARCH '+r.join(',')+'|'+document.querySelector('.xf-spazio').textContent.includes('Occupati 2,0 MB')})()"],
 ["foto","${NYCT_FOTO:-$TMP}/file-telefono-archivio.png"],
 ["js","(async()=>{const r=[...document.querySelectorAll('.xf-lista .xf-riga')].find(x=>x.querySelector('.xf-nome').textContent==='distro.iso'); [...r.querySelectorAll('button')].find(b=>b.textContent==='Elimina').click(); [...document.querySelectorAll('#dialogo button')].find(b=>b.textContent==='Elimina').click(); await new Promise(r=>setTimeout(r,1200)); const a=await (await fetch('/api/archivio')).json(); const fuori=await post('/api/elimina?p='+inHex('/etc/passwd')); const fuori2=await post('/api/condividi?p='+inHex('/etc/passwd')); const fuori3=await (await fetch('/api/sfoglia?d='+inHex('/etc'))).json(); return 'ELIMINA '+a.voci.map(v=>v.nome).join('/')+'|'+fuori.ok+'|'+fuori2.ok+'|'+(fuori3.errore||'')})()"],
 ["js","(()=>{chiudiFoglio(); const chiuso=document.getElementById('foglio').classList.contains('nascosto'); window.nyctApriFile(); const f=document.getElementById('foglio'); return 'RIENTRO '+chiuso+','+!f.classList.contains('nascosto')+','+!!f.querySelector('.file-pagina')+','+document.body.classList.contains('solo-rubrica')})()"],
 ["vai","$U/?avvio=3#file"],["attendi",3],
 ["js","(()=>{const f=document.getElementById('foglio'); return 'DAZERO '+!f.classList.contains('nascosto')+','+!!f.querySelector('.file-pagina')+','+document.body.classList.contains('solo-rubrica')})()"],
 ["js","(()=>{window._xinv=[]; const v=S.ws.send.bind(S.ws); S.ws.send=(r)=>{ if(/^X(VIA|PULISCI|CONF)/.test(r)) window._xinv.push(r); else v(r); }; evento('XNEW 1111222233334444 in $BOB 1048576 image/jpeg '+inHex('mare.jpg')+' fatto 1048576 '+inHex('/x/mare.jpg')); document.querySelector('.xf-schede [data-v=in]').click(); let r=[...document.querySelectorAll('.xf-riga')].find(x=>x.dataset.id==='1111222233334444'); const prima=[...r.querySelectorAll('.xf-azioni button')].map(b=>b.textContent).join('/'); evento('XNEW 1111222233334444 in $BOB 1048576 image/jpeg '+inHex('mare.jpg')+' eliminato 1048576 -'); r=[...document.querySelectorAll('.xf-riga')].find(x=>x.dataset.id==='1111222233334444'); const dopo=[...r.querySelectorAll('.xf-azioni button')].map(b=>b.textContent).join('/'); const st=r.querySelector('.xf-stato').textContent; [...r.querySelectorAll('.xf-azioni button')].find(b=>b.textContent==='✕').click(); const via=window._xinv.includes('XVIA 1111222233334444'); evento('XVIA 1111222233334444'); const sparita=![...document.querySelectorAll('.xf-riga')].some(x=>x.dataset.id==='1111222233334444'); return 'ELIM '+prima+'|'+st+'|'+dopo+'|'+via+','+sparita})()"],
 ["js","(()=>{[...document.querySelectorAll('.xf-barra-alta button')].find(b=>b.textContent==='🧹').click(); [...document.querySelectorAll('#dialogo button')].find(b=>b.textContent==='Pulisci').click(); [...document.querySelectorAll('.xf-barra-alta button')].find(b=>b.textContent==='⚙').click(); const c=document.querySelector('#dialogo input[type=checkbox]'); const testo=c.parentElement.textContent; c.checked=false; [...document.querySelectorAll('#dialogo button')].find(b=>b.textContent==='Salva').click(); const conf=window._xinv.find(r=>r.startsWith('XCONF soglia'))||''; return 'PULCONF '+window._xinv.includes('XPULISCI')+'|'+testo+'|'+conf.endsWith(' inchat 0')})()"],
 ["vai","$U/?avvio=4#terminale"],["attendi",3],
 ["js","(()=>{const f=document.getElementById('foglio'), r=f.getBoundingClientRect(); return 'TERM1 '+document.body.classList.contains('solo-rubrica')+','+(r.top===0)+','+!!f.querySelector('.term-uscita')+','+!!f.querySelector('.indietro-rubrica')+','+f.querySelectorAll('.term-rapido').length+','+(f.querySelector('.term-campo').getAttribute('autocapitalize')==='off')})()"],
 ["js","Terminale.esegui('io'); 'io'"],
 ["finche","document.querySelector('.term-uscita').textContent.includes(S.me) && document.querySelector('.term-ferma').disabled",20],
 ["js","'TERMIO '+document.querySelector('.term-uscita').textContent.split(String.fromCharCode(10)).includes(S.me)"],
 ["js","Terminale.esegui('aiuto'); 'aiuto'"],["attendi",0.5],
 ["foto","${NYCT_FOTO:-$TMP}/terminale-aiuto.png"],
 ["js","(()=>{const a=document.querySelector('.term-aiuto'); const voci=[...a.querySelectorAll('.term-aiuto-voce')]; const u=document.querySelector('.term-uscita'); const larghe=voci.filter(v=>v.scrollWidth>v.clientWidth+1).length; voci.find(v=>v.querySelector('.term-aiuto-cmd').textContent.startsWith('scrivi')).click(); const campo=document.querySelector('.term-campo').value; return 'TERMAIUTO '+voci.length+','+a.querySelectorAll('.term-aiuto-titolo').length+','+larghe+','+(u.scrollWidth<=u.clientWidth+1)+','+JSON.stringify(campo)+','+!a.textContent.includes('avvia')})()"],
 ["js","(()=>{document.querySelector('.term-campo').value=''; [...document.querySelectorAll('.term-aiuto-voce')].find(v=>v.querySelector('.term-aiuto-cmd').textContent==='io').click(); return 'tocco'})()"],
 ["finche","document.querySelector('.term-ferma').disabled && document.querySelector('.term-uscita').textContent.split(String.fromCharCode(10)).filter(r=>r===S.me).length>=2",20],
 ["js","'TERMTOCCO ok'"],
 ["js","Terminale.esegui('io \$(touch $H/iniezione) ; touch $H/iniezione2'); 'iniezione'"],["attendi",3],
 ["js","Terminale.esegui('nyctalus-cli avvia'); 'avvia'"],["attendi",1.5],
 ["js","'TERMAVVIA '+document.querySelector('.term-uscita').textContent.includes('sul telefono il motore lo accende')"],
 ["js","Terminale.esegui('ascolta'); 'ascolta'"],
 ["finche","document.querySelector('.term-uscita').textContent.includes('In ascolto')",20],
 ["js","'TERMCORSO '+!document.querySelector('.term-ferma').disabled"],
 ["foto","${NYCT_FOTO:-$TMP}/terminale-telefono.png"],
 ["js","document.querySelector('.term-ferma').click(); 'stop'"],
 ["finche","document.querySelector('.term-ferma').disabled",15],
 ["js","'TERMSTOP '+document.querySelector('.term-ferma').disabled"],
 ["js","(()=>{window._inv2=[]; const v=S.ws.send.bind(S.ws); S.ws.send=(r)=>{ if(/^CALL/.test(r)) window._inv2.push(r); else v(r); }; window.nyctRispondiChiamata('cccccccccccccccccccccccccccccccccccccccccccccccccccccccc.nyct'); evento('CALLIN cccccccccccccccccccccccccccccccccccccccccccccccccccccccc.nyct notif123'); return 'notifica'})()"],["attendi",3],
 ["js","(()=>{const s=Chiamate._stato(); const r='NOTIFRISP '+window._inv2.includes('CALLOK cccccccccccccccccccccccccccccccccccccccccccccccccccccccc.nyct notif123')+','+(s.c&&s.c.stato); evento('CALLEND cccccccccccccccccccccccccccccccccccccccccccccccccccccccc.nyct notif123'); return r})()"]]
EOF
timeout 420 python3 "$STRUMENTI/cdp.py" "$TMP/chrome" "$TMP/passi.json" > "$TMP/r.txt" 2>&1
[ -n "${NYCT_DIAG:-}" ] && cp "$TMP/r.txt" "$NYCT_DIAG" 2>/dev/null
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
[ "$(r INFO)" = "Inviato:oggi,Nella casella cifrata:oggi,Consegnato:oggi,Visualizzato:oggi" ] || [ "$(r INFO)" = "Inviato:si,Nella casella cifrata:oggi,Consegnato:oggi,Visualizzato:oggi" ] && pass "ℹ️ Info del messaggio: casella, consegna e visualizzazione con l'ora" || fail "info del messaggio: $(r INFO)"
r INFOMOT | grep -q "Consegnato:14 nov 2023 alle .*Visualizzato:non ancora || true$" && pass "Info con l'ora mandata dal motore (consegnato il 14 nov 2023), non ancora visualizzato; voce «Info» nel menu dei propri messaggi" || fail "info dal motore: $(r INFOMOT)"
[ "$(r SFILZA)" = "0,1,true" ] && pass "errori ripetuti: un solo avviso (motore giù → barra in alto, non 30 righe in chat)" || fail "sfilza di errori: $(r SFILZA)"
[ "$(r RETE)" = "pronto,true,false" ] && pass "impostazioni: rete «attiva», «Avvia» spento e «Ferma» acceso" || fail "pulsanti della rete: $(r RETE)"
[ "$(r SOLO)" = "true,true,true,true" ] && pass "riquadro «Rubrica» (#rubrica): la sola rubrica a tutto schermo, con «←»" || fail "sola rubrica: $(r SOLO)"
[ "$(r SENZA)" = "true" ] && pass "rete senza rubrica: lo dice e propone «Riprova» (niente scritta ferma)" || fail "rubrica su rete senza rubrica: $(r SENZA)"
[ "$(r UTENTI)" = "2" ] && pass "rubrica: elenco degli iscritti disegnato" || fail "rubrica: $(r UTENTI)"
[ "$(r SCRIVI)" = "false,true,true" ] && pass "dalla rubrica, scegliendo qualcuno si esce dalla rubrica e si apre la conversazione" || fail "scrivi dalla rubrica: $(r SCRIVI)"
[ "$(r DALLACHAT)" = "true,false,false" ] && pass "☰ della chat: impostazioni in finestra, chiudendole si resta nella chat (non si torna alla home dell'app)" || fail "impostazioni dalla chat: $(r DALLACHAT)"
[ "$(r SOLOIMP)" = "true,true,true,2,true,true" ] && pass "⚙️ dell'app (#impostazioni): le impostazioni COMPLETE a tutto schermo (Contact ID, tema, sfondo, rete), con «←»" || fail "impostazioni a schermo intero: $(r SOLOIMP)"
[ "$(r MENTA)" = "menta,#091813" ] && pass "tema nuovo «Menta» (verde): stessi colori calcolati sul PC" || fail "tema Menta: $(r MENTA)"
[ "$(r PERS)" = "personalizzato,true,true,true,true" ] && pass "tema Personalizzato: codice #ff8800 + Chiaro → tavolozza ricavata (selettore di colore allineato)" || fail "tema personalizzato: $(r PERS)"
[ "$(r RIPR)" = "notte,#3ecf9a,true,true,🌙 Notte,true" ] && pass "↺ Ripristina: personalizzato come mai toccato (colore iniziale) e tema Notte" || fail "ripristino del personalizzato: $(r RIPR)"
[ "$(r NOTTE)" = "|true" ] && pass "tornando a un tema pronto, i colori personalizzati si tolgono" || fail "ritorno a Notte: $(r NOTTE)"
[ "$(r FILE1)" = "true,true,3,true,true" ] && pass "📦 File dall'app (#file): schermata intera con In arrivo · In uscita · Archivio, motore che conosce i trasferimenti" || fail "schermata File: $(r FILE1)"
[ "$(r FILE2)" = "Vuole mandarti questo file|Rifiuta/Accetta|true" ] && pass "proposta in arrivo: «Vuole mandarti questo file», Rifiuta/Accetta → XOK al motore" || fail "proposta in arrivo: $(r FILE2)"
r FILE3 | grep -q "^7,0 MB · 1,0 MB/s · mancano 1[89] s|a .* · 25,0 MB$" && pass "in uscita: avanzamento, velocità e tempo che manca" || fail "in uscita: $(r FILE3)"
r SFOGLIA | grep -q "galleria|Invia (1)|true|video/mp4$" && pass "sfoglia le cartelle del telefono → Invia: parte il percorso del file ORIGINALE (niente copie)" || fail "sfoglia: $(r SFOGLIA)"
[ "$(r DOPOINVIO)" = "true,true,true,true,out" ] && pass "dopo «Invia» si torna alla schermata File a tutto schermo, su «In uscita» (non alla pagina iniziale)" || fail "dopo l'invio: $(r DOPOINVIO)"
[ "$(r ELIM)" = "Apri/✕|Eliminato dall'archivio|✕|true,true" ] && pass "file arrivato e poi eliminato: «Eliminato dall'archivio», niente «Apri»; ✕ lo toglie dall'elenco (XVIA)" || fail "eliminato/togli: $(r ELIM)"
[ "$(r PULCONF)" = "true|Mostra i file anche nella conversazione della chat|true" ] && pass "🧹 Pulisci elenco (XPULISCI) e interruttore «Mostra i file anche nella chat» (XCONF … inchat 0)" || fail "pulisci/interruttore: $(r PULCONF)"
[ "$(r RIENTRO)" = "true,true,true,true" ] && pass "uscendo da File e rientrando si vede di nuovo File (non la chat)" || fail "rientro in File: $(r RIENTRO)"
[ "$(r DAZERO)" = "true,true,true" ] && pass "app aperta direttamente su File (#file, pagina appena caricata): si vede File, non la chat" || fail "apertura diretta su File: $(r DAZERO)"
r ARCH | grep -q "^distro.iso:2,0 MB,nota.pdf:4,0 KB,.*|true$" && pass "Archivio privato: file ricevuti dal più grande, spazio occupato" || fail "archivio: $(r ARCH)"
r ELIMINA | grep -q "^nota.pdf/.*|false|false|cartella non consentita$" && pass "Elimina toglie il file; Elimina/Condividi/Sfoglia rifiutano ciò che sta fuori (es. /etc)" || fail "elimina e confini: $(r ELIMINA)"
[ "$(r ESCIIMP)" = "false,true" ] && pass "«←» chiude le impostazioni (nell'app si torna alla schermata iniziale)" || fail "uscita dalle impostazioni: $(r ESCIIMP)"
[ "$(r TERM1)" = "true,true,true,true,6,true" ] && pass "⌨ Terminale dall'app (#terminale): schermata intera, pulsanti rapidi, campo senza maiuscola automatica, «←»" || fail "schermata Terminale: $(r TERM1)"
[ "$(r TERMIO)" = "true" ] && pass "Terminale: «io» eseguito da nyctalus-cli in Termux, l'output torna nella schermata" || fail "Terminale io: $(r TERMIO)"
[ "$(r TERMAIUTO)" = '16,5,0,true,"scrivi ",true' ] && pass "Terminale: «aiuto» è una scheda per il telefono (16 comandi in 5 gruppi, niente righe spezzate né scorrimento in orizzontale); toccando «scrivi» lo prepara nel campo" || fail "Terminale aiuto: $(r TERMAIUTO)"
[ "$(r TERMTOCCO)" = "ok" ] && pass "Terminale: toccando «io» nell'aiuto si esegue subito" || fail "Terminale tocco su io"
[ ! -e "$H/iniezione" ] && [ ! -e "$H/iniezione2" ] && pass "Terminale: \$(…) e ; NON vengono eseguiti (solo nyctalus-cli, argomenti senza shell)" || fail "Terminale: comando iniettato eseguito!"
[ "$(r TERMAVVIA)" = "true" ] && pass "Terminale: «avvia» rifiutato (il motore lo accende l'app)" || fail "Terminale avvia: $(r TERMAVVIA)"
[ "$(r TERMCORSO)" = "true" ] && [ "$(r TERMSTOP)" = "true" ] && pass "Terminale: «ascolta» resta in diretta, ⏹ lo ferma" || fail "Terminale ascolta/stop: $(r TERMCORSO) $(r TERMSTOP)"
COD=$(curl -s -o /dev/null --max-time 3 -w '%{http_code}' -b "nyct=$TOK" -H "Origin: http://altro.sito" -H "Upgrade: websocket" -H "Connection: Upgrade" -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" -H "Sec-WebSocket-Version: 13" "$U/ws/terminale")
[ "$COD" = "403" ] && pass "Terminale: un'origine estranea viene rifiutata (403)" || fail "origine estranea: $COD"
[ "$(stat -c %a "$H/.nyctalus-chat/controllo" 2>/dev/null)" = "600" ] && [ "$(HOME="$H" python3 "$ROOT/pc/cli/nyctalus-cli" io 2>&1)" = "$(HOME="$H" python3 "$ROOT/pc/cli/nyctalus-cli" io 2>&1 | grep -oE '^[a-z2-7]{56}\.nyct$')" ] && HOME="$H" python3 "$ROOT/pc/cli/nyctalus-cli" io | grep -qE '\.nyct$' \
  && pass "nyctalus-cli anche da Termux a mano: collegamento in ~/.nyctalus-chat/controllo (privato, 600)" || fail "collegamento per nyctalus-cli: $(ls -l "$H/.nyctalus-chat/controllo" 2>&1)"
[ "$(r RISIMP)" = "true,true,true" ] && pass "impostazioni: interruttore «🕶 Modalità riservata» che segue il motore (spento e grigio se la rete non ha la casella)" || fail "modalità riservata nelle impostazioni: $(r RISIMP)"
[ "$(r NOTIFRISP)" = "true,attiva" ] && pass "«Rispondi» nella notifica della chiamata: appena la chiamata compare, l'app risponde da sola (CALLOK)" || fail "risposta dalla notifica: $(r NOTIFRISP)"
RIF=$(curl -s -X POST -b "nyct=$TOK" "$U/api/chiamata/rifiuta?da=cccccccccccccccccccccccccccccccccccccccccccccccccccccccc.nyct&id=abc123")
BAD=$(curl -s -o /dev/null -w '%{http_code}' -X POST -b "nyct=$TOK" "$U/api/chiamata/rifiuta?da=x&id=abc;rm")
NOC=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$U/api/chiamata/rifiuta?da=$BOB&id=abc123")
[ "$RIF" = '{"ok": true}' ] && [ "$BAD" = 400 ] && [ "$NOC" = 403 ] && pass "«Rifiuta» nella notifica: il servizio manda CALLNO al motore; parametri strani → 400, senza il segreto dell'app → 403" || fail "rifiuto dalla notifica: $RIF $BAD $NOC"
[ "$KO" -eq 0 ]
