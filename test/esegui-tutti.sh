#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Esegue tutte le prove di funzionamento e di sicurezza e stampa il riepilogo.
#   test/esegui-tutti.sh                 tutto
#   test/esegui-tutti.sh sicurezza       solo le prove di sicurezza
#   test/esegui-tutti.sh funzionamento   solo quelle di funzionamento
#   NYCT_TIENI=1 test/esegui-tutti.sh    conserva i log delle prove (per indagare)
# Le prove usano cartelle temporanee e porte 19xxx: non toccano i tuoi dati né
# gli altri programmi, e non accendono mai la webcam.
set -u
QUI="$(cd "$(dirname "$0")" && pwd)"
RISULTATI="$(mktemp /tmp/nyctalus-risultati.XXXXXX)"
export NYCT_RISULTATI="$RISULTATI"
GRUPPI="${1:-sicurezza funzionamento}"
INIZIO=$(date +%s)

for g in $GRUPPI; do
  for t in "$QUI/$g"/*.sh; do
    timeout 600 bash "$t"
    echo
  done
done

echo "════════════════════════ RIEPILOGO ════════════════════════"
TOT_OK=0; TOT_KO=0; TOT_S=0
while read -r nome ok ko s; do
  stato="\033[1;32mOK\033[0m"; [ "$ko" -gt 0 ] && stato="\033[1;31mFALLITA\033[0m"
  [ "$ok" -eq 0 ] && [ "$ko" -eq 0 ] && [ "$s" -gt 0 ] && stato="\033[1;33mSALTATA\033[0m"
  printf "  %-28s %b   (%s pass, %s fail, %s saltate)\n" "$nome" "$stato" "$ok" "$ko" "$s"
  TOT_OK=$((TOT_OK+ok)); TOT_KO=$((TOT_KO+ko)); TOT_S=$((TOT_S+s))
done < "$RISULTATI"
rm -f "$RISULTATI"
echo "────────────────────────────────────────────────────────────"
printf "  Verifiche: %d superate, %d fallite, %d saltate — %ds\n" "$TOT_OK" "$TOT_KO" "$TOT_S" "$(( $(date +%s) - INIZIO ))"
[ "$TOT_KO" -eq 0 ]
