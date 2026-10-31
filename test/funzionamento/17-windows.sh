#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Windows, simulato su Linux: il programma d'avvio accende il servizio della chat (qui col
# motore Linux al posto di nyctalus.exe) e dà l'indirizzo della finestra d'avvio, che si prova
# in Chrome: stato «pronto», riquadri, Rete, guida di Firefox, Esci. Poi il contenuto dello zip.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · Windows (simulato)"
assicura_binario
P="$TMP/pacchetto"; mkdir -p "$P/windows"
cp -r "$ROOT/mobile/orchestratore" "$ROOT/mobile/webapp" "$P/"; cp "$ROOT/pc/windows/nyctalus-windows.py" "$P/windows/"
H="$TMP/home"; mkdir -p "$H/.nyctalus-mobile"
echo '{"porte": {"http": 20931, "controllo": 20932, "backend": 20933, "socks": 20934, "socks_locale": 20935, "monitor": 20936, "nodi": [20940,20941,20942,20943], "socks_browser": 20937}}' > "$H/.nyctalus-mobile/config.json"
URL=$(HOME="$H" NYCTALUS_BIN="$BIN" NYCTALUS_NON_APRIRE=1 timeout 40 python3 "$P/windows/nyctalus-windows.py")
OPID=$(pgrep -f -- "$P/orchestratore/nyctalus_mobile.py" | head -1); [ -n "$OPID" ] && PIDS+=($OPID)
[[ "$URL" =~ ^http://127\.0\.0\.1:20931/avvio\.html\?t=[0-9a-f]{32}$ ]] && pass "avvio: servizio acceso e indirizzo della finestra (con il segreto)" || fail "avvio: «$URL»"
URL2=$(HOME="$H" NYCTALUS_BIN="$BIN" NYCTALUS_NON_APRIRE=1 timeout 20 python3 "$P/windows/nyctalus-windows.py")
[ "$(pgrep -fc -- "$P/orchestratore/nyctalus_mobile.py")" = 1 ] && [ "$URL2" = "$URL" ] && pass "secondo doppio clic: riapre la finestra, non accende un secondo servizio" || fail "doppio avvio"
if command -v google-chrome >/dev/null; then
  cat > "$TMP/passi.json" <<JSON
[["vai","$URL"],["finche","document.getElementById('stato').classList.contains('pronto')",60],
 ["js","'PRONTO ' + !document.getElementById('t-chat').disabled + ',' + !document.getElementById('t-rubrica').disabled"],
 ["finche","document.getElementById('rete').textContent.indexOf('…') < 0",10],
 ["js","'RETE ' + document.getElementById('rete').textContent"],
 ["js","document.getElementById('t-browser').click(); 'FIREFOX ' + document.getElementById('foglio').textContent.includes('20937')"],
 ["foto","${NYCT_FOTO:-$TMP}/avvio-windows.png"],
 ["js","document.getElementById('velo').click(); document.getElementById('rete').click(); 'x'"],["attendi",1],
 ["js","'IMPORTA ' + !!document.querySelector('#foglio textarea')"],
 ["js","(async()=>{const r=await fetch('/api/esci',{method:'POST'}); return 'ESCI '+r.status})()"]]
JSON
  timeout 120 python3 "$STRUMENTI/cdp.py" "$TMP/chrome" "$TMP/passi.json" > "$TMP/r.txt" 2>&1
  r() { grep -oE "^js → $1 .*" "$TMP/r.txt" | head -1 | cut -d' ' -f4-; }
  [ "$(r PRONTO)" = "true,true" ] && pass "finestra d'avvio: «pronto» e riquadri Chat/Rubrica accesi" || fail "pronto: $(r PRONTO) $(tail -3 "$TMP/r.txt")"
  r RETE | grep -q "prova locale" && pass "riga 🔗 Rete: rete di prova finché non si importa un codice" || fail "rete: $(r RETE)"
  [ "$(r FIREFOX)" = "true" ] && pass "🌐 Browser: guida di Firefox con la porta giusta" || fail "guida Firefox"
  [ "$(r IMPORTA)" = "true" ] && pass "🔗 Rete: incolla il codice di rete" || fail "importa"
  sleep 2
  pgrep -f -- "$P/orchestratore/nyctalus_mobile.py" > /dev/null && fail "⏻ Esci: il servizio è ancora acceso" || pass "⏻ Esci: spegne il servizio"
  [ "$(pgrep -fc -- "--data $H/.nyctalus-chat")" = 0 ] && pass "nessun processo del motore rimasto" || fail "processi rimasti"
else
  salta "google-chrome non installato: finestra d'avvio non provata"
fi
Z="$ROOT/installazioni/Windows/Nyctalus-Windows.zip"
if [ -f "$Z" ]; then
  python3 - "$Z" <<'PY' && pass "zip: Nyctalus.cmd, Python incluso, nyctalus.exe, servizio, finestra d'avvio, manuale" || fail "zip incompleto"
import sys, zipfile
n = set(zipfile.ZipFile(sys.argv[1]).namelist())
for f in ("Nyctalus.cmd", "python/pythonw.exe", "python/python312.zip", "bin/nyctalus.exe", "webapp/avvio.html",
          "webapp/avvio.js", "orchestratore/nyctalus_mobile.py", "windows/nyctalus-windows.py", "Manuali/MANUALE-WINDOWS.md"):
    assert "Nyctalus-Windows/" + f in n, f
PY
  unzip -l "$Z" | grep -qiE "identita|token|\.nyctalus-|privato" && fail "zip: dati personali dentro" || pass "zip: nessun dato personale"
else
  salta "pacchetto Windows non preparato (servono nyctalus.exe e Python embeddable in target/)"
fi
[ "$KO" -eq 0 ]
