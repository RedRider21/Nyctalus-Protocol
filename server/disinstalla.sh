#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Toglie i nodi Nyctalus da questo server: servizi, programma e comando.
# Le IDENTITÀ dei nodi (chiavi) si cancellano solo con --anche-identita: tenendole,
# reinstallando i nodi tornano con le stesse righe d'elenco.
set -u
R="${NYCTALUS_RADICE:-}"
ANCHE=0; [ "${1:-}" = "--anche-identita" ] && ANCHE=1
if [ -f "${XDG_DATA_HOME:-$HOME/.local/share}/nyctalus-server/server.conf" ]; then
  BASE="${XDG_DATA_HOME:-$HOME/.local/share}/nyctalus-server"; CONF="$BASE/server.conf"
  UNITA="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"; COMANDO="$HOME/.local/bin/nyctalus-server"; SC=(systemctl --user)
else
  BASE="$R/opt/nyctalus"; CONF="$R/etc/nyctalus/server.conf"; UNITA="$R/etc/systemd/system"
  COMANDO="$R/usr/local/bin/nyctalus-server"; SC=(systemctl)
fi
[ -f "$CONF" ] || { echo "Nessun nodo Nyctalus installato."; exit 0; }
# shellcheck disable=SC1090
. "$CONF"
NYCTALUS_STATO="$R${NYCTALUS_STATO:-}"      # (R vuota sui server veri)
for p in ${NYCTALUS_PORTE:-}; do "${SC[@]}" disable --now "nyctalus-nodo@$p" >/dev/null 2>&1; done
rm -f "$UNITA/nyctalus-nodo@.service"; "${SC[@]}" daemon-reload 2>/dev/null
[ "$(readlink "$COMANDO" 2>/dev/null)" = "$BASE/bin/nyctalus-server" ] && rm -f "$COMANDO"
case "$BASE" in */nyctalus|*/nyctalus-server) rm -rf "$BASE/bin" "$BASE/disinstalla.sh" ;; esac
if [ "$ANCHE" = 1 ]; then
  case "${NYCTALUS_STATO:-}" in */nyctalus|*/nodi) rm -rf "$NYCTALUS_STATO" ;; esac
  rm -f "$CONF" "$(dirname "$CONF")/righe-nodi.txt"
  echo "✓ Nodi e identità rimossi."
else
  echo "✓ Nodi rimossi. Identità conservate in ${NYCTALUS_STATO:-?} (cancellale con --anche-identita)."
fi
echo "  Ricorda al gestore della rete di togliere questi nodi dall'elenco: nyctalus-rete togli IP:PORTA"
