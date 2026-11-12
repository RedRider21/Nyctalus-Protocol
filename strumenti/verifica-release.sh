#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# VERIFICA DI UNA RELEASE: la firma di SHA256SUMS è della chiave della release di Nyctalus, e i
# file scaricati hanno l'impronta scritta lì. Serve solo ssh-keygen (OpenSSH) e sha256sum.
#   strumenti/verifica-release.sh CARTELLA   (con SHA256SUMS, SHA256SUMS.sig e i file scaricati)
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIRMATARI="${NYCTALUS_FIRMATARI:-$ROOT/firma/allowed_signers}"
D="${1:?uso: verifica-release.sh CARTELLA}"
cd "$D"
if ! ssh-keygen -Y verify -f "$FIRMATARI" -I release@nyctalus -n nyctalus-release -s SHA256SUMS.sig < SHA256SUMS > /dev/null 2>&1; then
  echo "✗ FIRMA NON VALIDA: SHA256SUMS non viene dalla chiave della release di Nyctalus" >&2; exit 1
fi
echo "✓ firma valida (chiave della release di Nyctalus)"
sha256sum -c --ignore-missing SHA256SUMS
