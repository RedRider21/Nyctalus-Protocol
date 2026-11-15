#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# «⬆ AGGIORNA NYCTALUS» (nyctalus-aggiorna, bottone del mini menu del PC): una finta GitHub
# locale e una chiave DI PROVA (mai quella vera). Versione nuova → si scarica, si verifica la
# firma di SHA256SUMS e l'impronta del pacchetto, l'installatore verifica l'elenco d'integrità;
# pacchetto alterato, firma d'altri o elenco ritoccato → non si installa nulla; già aggiornato
# → niente da fare. Tutto in una HOME finta: l'installazione e i servizi veri non si toccano.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · aggiornamento del PC da GitHub (firma e impronta verificate)"
command -v ssh-keygen >/dev/null || { salta "ssh-keygen non installato"; exit 0; }
command -v zip >/dev/null || { salta "zip non installato"; exit 0; }
AGG="$ROOT/pc/cli/nyctalus-aggiorna"; INT="$ROOT/pc/cli/nyctalus-integrita"
# isolamento: casa finta, dati finti, nomi di servizi che non esistono; l'installatore si ferma
# dopo il controllo d'integrità (NYCTALUS_SOLO_VERIFICA)
export HOME="$TMP/casa" XDG_DATA_HOME="$TMP/casa/dati"; mkdir -p "$HOME"
export NYCTALUS_AGGIORNA_SERVIZI="nyctalus-prova-55-inesistente.service" NYCTALUS_SOLO_VERIFICA=1
ssh-keygen -q -t ed25519 -N "" -C prova -f "$TMP/chiave" >/dev/null
export NYCTALUS_CHIAVE_RELEASE="$TMP/chiave" NYCTALUS_FIRMA_INTEGRITA="$(cut -d' ' -f1,2 "$TMP/chiave.pub")"
# un pacchetto PC piccolo ma vero nella forma: installatore, integrità, motore finto, VERSIONE
P="$TMP/pac/Nyctalus"; mkdir -p "$P/cli" "$P/bin"
cp "$ROOT/pc/installa.sh" "$P/"; cp "$INT" "$AGG" "$P/cli/"; printf '#!/bin/sh\nexit 0\n' > "$P/bin/nyctalus"; chmod +x "$P/bin/nyctalus"
echo "9.9.9" > "$P/VERSIONE"
python3 "$INT" firma "$P" > /dev/null
W="$TMP/www"; R="$W/download/v9.9.9"; mkdir -p "$R"
( cd "$TMP/pac" && zip -qr "$R/Nyctalus-PC-Linux.zip" Nyctalus )
firma_release() { ( cd "$R" && rm -f SHA256SUMS.sig && sha256sum Nyctalus-PC-Linux.zip > SHA256SUMS && ssh-keygen -q -Y sign -f "$1" -n nyctalus-release SHA256SUMS ); }
firma_release "$TMP/chiave"
PESO=$(stat -c %s "$R/Nyctalus-PC-Linux.zip")
echo "{\"tag_name\":\"v9.9.9\",\"assets\":[{\"name\":\"Nyctalus-PC-Linux.zip\",\"size\":$PESO}]}" > "$W/latest.json"
PORTA=24650
python3 -m http.server $PORTA --bind 127.0.0.1 --directory "$W" > /dev/null 2>&1 & PIDS+=($!)
for _ in $(seq 1 50); do porta_occupata $PORTA && break; sleep 0.2; done
export NYCTALUS_AGGIORNA_API="http://127.0.0.1:$PORTA/latest.json" NYCTALUS_AGGIORNA_FILE="http://127.0.0.1:$PORTA/download/{tag}/{file}"

J=$(python3 "$AGG" --controlla --json); E=$?
[ "$E" = 10 ] && echo "$J" | grep -q '"nuova": true' && echo "$J" | grep -q '"ultima": "9.9.9"' && echo "$J" | grep -q "\"peso\": $PESO" \
  && pass "--controlla: versione nuova 9.9.9 vista, con il peso del pacchetto (codice 10)" || fail "controlla: $E $J"
python3 "$AGG" --si > "$TMP/ok.txt" 2>&1; E=$?
[ "$E" = 0 ] && grep -q "firma della release valida" "$TMP/ok.txt" && grep -q "impronta del pacchetto corrisponde" "$TMP/ok.txt" \
  && grep -q "Integrità del pacchetto confermata" "$TMP/ok.txt" && grep -q "aggiornato alla 9.9.9" "$TMP/ok.txt" \
  && pass "--si: scarica, firma valida, impronta giusta, l'installatore conferma l'integrità" || fail "aggiornamento: $E $(cat "$TMP/ok.txt" | tr "\n" " ")"

# pacchetto sostituito dopo la firma (SHA256SUMS firmato non torna) → niente installazione
cp "$R/Nyctalus-PC-Linux.zip" "$TMP/buono.zip"; echo "x" >> "$R/Nyctalus-PC-Linux.zip"
python3 "$AGG" --si > "$TMP/alt.txt" 2>&1; E=$?
[ "$E" = 1 ] && grep -q "NON corrisponde all'elenco firmato" "$TMP/alt.txt" && ! grep -q "» installo" "$TMP/alt.txt" \
  && pass "pacchetto alterato dopo la pubblicazione: rifiutato, nulla installato" || fail "alterato: $E $(tail -3 "$TMP/alt.txt")"
cp "$TMP/buono.zip" "$R/Nyctalus-PC-Linux.zip"

# firma di un'altra chiave → niente installazione
ssh-keygen -q -t ed25519 -N "" -f "$TMP/altra" >/dev/null; firma_release "$TMP/altra"
python3 "$AGG" --si > "$TMP/fir.txt" 2>&1; E=$?
[ "$E" = 1 ] && grep -q "FIRMA della release non è valida" "$TMP/fir.txt" && pass "release firmata da un'altra chiave: rifiutata" || fail "firma d'altri: $E $(tail -3 "$TMP/fir.txt")"
firma_release "$TMP/chiave"

# elenco ritoccato (impronta cambiata a mano) → la firma non torna
sed -i '1s/^./0/' "$R/SHA256SUMS"
python3 "$AGG" --si > "$TMP/eln.txt" 2>&1; E=$?
[ "$E" = 1 ] && grep -q "FIRMA della release non è valida" "$TMP/eln.txt" && pass "SHA256SUMS ritoccato: la firma non torna, rifiutato" || fail "elenco ritoccato: $E"
firma_release "$TMP/chiave"

# già aggiornati (ultima = installata)
V=$(cat "$ROOT/VERSIONE")
echo "{\"tag_name\":\"v$V\",\"assets\":[]}" > "$W/latest.json"
python3 "$AGG" --controlla > /dev/null 2>&1; E1=$?
python3 "$AGG" --si > "$TMP/gia.txt" 2>&1; E2=$?
[ "$E1" = 0 ] && [ "$E2" = 0 ] && grep -q "già l'ultima versione" "$TMP/gia.txt" && pass "già aggiornato: niente da scaricare (codice 0)" || fail "già aggiornato: $E1 $E2"
# risposta strana da GitHub
echo '{"tag_name":"<script>"}' > "$W/latest.json"
python3 "$AGG" --controlla --json > "$TMP/strano.txt" 2>&1; E=$?
[ "$E" = 1 ] && grep -q "errore" "$TMP/strano.txt" && pass "risposta strana da GitHub: errore chiaro, nessuna azione" || fail "risposta strana: $E"
[ ! -e "$HOME/.local/share/nyctalus" ] && pass "nessuna installazione fuori dalla casa finta della prova" || fail "installazione inattesa"
grep -q '"lau.agg.vai"' "$ROOT/pc/browser/nxs_i18n/strings/de.json" && grep -q "_aggiorna" "$ROOT/pc/launcher/nyctalus_launcher/app.py" \
  && pass "bottone «⬆ Aggiorna Nyctalus» nel menu ⚙ del mini menu, testi in 5 lingue" || fail "bottone o testi mancanti"
[ "$KO" -eq 0 ]
