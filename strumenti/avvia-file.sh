#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Nyctalus · File: scambio di file grandi con i contatti. Usa lo stesso motore della chat:
# se è già acceso ci si aggancia, altrimenti lo accende (e lo spegne quando chiudi l'ultima
# finestra fra chat e File).
QUI="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
NYCTALUS_INTERFACCIA=nyctalus_file exec bash "$QUI/avvia-chat.sh" "$@"
