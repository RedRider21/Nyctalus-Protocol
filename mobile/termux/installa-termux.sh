#!/data/data/com.termux/files/usr/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Nyctalus in Termux con UN SOLO COMANDO (da incollare in Termux, una volta):
#
#   curl -fsSL https://github.com/RedRider21/Nyctalus-Protocol/releases/latest/download/installa-termux.sh | bash
#
# Fa tutto da solo:
#   1) permette all'app Nyctalus di comandare Termux (allow-external-apps), così poi non serve
#      più toccare Termux;
#   2) se Termux è appena installato, lo aggiorna (senza, Python può non installarsi);
#   3) scarica da GitHub il pacchetto di Nyctalus per Termux e ne VERIFICA la FIRMA (chiave della
#      release di Nyctalus, ssh-keygen -Y) e l'impronta SHA-256;
#   4) lo installa (il motore giusto per il processore: ARM 64/32 bit, Intel/AMD 64/32 bit);
#   5) riapre l'app Nyctalus, che avvia il servizio.
# Convive con gli altri progetti in Termux (vedi installa.sh): niente viene cancellato.

principale() {
  set -uo pipefail
  local REPO="${NYCTALUS_REPO:-https://github.com/RedRider21/Nyctalus-Protocol/releases/latest/download}"
  say() { printf '\033[1;36m» %s\033[0m\n' "$*"; }
  ok()  { printf '\033[1;32m✓ %s\033[0m\n' "$*"; }
  avv() { printf '\033[1;33m! %s\033[0m\n' "$*"; }
  err() { printf '\033[1;31m✗ %s\033[0m\n' "$*" >&2; }
  local PREFIX="${PREFIX:-/data/data/com.termux/files/usr}"
  [ -d "$PREFIX" ] || { err "Questo comando va incollato dentro Termux."; return 1; }

  # 1) l'app Nyctalus può comandare Termux (una riga in termux.properties, solo se manca)
  mkdir -p "$HOME/.termux"
  local P="$HOME/.termux/termux.properties"
  if ! grep -Eq '^[[:space:]]*allow-external-apps[[:space:]]*=[[:space:]]*true' "$P" 2>/dev/null; then
    printf '\n# aggiunto da Nyctalus: permette all app Nyctalus di avviare il suo servizio in Termux\nallow-external-apps = true\n' >> "$P"
  fi
  termux-reload-settings >/dev/null 2>&1 || true
  ok "L'app Nyctalus può avviare Nyctalus in Termux"

  # 2) Termux appena installato: aggiornamento iniziale
  if ! command -v python >/dev/null 2>&1; then
    say "Preparo Termux la prima volta (qualche minuto)…"
    export DEBIAN_FRONTEND=noninteractive
    yes | pkg upgrade -y -o Dpkg::Options::=--force-confnew || avv "aggiornamento di Termux non completo: provo lo stesso"
  fi
  command -v curl >/dev/null 2>&1 || pkg install -y curl

  # 3) il pacchetto di Nyctalus da GitHub, con l'impronta FIRMATA dalla chiave della release
  local D
  D="$(mktemp -d)"
  cd "$D" || return 1
  # chiave pubblica della release di Nyctalus (strumenti/firma-release.sh): un pacchetto la cui
  # impronta non è firmata da lei non si installa, nemmeno se viene da GitHub
  printf '%s\n' "${NYCTALUS_FIRMA_RELEASE:-release@nyctalus namespaces=\"nyctalus-release\" ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIIH2tSuN3+IVKV6CPiXCbkdT5FCyjciRePBoxVokIiiY}" > firmatari
  command -v ssh-keygen >/dev/null 2>&1 || pkg install -y openssh >/dev/null 2>&1 || true
  if ! command -v ssh-keygen >/dev/null 2>&1; then
    err "Manca ssh-keygen per verificare la firma del pacchetto (pkg install openssh): NON installo."
    return 1
  fi
  say "Scarico Nyctalus da GitHub…"
  if ! curl -fL --retry 5 --retry-delay 2 --connect-timeout 15 -o n.tar.gz "$REPO/nyctalus-termux.tar.gz" \
     || ! curl -fsSL --retry 5 -o SHA256SUMS "$REPO/SHA256SUMS" \
     || ! curl -fsSL --retry 5 -o SHA256SUMS.sig "$REPO/SHA256SUMS.sig"; then
    err "Scaricamento non riuscito (pacchetto o firma): controlla la connessione e incolla di nuovo il comando."
    return 1
  fi
  if ! ssh-keygen -Y verify -f firmatari -I release@nyctalus -n nyctalus-release -s SHA256SUMS.sig < SHA256SUMS >/dev/null 2>&1; then
    err "FIRMA NON VALIDA: questo pacchetto non viene da Nyctalus. NON lo installo."
    return 1
  fi
  local atteso
  atteso="$(awk '$2=="nyctalus-termux.tar.gz"{print $1}' SHA256SUMS)"
  if [ -z "$atteso" ] || ! echo "$atteso  n.tar.gz" | sha256sum -c --quiet; then
    err "L'impronta del pacchetto non corrisponde: NON lo installo (scaricamento rovinato o alterato)."
    return 1
  fi
  ok "Pacchetto scaricato, firmato da Nyctalus e verificato"

  # 4) installazione
  tar xzf n.tar.gz && bash nyctalus-termux/installa.sh || { err "Installazione non riuscita: leggi i messaggi qui sopra."; return 1; }
  cd "$HOME" && rm -rf "$D"

  # 5) di nuovo nell'app: avvia lei il servizio (con il suo segreto)
  ok "Fatto: torno nell'app Nyctalus…"
  sleep 2
  am start -n org.nyctalus.mobile/.MainActivity >/dev/null 2>&1 || echo "Apri l'app Nyctalus."
}

# tutto dentro una funzione: con «curl … | bash» lo script viene letto per intero prima di partire
principale "$@"
