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

# --- «Installa / aggiorna Nyctalus in Termux», in un Termux simulato ---
# Il comando è ricostruito dal codice Java; il pacchetto è servito come lo serve l'app
# (127.0.0.1, indirizzo casuale) e l'impronta SHA-256 deve corrispondere.
assicura_binario
python3 - "$J" > "$TMP/inst.tpl" <<'PY'
import re, sys
s = open(sys.argv[1]).read()
corpo = re.search(r'String cmd = (.*?);\n', s, re.S).group(1)
avvio = re.search(r'private String comandoAvvio\(\) \{(.*?)\}', s, re.S).group(1)
def unisci(c):
    out = []
    for lit, ident in re.findall(r'"((?:[^"\\]|\\.)*)"|([A-Za-z_][\w.]*(?:\(\))?)', c):
        if lit or (not ident): out.append(lit.replace('\\"', '"').replace("\\\\", "\\"))
        elif ident == "sp.indirizzo()": out.append("@URL@")
        elif ident == "sp.impronta": out.append("@HASH@")
        elif ident == "comandoAvvio()": out.append(unisci(avvio.split("return", 1)[1]))
        elif ident == "segreto": out.append("@SEGRETO@")
    return "".join(out)
print(unisci(corpo))
PY
grep -q "installa.sh" "$TMP/inst.tpl" && pass "comando d'installazione estratto" || fail "comando d'installazione non trovato"
PK="$TMP/pk"; mkdir -p "$PK/nyctalus-termux/bin"
cp -r "$ROOT/mobile/orchestratore" "$ROOT/mobile/webapp" "$PK/nyctalus-termux/"
cp "$ROOT/mobile/termux/installa.sh" "$ROOT/mobile/termux/disinstalla.sh" "$ROOT/mobile/termux/nyctalus-mobile" "$PK/nyctalus-termux/"
cp "$BIN" "$PK/nyctalus-termux/bin/nyctalus"
( cd "$PK" && tar czf servito.tar.gz nyctalus-termux )
HASH=$(sha256sum "$PK/servito.tar.gz" | cut -d' ' -f1); CASUALE=$(python3 -c 'import secrets;print(secrets.token_hex(16))')
mkdir -p "$PK/web" && cp "$PK/servito.tar.gz" "$PK/web/$CASUALE"
python3 -m http.server 19735 --bind 127.0.0.1 --directory "$PK/web" > /dev/null 2>&1 & PIDS+=($!)
H2="$TMP/home2"; P2="$TMP/usr2"; F2="$TMP/finti2"; mkdir -p "$H2/.termux" "$P2/bin" "$F2"
printf 'extra-keys = [[ESC]]\n' > "$H2/.termux/termux.properties"
printf '#!/bin/sh\necho "$*" >> "%s/pkg.txt"\n' "$TMP" > "$F2/pkg"
printf '#!/bin/sh\n[ "$1" = "-s" ] || exit 1\ncase "$2" in python|termux-tools) exit 0 ;; *) exit 1 ;; esac\n' > "$F2/dpkg"
printf '#!/bin/sh\nexec python3 "$@"\n' > "$F2/python"
printf '#!/bin/sh\necho "$*" >> "%s/am.txt"\n' "$TMP" > "$F2/am"
printf '#!/bin/sh\necho "--sfondo" >> "%s/avvii2.txt"\n' "$TMP" > "$F2/nyctalus-mobile"
chmod +x "$F2"/*
SEG2=$(python3 -c 'import secrets;print(secrets.token_hex(16))')
istanza() { sed -e "s#@URL@#http://127.0.0.1:19735/$1#" -e "s#@HASH@#$2#" -e "s#@SEGRETO@#$SEG2#" "$TMP/inst.tpl"; }
sleep 0.5
env HOME="$H2" PREFIX="$P2" PATH="$F2:$PATH" bash -c "$(istanza "$CASUALE" 0000000000000000000000000000000000000000000000000000000000000000)" > "$TMP/manomesso.log" 2>&1
[ ! -e "$H2/nyctalus" ] && grep -q "non riuscita" "$TMP/manomesso.log" && pass "impronta diversa: pacchetto rifiutato, nulla installato" || fail "pacchetto manomesso accettato"
env HOME="$H2" PREFIX="$P2" PATH="$F2:$PATH" bash -c "$(istanza "$CASUALE" "$HASH")" > "$TMP/inst.log" 2>&1
[ -x "$H2/nyctalus/bin/nyctalus" ] && [ -f "$H2/nyctalus/webapp/index.html" ] && pass "Nyctalus installato in ~/nyctalus dal pacchetto dell'app" || fail "installazione dall'app: $(tail -3 "$TMP/inst.log")"
grep -q "allow-external-apps = true" "$H2/.termux/termux.properties" && grep -q "^extra-keys" "$H2/.termux/termux.properties" \
  && pass "allow-external-apps abilitato, resto di termux.properties intatto" || fail "termux.properties"
[ "$(cat "$H2/.nyctalus-mobile/token" 2>/dev/null)" = "$SEG2" ] && pass "segreto dell'app scritto dopo l'installazione" || fail "segreto non scritto dopo l'installazione"
grep -q -- "-n org.nyctalus.mobile/.MainActivity" "$TMP/am.txt" 2>/dev/null && pass "alla fine riporta in primo piano l'app" || fail "l'app non viene riaperta"
! grep -q upgrade "$TMP/pkg.txt" 2>/dev/null && pass "nessun pkg upgrade durante l'installazione" || fail "pkg upgrade lanciato"
T0=$(find /tmp -maxdepth 2 -name n.tar.gz -newer "$TMP/inst.tpl" 2>/dev/null | head -1)
[ -z "$T0" ] && pass "cartelle temporanee rimosse (anche dopo il rifiuto)" || fail "rimasto: $T0"

# --- compilazione (solo se c'è un Android SDK) ---
if [ -d "${ANDROID_HOME:-/nessuno}/build-tools" ]; then
  APK="$ROOT/mobile/android/Nyctalus.apk"
  bash "$ROOT/strumenti/compila-apk.sh" > "$TMP/c.log" 2>&1 && [ -s "$APK" ] && pass "APK compilata e firmata ($(du -h "$APK" | cut -f1))" || fail "compilazione APK: $(tail -3 "$TMP/c.log")"
  python3 -c "import zipfile,sys;sys.exit(0 if 'assets/nyctalus-termux.tar.gz' in zipfile.ZipFile(sys.argv[1]).namelist() else 1)" "$APK" \
    && pass "l'APK contiene il pacchetto per Termux" || fail "pacchetto Termux assente dall'APK"
else
  salta "compilazione APK: nessun Android SDK (imposta ANDROID_HOME)"
fi
[ "$KO" -eq 0 ]
