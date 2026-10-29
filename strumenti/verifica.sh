#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Verifiche di sicurezza: scorciatoia verso le prove in test/sicurezza/.
# Per tutte le prove (funzionamento + sicurezza): test/esegui-tutti.sh
exec bash "$(cd "$(dirname "$0")/.." && pwd)/test/esegui-tutti.sh" sicurezza
