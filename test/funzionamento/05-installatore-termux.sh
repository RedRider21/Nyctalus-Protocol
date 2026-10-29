#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Installatore per Termux, in un Termux SIMULATO (HOME e PREFIX finti, pkg e
# dpkg finti che registrano le chiamate, "altri progetti" già presenti):
# convive con gli altri, si può rilanciare, e il disinstallatore toglie solo
# le cose di Nyctalus.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · installatore Termux (simulato)"
assicura_binario
H="$TMP/home"; P="$TMP/usr"; F="$TMP/finti"; PKG="$TMP/pkg-chiamate.txt"
mkdir -p "$H/.termux/boot" "$P/bin" "$F"
# "altri progetti" già presenti in Termux
printf 'extra-keys = [[ESC, TAB]]\nbell-character = ignore\n' > "$H/.termux/termux.properties"
printf '#!/bin/sh\necho altro progetto\n' > "$H/.termux/boot/altro-progetto"
printf '#!/bin/sh\necho altro comando\n' > "$P/bin/altro-comando"
chmod +x "$H/.termux/boot/altro-progetto" "$P/bin/altro-comando"
# finti strumenti di Termux
printf '#!/bin/sh\necho "$*" >> "%s"\n' "$PKG" > "$F/pkg"
printf '#!/bin/sh\n[ "$1" = "-s" ] || exit 1\ncase "$2" in python|termux-tools) exit 0 ;; *) exit 1 ;; esac\n' > "$F/dpkg"
printf '#!/bin/sh\nexec python3 "$@"\n' > "$F/python"
chmod +x "$F"/*
# l'archivio come lo produce crea-installazioni (col motore già pronto)
A="$TMP/nyctalus-termux"; mkdir -p "$A/bin"
cp -r "$ROOT/mobile/orchestratore" "$ROOT/mobile/webapp" "$A/"
cp "$ROOT/mobile/termux/installa.sh" "$ROOT/mobile/termux/disinstalla.sh" "$ROOT/mobile/termux/nyctalus-mobile" "$A/"
cp "$BIN" "$A/bin/nyctalus"
mkdir -p "$H/.nyctalus-mobile"
echo '{"porte": {"http": 19734, "controllo": 19691, "backend": 19682, "socks": 19680, "socks_locale": 19686, "monitor": 19696, "nodi": [19640,19641,19642,19643]}}' > "$H/.nyctalus-mobile/config.json"
T() { env HOME="$H" PREFIX="$P" PATH="$F:$PATH" "$@"; }

T bash "$A/installa.sh" > "$TMP/i1.log" 2>&1 && pass "installazione completata" || { fail "installazione fallita: $(tail -3 "$TMP/i1.log")"; exit 1; }
[ -x "$H/nyctalus/bin/nyctalus" ] && [ -f "$H/nyctalus/orchestratore/nyctalus_mobile.py" ] && [ -f "$H/nyctalus/webapp/index.html" ] \
  && pass "tutto dentro ~/nyctalus" || fail "file mancanti in ~/nyctalus"
[ "$(readlink "$P/bin/nyctalus-mobile")" = "$H/nyctalus/nyctalus-mobile" ] && pass "comando nyctalus-mobile creato" || fail "comando non creato"
[ ! -s "$PKG" ] && pass "nessun pkg install superfluo (era già tutto presente)" || fail "pkg chiamato inutilmente: $(cat "$PKG")"
grep -q "upgrade" "$PKG" 2>/dev/null && fail "è stato lanciato un pkg upgrade (vietato)" || pass "nessun pkg upgrade"
grep -q '^extra-keys' "$H/.termux/termux.properties" && grep -q '^bell-character' "$H/.termux/termux.properties" \
  && pass "termux.properties: impostazioni esistenti intatte" || fail "termux.properties alterato"
T bash "$A/installa.sh" > "$TMP/i2.log" 2>&1
[ "$(grep -c '^allow-external-apps' "$H/.termux/termux.properties")" = "1" ] && pass "seconda installazione: nessuna riga duplicata" || fail "righe duplicate in termux.properties"
[ ! -e "$H/.termux/boot/nyctalus-mobile" ] && pass "nessun avvio automatico se non richiesto" || fail "avvio automatico creato senza richiesta"
T bash "$A/installa.sh" --avvio-automatico > "$TMP/i3.log" 2>&1
[ -x "$H/.termux/boot/nyctalus-mobile" ] && pass "--avvio-automatico crea il SUO file di avvio" || fail "avvio automatico non creato"

# avvio / stato / arresto
T bash "$H/nyctalus/nyctalus-mobile" --sfondo
sleep 4
T bash "$H/nyctalus/nyctalus-mobile" --stato | grep -q "In esecuzione" && pass "nyctalus-mobile --sfondo / --stato" || fail "non risulta in esecuzione"
T bash "$H/nyctalus/nyctalus-mobile" --ferma > /dev/null; sleep 1.5
T bash "$H/nyctalus/nyctalus-mobile" --stato | grep -q "Fermo" && pass "nyctalus-mobile --ferma" || fail "non si è fermato"

# disinstallazione: dati conservati, altri progetti intatti
echo n | T bash "$H/nyctalus/disinstalla.sh" > "$TMP/d1.log" 2>&1
[ ! -e "$H/nyctalus" ] && [ ! -e "$P/bin/nyctalus-mobile" ] && [ ! -e "$H/.termux/boot/nyctalus-mobile" ] \
  && pass "disinstallato: programma, comando e avvio automatico tolti" || fail "disinstallazione incompleta"
[ -x "$H/.termux/boot/altro-progetto" ] && grep -q "altro progetto" "$H/.termux/boot/altro-progetto" \
  && [ -x "$P/bin/altro-comando" ] && grep -q "altro comando" "$P/bin/altro-comando" && pass "altri progetti intatti" || fail "toccati file di altri progetti (grave)"
[ -d "$H/.nyctalus-mobile" ] && pass "dati conservati se rispondi «no»" || fail "dati cancellati senza conferma"
# un comando omonimo di un altro progetto non viene sovrascritto
printf '#!/bin/sh\necho non mio\n' > "$P/bin/nyctalus-mobile"
T bash "$A/installa.sh" > "$TMP/i4.log" 2>&1
grep -q "non mio" "$P/bin/nyctalus-mobile" && pass "comando omonimo altrui non sovrascritto" || fail "sovrascritto un comando altrui"
[ "$KO" -eq 0 ]
