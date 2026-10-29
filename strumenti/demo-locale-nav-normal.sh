#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Avvia il Nyctalus Browser in NAVIGAZIONE NORMALE (come un browser qualunque,
# senza rete di prova né demone). Lo stealth (rete Nyctalus) si accende col
# bottone in alto a destra; richiede però un demone attivo con proxy SOCKS
# (per la rete di prova completa, usa invece ./strumenti/demo-locale.sh).
#
#   ./strumenti/demo-locale-nav-normal.sh                 # pagina iniziale
#   ./strumenti/demo-locale-nav-normal.sh example.com     # apre un sito
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BROWSER_DIR="$ROOT/pc/browser"

err() { printf '\033[1;31m✗ %s\033[0m\n' "$*" >&2; }

if ! python3 -c 'import gi; gi.require_version("Gtk","3.0")' 2>/dev/null; then
  err "PyGObject/GTK3 non disponibile."
  echo "   Installa:  sudo apt install python3-gi gir1.2-gtk-3.0 gir1.2-webkit2-4.1"
  exit 1
fi

if [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  err "nessun display grafico (DISPLAY/WAYLAND_DISPLAY non impostati)."
  exit 1
fi

exec env PYTHONPATH="$BROWSER_DIR" python3 -m nyctalus_browser "$@"
