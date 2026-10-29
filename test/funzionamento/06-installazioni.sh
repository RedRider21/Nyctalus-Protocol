#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# La cartella installazioni/: un pacchetto e un LEGGIMI per dispositivo,
# pacchetti completi e funzionanti, e MAI dati personali dentro.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · pacchetti di installazione"
bash "$ROOT/strumenti/crea-installazioni.sh" > "$TMP/crea.log" 2>&1 && pass "installazioni generate" || { fail "generazione fallita"; exit 1; }
I="$ROOT/installazioni"
for f in LEGGIMI.txt PC-Linux/LEGGIMI.txt PC-Linux/Nyctalus-PC-Linux.tar.gz Android/LEGGIMI.txt Android/nyctalus-termux.tar.gz; do
  [ -s "$I/$f" ] && pass "presente: $f" || fail "manca: $f"
done
# nessun dato personale nei pacchetti
for t in "$I/PC-Linux/Nyctalus-PC-Linux.tar.gz" "$I/Android/nyctalus-termux.tar.gz"; do
  X=$(tar tzf "$t" | grep -E '/identita$|/token$|\.nyctalus-chat|\.nyctalus-mobile|cronologia/|contatti$|__pycache__|\.pyc$' | head -3)
  [ -z "$X" ] && pass "$(basename "$t"): nessun dato personale né file di cache" || fail "$(basename "$t") contiene: $X"
done
# il pacchetto PC si estrae e i programmi si caricano
mkdir -p "$TMP/pc" && tar xzf "$I/PC-Linux/Nyctalus-PC-Linux.tar.gz" -C "$TMP/pc"
N="$TMP/pc/Nyctalus"
[ -x "$N/bin/nyctalus" ] && "$N/bin/nyctalus" help >/dev/null 2>&1 && pass "PC: il motore parte" || fail "PC: motore assente o non parte"
for s in avvia-launcher.sh avvia-browser.sh avvia-chat.sh; do [ -x "$N/$s" ] && pass "PC: $s eseguibile" || fail "PC: $s"; done
PYTHONPATH="$N/launcher:$N/browser:$N/chat" python3 -c "
import nyctalus_launcher.app as l, nyctalus_chat.app as c, nyctalus_browser.config
assert l.LOGO.exists() and c.LOGO.exists()" 2>/dev/null && pass "PC: launcher, chat e browser si caricano e trovano il logo" || fail "PC: moduli o logo non trovati"
# il pacchetto Android contiene ciò che serve all'installatore
L=$(tar tzf "$I/Android/nyctalus-termux.tar.gz")
for f in installa.sh disinstalla.sh nyctalus-mobile orchestratore/nyctalus_mobile.py webapp/index.html webapp/app.js sorgenti/Cargo.lock sorgenti/motore/nyctalus/src/chat.rs; do
  echo "$L" | grep -q "nyctalus-termux/$f$" && pass "Android: $f" || fail "Android: manca $f"
done
[ "$KO" -eq 0 ]
