#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Il mini menu (launcher) dai SORGENTI, senza installare. Installato, c'è nel menu
# delle applicazioni come «Nyctalus» e col comando nyctalus-launcher.
exec bash "$(cd "$(dirname "$(readlink -f "$0")")/.." && pwd)/pc/launcher/nyctalus-launcher" "$@"
