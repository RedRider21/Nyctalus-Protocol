#!/data/data/com.termux/files/usr/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Installa Nyctalus mobile in Termux, CONVIVENDO con gli altri progetti:
#  - tutto in ~/nyctalus (codice, motore) e nei dati ~/.nyctalus-mobile, ~/.nyctalus-chat;
#  - comandi solo col prefisso "nyctalus-";
#  - `pkg install` SOLO dei pacchetti mancanti, MAI `pkg upgrade` di tutto il sistema;
#  - termux.properties: si AGGIUNGE una riga solo se manca, il file non viene riscritto;
#  - avvio automatico (Termux:Boot) solo se lo chiedi: ./installa.sh --avvio-automatico
#
# Uso (dalla cartella estratta dell'archivio):   bash installa.sh
set -euo pipefail
QUI="$(cd "$(dirname "$0")" && pwd)"
DEST="$HOME/nyctalus"
PREFIX="${PREFIX:-/data/data/com.termux/files/usr}"
AVVIO_AUTO=0
for a in "$@"; do [ "$a" = "--avvio-automatico" ] && AVVIO_AUTO=1; done

say()  { printf '\033[1;36m» %s\033[0m\n' "$*"; }
ok()   { printf '\033[1;32m✓ %s\033[0m\n' "$*"; }
avv()  { printf '\033[1;33m! %s\033[0m\n' "$*"; }
err()  { printf '\033[1;31m✗ %s\033[0m\n' "$*" >&2; }

[ -d "$PREFIX" ] || { err "Questo installatore va lanciato dentro Termux."; exit 1; }

# --- 1) pacchetti: solo quelli che mancano ------------------------------------------
manca() { ! dpkg -s "$1" >/dev/null 2>&1; }
DA_INSTALLARE=()
for p in python termux-tools; do manca "$p" && DA_INSTALLARE+=("$p"); done
if [ ! -x "$QUI/bin/nyctalus" ]; then          # motore da compilare sul telefono
  for p in rust clang; do manca "$p" && DA_INSTALLARE+=("$p"); done
fi
if [ ${#DA_INSTALLARE[@]} -gt 0 ]; then
  say "Installo solo i pacchetti mancanti: ${DA_INSTALLARE[*]}"
  pkg install -y "${DA_INSTALLARE[@]}"
else
  ok "Pacchetti necessari già presenti"
fi
# Pillow: miniature veloci e nitide nella galleria degli allegati (facoltativo: senza,
# si usano le miniature già dentro le foto). Se l'installazione non riesce si prosegue.
if manca python-pillow; then
  say "Installo python-pillow (miniature della galleria, pochi MB)"
  pkg install -y python-pillow || avv "python-pillow non installato: la galleria userà miniature più piccole"
fi
if manca ffmpeg; then
  avv "Facoltativo: 'pkg install ffmpeg' per le miniature dei video in chat."
fi

# --- 2) copia dei file in ~/nyctalus -----------------------------------------------
say "Copio Nyctalus in $DEST"
mkdir -p "$DEST/bin"
rm -rf "$DEST/orchestratore" "$DEST/webapp"
cp -r "$QUI/orchestratore" "$QUI/webapp" "$DEST/"
cp "$QUI/disinstalla.sh" "$QUI/nyctalus-mobile" "$DEST/"
chmod +x "$DEST/nyctalus-mobile" "$DEST/disinstalla.sh"

# --- 3) il motore: già pronto nell'archivio, oppure compilato qui ------------------
if [ -x "$QUI/bin/nyctalus" ]; then
  cp "$QUI/bin/nyctalus" "$DEST/bin/nyctalus"
  ok "Motore precompilato installato"
else
  STAMPA="$(cd "$QUI/sorgenti" && find . -type f -name '*.rs' -o -name 'Cargo.*' | sort | xargs cat | sha256sum | cut -c1-16)"
  if [ -x "$DEST/bin/nyctalus" ] && [ "$(cat "$DEST/bin/.sorgenti" 2>/dev/null)" = "$STAMPA" ]; then
    ok "Motore già compilato per questi sorgenti"
  else
    say "Compilo il motore per il telefono (la prima volta richiede 10-30 minuti)…"
    rm -rf "$DEST/sorgenti"; cp -r "$QUI/sorgenti" "$DEST/sorgenti"
    ( cd "$DEST/sorgenti" && CARGO_TARGET_DIR="$DEST/.compilazione" cargo build --release -p nyctalus )
    cp "$DEST/.compilazione/release/nyctalus" "$DEST/bin/nyctalus"
    echo "$STAMPA" > "$DEST/bin/.sorgenti"
    ok "Motore compilato"
  fi
fi
"$DEST/bin/nyctalus" help >/dev/null 2>&1 || { err "il motore non parte su questo telefono"; exit 1; }

# --- 4) comando "nyctalus-mobile" (solo se il nome è libero o è già nostro) --------
LINK="$PREFIX/bin/nyctalus-mobile"
if [ -e "$LINK" ] && [ "$(readlink "$LINK" 2>/dev/null)" != "$DEST/nyctalus-mobile" ]; then
  avv "$LINK esiste già e non è nostro: non lo tocco. Usa $DEST/nyctalus-mobile"
else
  ln -sf "$DEST/nyctalus-mobile" "$LINK"
  ok "Comando: nyctalus-mobile"
fi

# --- 5) permesso per l'app Nyctalus (intent RUN_COMMAND) ---------------------------
PROP="$HOME/.termux/termux.properties"
mkdir -p "$HOME/.termux"
if ! grep -Eq '^[[:space:]]*allow-external-apps[[:space:]]*=[[:space:]]*true' "$PROP" 2>/dev/null; then
  printf '\n# aggiunto da Nyctalus: permette all app Nyctalus di avviare il suo servizio in Termux\nallow-external-apps = true\n' >> "$PROP"
  command -v termux-reload-settings >/dev/null && termux-reload-settings || true
  ok "Abilitato allow-external-apps in termux.properties (riga aggiunta, nient'altro toccato)"
else
  ok "allow-external-apps già abilitato"
fi

# --- 6) avvio automatico, solo se richiesto ----------------------------------------
if [ "$AVVIO_AUTO" = 1 ]; then
  mkdir -p "$HOME/.termux/boot"
  printf '#!/data/data/com.termux/files/usr/bin/sh\n# Nyctalus: avvio all accensione (Termux:Boot)\ntermux-wake-lock\n%s/nyctalus-mobile --sfondo\n' "$DEST" > "$HOME/.termux/boot/nyctalus-mobile"
  chmod +x "$HOME/.termux/boot/nyctalus-mobile"
  ok "Avvio automatico: ~/.termux/boot/nyctalus-mobile (serve l'app Termux:Boot)"
fi

echo
ok "Nyctalus mobile installato."
echo "   Avvia:      nyctalus-mobile"
echo "   Poi apri nel browser del telefono l'indirizzo che stampa (http://127.0.0.1:18731/?t=…),"
echo "   oppure apri l'app Nyctalus (APK), che fa tutto da sola."
echo "   Disinstalla: bash $DEST/disinstalla.sh"
