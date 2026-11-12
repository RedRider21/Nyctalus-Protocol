#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# FIRMA DI UNA RELEASE (piano di sicurezza, punto 7): nella cartella con i file da pubblicare crea
# SHA256SUMS (l'impronta di ogni file) e SHA256SUMS.sig (la sua firma con la chiave della
# release, strumento standard ssh-keygen -Y). La chiave privata sta in privato/ e NON va mai su
# GitHub; la pubblica è in firma/ (e dentro lo script unico di Termux).
#   strumenti/firma-release.sh CARTELLA
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CHIAVE="${NYCTALUS_CHIAVE_RELEASE:-$ROOT/privato/firma-release/nyctalus-release}"
D="${1:?uso: firma-release.sh CARTELLA}"
[ -f "$CHIAVE" ] || { echo "manca la chiave privata della release: $CHIAVE" >&2; exit 1; }
cd "$D"
rm -f SHA256SUMS SHA256SUMS.sig
find . -maxdepth 1 -type f ! -name 'SHA256SUMS*' -printf '%f\n' | LC_ALL=C sort | xargs -d '\n' sha256sum > SHA256SUMS
ssh-keygen -q -Y sign -f "$CHIAVE" -n nyctalus-release SHA256SUMS
echo "✓ firmati $(wc -l < SHA256SUMS) file: $D/SHA256SUMS + SHA256SUMS.sig"
