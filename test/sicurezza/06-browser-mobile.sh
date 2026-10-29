#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Browser dell'app in modalità Nyctalus: l'inoltratore SOCKS dell'orchestratore
# (pagina di verifica, niente indirizzi locali, niente UDP, rete non pronta = niente
# scorciatoie) e le regole nel codice del browser (processo separato, chiuso in caso di
# errore, .nyct solo in rete, niente uscite verso altre app, WebRTC spento).
. "$(dirname "$0")/../lib.sh"
titolo "Sicurezza · browser del telefono in modalità Nyctalus"
assicura_binario
J="$ROOT/mobile/android/app/src/main/java/org/nyctalus/mobile"; B="$J/BrowserActivity.java"; M="$ROOT/mobile/android/app/src/main/AndroidManifest.xml"

# --- codice ------------------------------------------------------------------------------
grep -A3 'android:name=".BrowserActivity"' "$M" | grep -q 'android:process=":browser"' \
  && pass "browser in un processo separato (il proxy non tocca la chat)" || fail "browser nello stesso processo"
! grep -qE 'permission\.(CAMERA|RECORD_AUDIO|ACCESS_FINE_LOCATION)' "$M" && pass "nessun permesso di fotocamera, microfono o posizione" || fail "permessi sensibili nel manifest"
grep -q 'nyctalus-ok /" + gettone' "$B" && grep -q 'chiudi.accept(false), 20000' "$B" \
  && pass "verifica col gettone casuale prima di aprire pagine (e scadenza)" || fail "verifica della modalità Nyctalus"
grep -q 'if (eNyct(u) && !nyctalus)' "$B" && grep -q 'shouldInterceptRequest' "$B" \
  && pass ".nyct bloccati fuori dalla modalità Nyctalus (anche dentro le pagine)" || fail ".nyct fuori dalla rete"
grep -q 'Bloccato in modalità Nyctalus: ' "$B" && pass "in modalità Nyctalus nessun indirizzo passa ad altre app" || fail "schemi esterni non bloccati"
grep -q 'RTCPeerConnection' "$B" && grep -q 'scriptAllAvvio(v, JS_NO_WEBRTC)' "$B" && pass "WebRTC spento con script all'avvio della pagina" || fail "WebRTC"
grep -q 'if (nyctalus) { richiesta.deny(); return; }' "$B" && pass "fotocamera/microfono negati in modalità Nyctalus" || fail "permessi media"
grep -q 'if (nyctalus) { scaricaInRete' "$B" && grep -q 'apri(new URL(TRADUCI_URL))' "$B" \
  && pass "download e traduzione passano dalla rete (non dal sistema)" || fail "connessioni fuori rete"
grep -q '"<-loopback>"' "$J/ProxyNyctalus.java" && pass "proxy anche per 127.0.0.1 (i siti non toccano i servizi del telefono)" || fail "loopback esclusa dal proxy"
[ "$(grep -c 'openConnection()' "$B")" -le 1 ] && pass "nessun'altra connessione diretta nel codice del browser" || fail "connessioni dirette: $(grep -n 'openConnection()' "$B")"

# --- inoltratore --------------------------------------------------------------------------
H="$TMP/home"; mkdir -p "$H/.nyctalus-mobile"
echo '{"porte": {"http": 19821, "controllo": 19822, "backend": 19823, "socks": 19824, "socks_locale": 19825, "monitor": 19826, "nodi": [19827,19828,19829,19830], "socks_browser": 19831}}' > "$H/.nyctalus-mobile/config.json"
HOME="$H" NYCTALUS_BIN="$BIN" NYCTALUS_AVVIO_AUTOMATICO=0 python3 "$ROOT/mobile/orchestratore/nyctalus_mobile.py" > "$TMP/o.log" 2>&1 & PIDS+=($!)
for _ in $(seq 1 40); do ss -ltn | grep -q ":19831 " && break; sleep 0.2; done
S() { curl -s --max-time 15 --socks5-hostname 127.0.0.1:19831 "$@"; }
G=$(python3 -c 'import secrets;print(secrets.token_hex(8))')
S "http://verifica.nyctalus.invalid/$G" | grep -q "nyctalus-ok /$G" && pass "pagina di verifica risponde col gettone" || fail "pagina di verifica"
S -o /dev/null https://example.com/; [ $? -ne 0 ] && pass "rete non pronta: nessuna connessione (niente scorciatoie dirette)" || fail "connessione diretta con la rete ferma (grave)"
python3 - <<'PY' && pass "UDP (ASSOCIATE) rifiutato: niente traffico fuori dal tunnel" || fail "UDP accettato"
import socket, sys
s = socket.create_connection(("127.0.0.1", 19831), timeout=5)
s.sendall(b"\x05\x01\x00"); assert s.recv(2) == b"\x05\x00"
s.sendall(b"\x05\x03\x00\x01\x00\x00\x00\x00\x00\x00")
sys.exit(0 if s.recv(10)[1] != 0 else 1)
PY
# rete pronta (prova locale), poi indirizzi locali e un sito vero
TOK=$(cat "$H/.nyctalus-mobile/token")
curl -s -X POST -H "Cookie: nyct=$TOK" -H "Origin: http://127.0.0.1:19821" http://127.0.0.1:19821/api/servizi/avvia > /dev/null
attendi_regex "$TMP/o.log" 'servizi pronti' 40 > /dev/null
for dest in http://127.0.0.1:19821/ http://localhost:19821/ http://192.168.1.1/ http://10.0.0.1/ "http://[::1]:19821/"; do
  S -o /dev/null "$dest"; rc=$?
  [ "$rc" -ne 0 ] && pass "indirizzo locale rifiutato: $dest" || fail "indirizzo locale raggiunto: $dest (grave)"
done
C=$(S -o /dev/null -w '%{http_code}' https://example.com/)
if [ "$C" = 200 ]; then pass "sito vero raggiunto attraverso la rete di prova (HTTPS)"
else salta "sito esterno non raggiungibile da qui (rete dell'ufficio?): codice $C"; fi
[ "$KO" -eq 0 ]
