#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Prepara il pacchetto di Nyctalus per Termux:  pacchetto-termux.sh CARTELLA
# Crea CARTELLA/nyctalus-termux/ (installatore, servizio, web-app, sorgenti del
# motore, motore aarch64 precompilato se c'è) e CARTELLA/nyctalus-termux.tar.gz.
# Lo usano crea-installazioni.sh (cartella installazioni/) e compila-apk.sh
# (l'app lo porta con sé per installare Nyctalus in Termux con un tocco).
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$(mkdir -p "$1" && cd "$1" && pwd)"
A="$OUT/nyctalus-termux"
rm -rf "$A"; mkdir -p "$A/sorgenti"
cp -r "$ROOT/mobile/orchestratore" "$ROOT/mobile/webapp" "$A/"
cp "$ROOT/mobile/termux/installa.sh" "$ROOT/mobile/termux/disinstalla.sh" "$ROOT/mobile/termux/nyctalus-mobile" "$A/"
cp "$ROOT/Cargo.toml" "$ROOT/Cargo.lock" "$A/sorgenti/"
cp -r "$ROOT/motore" "$A/sorgenti/"
mkdir -p "$A/Manuali"
cp "$ROOT"/documenti/manuali/*.md "$ROOT/documenti/DEPLOY.md" "$ROOT/documenti/SICUREZZA.md" "$A/Manuali/"
find "$A" -name __pycache__ -type d -prune -exec rm -rf {} +
for t in aarch64-linux-android aarch64-unknown-linux-musl; do
  if [ -x "$ROOT/target/$t/release/nyctalus" ]; then
    mkdir -p "$A/bin"; cp "$ROOT/target/$t/release/nyctalus" "$A/bin/nyctalus"
    echo "» pacchetto Termux: incluso il motore precompilato ($t)"; break
  fi
done
chmod +x "$A/installa.sh" "$A/disinstalla.sh" "$A/nyctalus-mobile"
cp "$ROOT/LICENSE" "$A/"
( cd "$OUT" && tar czf nyctalus-termux.tar.gz nyctalus-termux )
