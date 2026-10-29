#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Installatore del PC in una HOME finta: dallo zip scompattato a menu e comandi;
# niente sudo se i pacchetti ci sono già; i comandi lanciati da collegamento
# trovano il programma; niente sovrascritture di cose altrui; disinstallazione pulita.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · installatore del PC (HOME finta)"
Z="$ROOT/installazioni/PC-Linux/Nyctalus-PC-Linux.zip"
[ -s "$Z" ] || bash "$ROOT/strumenti/crea-installazioni.sh" > /dev/null 2>&1
H="$TMP/home"; F="$TMP/finti"; mkdir -p "$H/.local/bin" "$H/.local/share/applications" "$F"
printf '#!/bin/sh\necho "$*" >> "%s/sudo.txt"\n' "$TMP" > "$F/sudo"           # sudo finto: registra
printf '#!/bin/sh\necho "PYTHONPATH=$PYTHONPATH"\n' > "$F/python3-finto"        # per vedere dove punta l'avvio
printf '#!/bin/sh\necho comando di un altro programma\n' > "$H/.local/bin/nyctalus-chat"
printf '[Desktop Entry]\nName=Altro\n' > "$H/.local/share/applications/altro.desktop"
chmod +x "$F"/* "$H/.local/bin/nyctalus-chat"
python3 -c "import zipfile;zipfile.ZipFile('$Z').extractall('$TMP/scompattato')" 2>/dev/null || unzip -q "$Z" -d "$TMP/scompattato"
chmod +x "$TMP/scompattato/Nyctalus/"*.sh "$TMP/scompattato/Nyctalus/bin/nyctalus" 2>/dev/null
T() { env -u XDG_DATA_HOME -u XDG_CONFIG_HOME -u XDG_CACHE_HOME HOME="$H" PATH="$F:$PATH" "$@"; }

T bash "$TMP/scompattato/Nyctalus/installa.sh" --si > "$TMP/i.log" 2>&1 && pass "installazione completata" || { fail "installazione fallita: $(tail -3 "$TMP/i.log")"; exit 1; }
D="$H/.local/share/nyctalus"
[ -x "$D/bin/nyctalus" ] && [ -x "$D/avvia-launcher.sh" ] && [ -d "$D/Manuali" ] && pass "programma e manuali in ~/.local/share/nyctalus" || fail "file mancanti"
[ ! -s "$TMP/sudo.txt" ] && pass "nessun sudo: i pacchetti necessari c'erano già" || fail "sudo chiamato senza bisogno: $(cat "$TMP/sudo.txt")"
for c in launcher browser; do
  [ "$(readlink "$H/.local/bin/nyctalus-$c")" = "$D/avvia-$c.sh" ] && pass "comando nyctalus-$c" || fail "comando nyctalus-$c mancante"
done
grep -q "un altro programma" "$H/.local/bin/nyctalus-chat" && pass "comando omonimo altrui non sovrascritto" || fail "sovrascritto un comando altrui (grave)"
for v in launcher browser chat; do
  f="$H/.local/share/applications/nyctalus-$v.desktop"
  grep -q "^Exec=\"$D/avvia-$v.sh\"" "$f" 2>/dev/null && grep -q "^Icon=$D/logo.svg" "$f" && pass "voce di menu: $v" || fail "voce di menu $v"
done
# il comando, lanciato da collegamento, deve trovare il programma installato
ln -sf "$F/python3-finto" "$F/python3"
OUT=$(T "$H/.local/bin/nyctalus-launcher" 2>&1); rm -f "$F/python3"
echo "$OUT" | grep -q "PYTHONPATH=$D/launcher:$D/browser" && pass "lanciato dal collegamento trova il programma" || fail "percorso sbagliato: $OUT"
[ ! -e "$H/.config/autostart/nyctalus-launcher.desktop" ] && pass "nessun avvio automatico se non richiesto" || fail "avvio automatico non richiesto"
T bash "$TMP/scompattato/Nyctalus/installa.sh" --si --avvio-automatico > /dev/null 2>&1
[ -f "$H/.config/autostart/nyctalus-launcher.desktop" ] && pass "--avvio-automatico" || fail "avvio automatico non creato"
mkdir -p "$H/.nyctalus-chat"; echo dato > "$H/.nyctalus-chat/contatti"
T bash "$D/disinstalla.sh" --si > /dev/null 2>&1
[ ! -e "$D" ] && [ ! -e "$H/.local/bin/nyctalus-launcher" ] && [ ! -e "$H/.local/share/applications/nyctalus-chat.desktop" ] \
  && [ ! -e "$H/.config/autostart/nyctalus-launcher.desktop" ] && pass "disinstallato: programma, comandi, menu e avvio automatico" || fail "disinstallazione incompleta"
[ -f "$H/.local/share/applications/altro.desktop" ] && grep -q "un altro programma" "$H/.local/bin/nyctalus-chat" \
  && pass "cose di altri programmi intatte" || fail "toccate cose di altri programmi (grave)"
[ -f "$H/.nyctalus-chat/contatti" ] && pass "dati personali conservati" || fail "dati cancellati senza conferma"
[ "$KO" -eq 0 ]
