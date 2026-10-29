#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Installa Nyctalus sul PC per l'utente corrente (niente root, tranne per gli
# eventuali pacchetti di sistema mancanti, e solo se lo confermi):
#  - programma in ~/.local/share/nyctalus
#  - comandi nyctalus-launcher / nyctalus-browser / nyctalus-chat in ~/.local/bin
#    (mai sovrascritti se appartengono a un altro programma)
#  - voci nel menu delle applicazioni, con l'icona del pipistrello
#  - --avvio-automatico: il launcher parte all'accesso
#  - --si: risponde «sì» alle domande (installazione senza interazione)
#
# Uso, dalla cartella scompattata:   ./installa.sh
set -euo pipefail
QUI="$(cd "$(dirname "$0")" && pwd)"
DEST="${XDG_DATA_HOME:-$HOME/.local/share}/nyctalus"
BIN_DIR="$HOME/.local/bin"
APP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
AUTO_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"
AVVIO=0; SI=0
for a in "$@"; do case "$a" in --avvio-automatico) AVVIO=1 ;; --si) SI=1 ;; esac; done

say() { printf '\033[1;36m» %s\033[0m\n' "$*"; }
ok()  { printf '\033[1;32m✓ %s\033[0m\n' "$*"; }
avv() { printf '\033[1;33m! %s\033[0m\n' "$*"; }
chiedi() { [ "$SI" = 1 ] && return 0; printf '%s [s/N] ' "$1"; read -r r; [ "$r" = s ] || [ "$r" = S ]; }

[ -x "$QUI/bin/nyctalus" ] || { echo "✗ lancia installa.sh dalla cartella scompattata di Nyctalus"; exit 1; }

# --- 1) pacchetti di sistema: si controlla, e si installa solo su conferma ----------
manca_py() { ! python3 -c "import gi; gi.require_version('$1', '$2')" 2>/dev/null; }
MANCANTI=()
command -v python3 >/dev/null || MANCANTI+=(python3)
manca_py Gtk 3.0 && MANCANTI+=(python3-gi gir1.2-gtk-3.0)
if manca_py WebKit2 4.1 && manca_py WebKit2 4.0; then MANCANTI+=(gir1.2-webkit2-4.1); fi
if [ ${#MANCANTI[@]} -gt 0 ]; then
  avv "Mancano pacchetti necessari: ${MANCANTI[*]}"
  if command -v apt >/dev/null && chiedi "Li installo ora con «sudo apt install»?"; then
    sudo apt install -y "${MANCANTI[@]}"
  else
    avv "Installali tu con:  sudo apt install ${MANCANTI[*]}"
  fi
else
  ok "Pacchetti necessari presenti"
fi
FACOLTATIVI=()
manca_py Keybinder 3.0 && FACOLTATIVI+=(gir1.2-keybinder-3.0)
manca_py Gst 1.0 && FACOLTATIVI+=(gir1.2-gstreamer-1.0 gstreamer1.0-gtk3 gstreamer1.0-plugins-good gstreamer1.0-plugins-ugly gstreamer1.0-libav)
command -v ffmpeg >/dev/null || FACOLTATIVI+=(ffmpeg)
if [ ${#FACOLTATIVI[@]} -gt 0 ]; then
  avv "Facoltativi (tasti rapidi globali, fotocamera, audio, miniature video): ${FACOLTATIVI[*]}"
  if command -v apt >/dev/null && [ "$SI" = 0 ] && chiedi "Installo anche questi?"; then
    sudo apt install -y "${FACOLTATIVI[@]}"
  fi
fi

# --- 2) il programma -------------------------------------------------------------
say "Copio Nyctalus in $DEST"
mkdir -p "$DEST"
for x in bin browser chat launcher logo.svg avvia-launcher.sh avvia-browser.sh avvia-chat.sh LICENSE disinstalla.sh Manuali; do
  [ -e "$QUI/$x" ] || continue
  rm -rf "${DEST:?}/$x"; cp -r "$QUI/$x" "$DEST/"
done
chmod +x "$DEST"/*.sh "$DEST/bin/nyctalus"
"$DEST/bin/nyctalus" help >/dev/null 2>&1 || { echo "✗ il motore non parte su questo PC"; exit 1; }
ok "Programma installato"

# --- 3) comandi (solo se il nome è libero o è già nostro) ------------------------
mkdir -p "$BIN_DIR"
for c in launcher browser chat; do
  L="$BIN_DIR/nyctalus-$c"
  if [ -e "$L" ] && [ "$(readlink "$L" 2>/dev/null)" != "$DEST/avvia-$c.sh" ]; then
    avv "$L esiste già e non è di Nyctalus: non lo tocco"
  else
    ln -sf "$DEST/avvia-$c.sh" "$L"
  fi
done
ok "Comandi: nyctalus-launcher, nyctalus-browser, nyctalus-chat"
case ":$PATH:" in *":$BIN_DIR:"*) ;; *) avv "$BIN_DIR non è nel PATH: i comandi funzioneranno dopo il prossimo accesso (o usa il menu)";; esac

# --- 4) voci nel menu delle applicazioni -----------------------------------------
mkdir -p "$APP_DIR"
voce() { # file nome commento comando
  cat > "$APP_DIR/$1" <<EOF
[Desktop Entry]
Type=Application
Name=$2
Comment=$3
Exec="$DEST/$4"
Icon=$DEST/logo.svg
Terminal=false
Categories=Network;Chat;Security;
StartupNotify=true
X-Nyctalus=1
EOF
}
voce nyctalus-launcher.desktop "Nyctalus" "Pannello di Nyctalus: browser e chat anonimi" avvia-launcher.sh
voce nyctalus-browser.desktop "Nyctalus Browser" "Navigazione normale o anonima (Stealth) e siti .nyct" avvia-browser.sh
voce nyctalus-chat.desktop "Nyctalus Chat" "Chat .nyct senza server e senza metadati" avvia-chat.sh
command -v update-desktop-database >/dev/null && update-desktop-database "$APP_DIR" 2>/dev/null || true
ok "Voci nel menu delle applicazioni"

# --- 5) avvio automatico, solo se richiesto --------------------------------------
if [ "$AVVIO" = 1 ]; then
  mkdir -p "$AUTO_DIR"; cp "$APP_DIR/nyctalus-launcher.desktop" "$AUTO_DIR/nyctalus-launcher.desktop"
  ok "Il launcher partirà all'accesso"
fi

echo
ok "Nyctalus è installato."
echo "   Avvia dal menu delle applicazioni («Nyctalus»), oppure:  nyctalus-launcher"
echo "   Manuali: $DEST/Manuali/   ·   Disinstalla: $DEST/disinstalla.sh"
