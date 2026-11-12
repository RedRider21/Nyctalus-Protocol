#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# AUDIT DELLE DIPENDENZE (piano di sicurezza, punto 7): controlla Cargo.lock contro il database
# pubblico delle vulnerabilità note (RustSec). Non scarica nulla da solo: usa cargo-audit e il
# database in ~/.local/share/nyctalus-strumenti (o NYCTALUS_STRUMENTI). Per aggiornare il
# database (≈12 MB, a casa):  git -C ~/.local/share/nyctalus-strumenti/advisory-db pull
#   strumenti/audit.sh        → 0 = nessuna vulnerabilità nota, 1 = da correggere
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
D="${NYCTALUS_STRUMENTI:-$HOME/.local/share/nyctalus-strumenti}"
CA="$D/cargo-audit"; command -v cargo-audit >/dev/null 2>&1 && [ ! -x "$CA" ] && CA="$(command -v cargo-audit)"
if [ ! -x "$CA" ] || [ ! -d "$D/advisory-db" ]; then
  echo "Manca cargo-audit o il database in $D."
  echo "Prendi l'eseguibile da https://github.com/rustsec/rustsec/releases (cargo-audit-x86_64-unknown-linux-musl)"
  echo "e il database con: git clone --depth 1 https://github.com/rustsec/advisory-db.git $D/advisory-db"
  exit 2
fi
echo "» database delle vulnerabilità del $(git -C "$D/advisory-db" log -1 --format=%cd --date=short 2>/dev/null || echo ?)"
exec "$CA" audit --db "$D/advisory-db" --no-fetch --file "$ROOT/Cargo.lock"
