#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# La cartella installazioni/: un pacchetto e un LEGGIMI per dispositivo,
# pacchetti completi e funzionanti, e MAI dati personali dentro.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · pacchetti di installazione"
bash "$ROOT/strumenti/crea-installazioni.sh" > "$TMP/crea.log" 2>&1 && pass "installazioni generate" || { fail "generazione fallita"; exit 1; }
I="$ROOT/installazioni"
for f in LEGGIMI.txt Nyctalus-completo.zip PC-Linux/LEGGIMI.txt PC-Linux/Nyctalus-PC-Linux.zip Server-Linux/LEGGIMI.txt Server-Linux/Nyctalus-Server-Linux.zip Manuali/SCELTE-OPERATIVE.md Manuali/MANUALE-SERVER.md Manuali/MANUALE-GESTORE-RETE.md Android/LEGGIMI.txt Android/nyctalus-termux.tar.gz Android/nyctalus-termux.zip Manuali/MANUALE-INSTALLAZIONE.md Manuali/MANUALE-TELEFONO.md Manuali/MANUALE-PC.md; do
  [ -s "$I/$f" ] && pass "presente: $f" || fail "manca: $f"
done
# nessun dato personale nei pacchetti
elenco() { case "$1" in *.zip) python3 -c "import zipfile,sys;print('\n'.join(zipfile.ZipFile(sys.argv[1]).namelist()))" "$1" ;; *) tar tzf "$1" ;; esac; }
for t in "$I/PC-Linux/Nyctalus-PC-Linux.zip" "$I/Server-Linux/Nyctalus-Server-Linux.zip" "$I/Android/nyctalus-termux.tar.gz" "$I/Android/nyctalus-termux.zip" "$I/Nyctalus-completo.zip"; do
  X=$(elenco "$t" | grep -E '/identita$|/token$|\.nyctalus-chat|\.nyctalus-mobile|cronologia/|contatti$|__pycache__|\.pyc$' | head -3)
  [ -z "$X" ] && pass "$(basename "$t"): nessun dato personale né file di cache" || fail "$(basename "$t") contiene: $X"
done
# il pacchetto PC si estrae e i programmi si caricano
mkdir -p "$TMP/pc" && unzip -q "$I/PC-Linux/Nyctalus-PC-Linux.zip" -d "$TMP/pc"
N="$TMP/pc/Nyctalus"
[ -x "$N/bin/nyctalus" ] && "$N/bin/nyctalus" help >/dev/null 2>&1 && pass "PC: il motore parte" || fail "PC: motore assente o non parte"
for s in avvia-launcher.sh avvia-browser.sh avvia-chat.sh installa.sh disinstalla.sh gestore-rete/nyctalus-rete server/installa.sh server/nyctalus-server; do [ -x "$N/$s" ] && pass "PC: $s eseguibile" || fail "PC: $s"; done
# il pacchetto server: motore + installatore dei nodi
mkdir -p "$TMP/srv" && python3 -c "import os,zipfile,sys;z=zipfile.ZipFile(sys.argv[1]);[os.chmod(z.extract(i,sys.argv[2]),i.external_attr>>16) for i in z.infolist()]" "$I/Server-Linux/Nyctalus-Server-Linux.zip" "$TMP/srv"
V="$TMP/srv/Nyctalus-Server"
[ -x "$V/bin/nyctalus" ] && [ -x "$V/installa.sh" ] && [ -x "$V/nyctalus-server" ] && [ -f "$V/Manuali/MANUALE-SERVER.md" ] \
  && pass "Server: motore, installatore, comando di gestione e manuale" || fail "Server: pacchetto incompleto"
! python3 -c "import zipfile,sys;n=zipfile.ZipFile(sys.argv[1]).namelist();sys.exit(0 if any(x.endswith(('.py','.svg')) or '/chat/' in x for x in n) else 1)" "$I/Server-Linux/Nyctalus-Server-Linux.zip" \
  && pass "Server: niente grafica né Python (solo il necessario)" || fail "Server: contiene file superflui"
PYTHONPATH="$N/launcher:$N/browser:$N/chat" python3 -c "
import nyctalus_launcher.app as l, nyctalus_chat.app as c, nyctalus_browser.config
assert l.LOGO.exists() and c.LOGO.exists()" 2>/dev/null && pass "PC: launcher, chat e browser si caricano e trovano il logo" || fail "PC: moduli o logo non trovati"
# il pacchetto Android contiene ciò che serve all'installatore
L=$(tar tzf "$I/Android/nyctalus-termux.tar.gz")
for f in installa.sh disinstalla.sh nyctalus-mobile orchestratore/nyctalus_mobile.py webapp/index.html webapp/app.js sorgenti/Cargo.lock sorgenti/motore/nyctalus/src/chat.rs; do
  echo "$L" | grep -q "nyctalus-termux/$f$" && pass "Android: $f" || fail "Android: manca $f"
done
[ "$KO" -eq 0 ]
