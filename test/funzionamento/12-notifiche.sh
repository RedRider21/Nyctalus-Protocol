#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Notifiche dei messaggi con l'app chiusa, senza Google: il servizio del telefono offre il
# flusso /api/notifiche (solo con il segreto dell'app) e l'app Android ha il servizio che
# lo ascolta, con permessi e interruttore. La chat del PC avvisa se la finestra non è attiva.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · notifiche dei messaggi"
assicura_binario
H="$TMP/home"; mkdir -p "$H/.nyctalus-mobile"
TOK=segreto0123456789abcdef0123456789; printf %s "$TOK" > "$H/.nyctalus-mobile/token"
cat > "$H/.nyctalus-mobile/config.json" <<'JSON'
{"porte": {"http": 19735, "controllo": 19791, "backend": 19782, "socks": 19780, "socks_locale": 19786,
           "monitor": 19796, "nodi": [19740, 19741, 19742, 19743]}}
JSON
HOME="$H" NYCTALUS_BIN="$BIN" python3 "$ROOT/mobile/orchestratore/nyctalus_mobile.py" > "$TMP/o.log" 2>&1 & PIDS+=($!)
[ -n "$(attendi_regex "$TMP/o.log" 'servizi pronti' 40)" ] || { fail "servizi non avviati: $(tail -2 "$TMP/o.log")"; exit 1; }
python3 "$STRUMENTI/prova_notifiche.py" 19735 "$TOK" > "$TMP/r.txt" 2>&1 || echo "KO prova interrotta: $(tail -1 "$TMP/r.txt")" >> "$TMP/r.txt"
while read -r esito resto; do
  case "$esito" in OK) pass "$resto" ;; KO) fail "$resto" ;; esac
done < "$TMP/r.txt"
M="$ROOT/mobile/android/app/src/main/AndroidManifest.xml"; J="$ROOT/mobile/android/app/src/main/java/org/nyctalus/mobile"
grep -q 'POST_NOTIFICATIONS' "$M" && grep -q 'ServizioNotifiche' "$M" && grep -A3 'ServizioNotifiche' "$M" | grep -q 'exported="false"' \
  && pass "app: servizio delle notifiche dichiarato (non esportato) con il permesso di Android 13" || fail "manifest delle notifiche"
grep -q 'FOREGROUND_SERVICE_TYPE_REMOTE_MESSAGING' "$J/ServizioNotifiche.java" && grep -q 'appInPrimoPiano) return' "$J/ServizioNotifiche.java" \
  && pass "app: servizio di messaggistica; nessuna notifica mentre l'app è aperta" || fail "servizio delle notifiche"
grep -q 'a.matches("\[a-z2-7\]{56}' "$J/MainActivity.java" && grep -q 'nyctApriChat' "$J/MainActivity.java" && grep -q 'window.nyctApriChat' "$ROOT/mobile/webapp/app.js" \
  && pass "toccando la notifica si apre quella conversazione (indirizzo controllato)" || fail "apertura dalla notifica"
grep -q 'cambiaNotifiche' "$J/MainActivity.java" && pass "interruttore «Notifiche» in Strumenti" || fail "interruttore delle notifiche"
grep -q '_notifica_desktop' "$ROOT/pc/chat/nyctalus_chat/app.py" && pass "PC: notifica sul desktop se la finestra della chat non è attiva" || fail "notifica del PC"
[ "$KO" -eq 0 ]
