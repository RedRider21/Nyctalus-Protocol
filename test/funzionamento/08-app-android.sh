#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# App Android (APK): controlli sul manifest e sul codice, simulazione sul PC del
# comando che l'app fa eseguire a Termux (segreto nuovo → il servizio già acceso lo
# accetta, il vecchio no), e compilazione se c'è un Android SDK (ANDROID_HOME).
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · app Android (APK)"
A="$ROOT/mobile/android/app/src/main"; M="$A/AndroidManifest.xml"; J="$A/java/org/nyctalus/mobile/MainActivity.java"

! grep -q 'permission.CAMERA' "$M" && pass "nessun permesso fotocamera (usa l'app del telefono)" || fail "chiede il permesso fotocamera"
grep -q 'allowBackup="false"' "$M" && pass "backup dell'app disattivato (il segreto non finisce nei backup)" || fail "allowBackup"
grep -A3 'FotoProvider' "$M" | grep -q 'exported="false"' && pass "FotoProvider non esportato (scatti privati)" || fail "FotoProvider esportato"
grep -q 'SecureRandom' "$J" && pass "segreto generato con SecureRandom" || fail "segreto non casuale"
grep -q '"127.0.0.1".equals(u.getHost())' "$J" && pass "solo il servizio locale resta nell'app" || fail "navigazione non limitata"
python3 - "$ROOT/mobile/android/app/src/main/java/org/nyctalus/mobile/FotoProvider.java" <<'PY' && pass "FotoProvider rifiuta nomi con percorsi (../)" || fail "FotoProvider: nomi non filtrati"
import re, sys
s = open(sys.argv[1]).read()
rx = re.search(r'nome\.matches\("([^"]+)"\)', s).group(1).replace("\\\\", "\\")
ok = lambda n: re.fullmatch(rx, n) and not n.startswith(".")
assert ok("IMG_1.jpg") and ok("VID_2.mp4")
assert not any(ok(n) for n in ["../token", "a/b.jpg", "..", ".x", "x" * 65])
PY

# --- il comando che l'app fa eseguire a Termux, ricostruito dal codice Java ---
CMD=$(python3 - "$J" <<'PY'
import re, sys
s = open(sys.argv[1]).read()
corpo = re.search(r'private String comandoAvvio\(\) \{(.*?)\}', s, re.S).group(1)
pezzi = re.findall(r'"((?:[^"\\]|\\.)*)"|\b(segreto)\b', corpo)
print("".join(p if p else "@SEGRETO@" for p, v in pezzi).replace('\\"', '"'))
PY
)
[ -n "$CMD" ] && pass "comando di avvio estratto" || { fail "comando di avvio non trovato"; exit 1; }
H="$TMP/home"; F="$TMP/finti"; mkdir -p "$H/.nyctalus-mobile" "$F"
VECCHIO=vecchiosegreto0123456789abcdef01; NUOVO=$(python3 -c 'import secrets;print(secrets.token_hex(16))')
printf %s "$VECCHIO" > "$H/.nyctalus-mobile/token"
echo '{"porte": {"http": 19733}}' > "$H/.nyctalus-mobile/config.json"
printf '#!/bin/sh\necho "$@" >> "%s/avvii.txt"\n' "$TMP" > "$F/nyctalus-mobile"; chmod +x "$F/nyctalus-mobile"
HOME="$H" NYCTALUS_AVVIO_AUTOMATICO=0 python3 "$ROOT/mobile/orchestratore/nyctalus_mobile.py" > "$TMP/o.log" 2>&1 &
PIDS+=($!)
codice() { python3 - "$1" <<'PY'
import sys, urllib.request, urllib.error
r = urllib.request.Request("http://127.0.0.1:19733/api/stato", headers={"Cookie": "nyct=" + sys.argv[1]})
try: print(urllib.request.urlopen(r, timeout=3).status)
except urllib.error.HTTPError as e: print(e.code)
except Exception: print(0)
PY
}
for i in $(seq 1 50); do [ "$(codice "$VECCHIO")" = 200 ] && break; sleep 0.2; done
[ "$(codice "$VECCHIO")" = 200 ] && pass "servizio acceso con il segreto di Termux" || { fail "servizio non partito: $(tail -2 "$TMP/o.log")"; exit 1; }
HOME="$H" PATH="$F:$PATH" bash -c "${CMD//@SEGRETO@/$NUOVO}" && pass "comando dell'app eseguito" || fail "comando dell'app fallito"
[ "$(cat "$H/.nyctalus-mobile/token")" = "$NUOVO" ] && pass "segreto dell'app scritto in ~/.nyctalus-mobile/token" || fail "segreto non scritto"
[ "$(stat -c %a "$H/.nyctalus-mobile/token")" = 600 ] && [ "$(stat -c %a "$H/.nyctalus-mobile")" = 700 ] && pass "permessi privati (600 file, 700 cartella)" || fail "permessi del segreto"
grep -qx -- "--sfondo" "$TMP/avvii.txt" 2>/dev/null && pass "servizio avviato con nyctalus-mobile --sfondo" || fail "nyctalus-mobile non chiamato"
sleep 1.1
[ "$(codice "$NUOVO")" = 200 ] && pass "il servizio già acceso accetta il segreto nuovo dell'app" || fail "segreto nuovo rifiutato"
[ "$(codice "$VECCHIO")" != 200 ] && pass "il segreto vecchio non vale più" || fail "segreto vecchio ancora valido"
[ "$(codice "")" != 200 ] && pass "senza segreto: accesso negato" || fail "accesso senza segreto"

# --- compilazione (solo se c'è un Android SDK) ---
if [ -d "${ANDROID_HOME:-/nessuno}/build-tools" ]; then
  APK="$ROOT/mobile/android/Nyctalus.apk"
  bash "$ROOT/strumenti/compila-apk.sh" > "$TMP/c.log" 2>&1 && [ -s "$APK" ] && pass "APK compilata e firmata ($(du -h "$APK" | cut -f1))" || fail "compilazione APK: $(tail -3 "$TMP/c.log")"
else
  salta "compilazione APK: nessun Android SDK (imposta ANDROID_HOME)"
fi
[ "$KO" -eq 0 ]
