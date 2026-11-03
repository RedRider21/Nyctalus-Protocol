#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# ⌨ Terminale di Nyctalus (PC): apre il terminale del sistema pronto per nyctalus-cli.
# Se il motore è spento lo accende senza finestre e lo spegne quando chiudi il terminale;
# se è acceso (chat o File aperti) lo usa e basta.
#   avvia-terminale.sh            → apre una finestra di terminale
#   avvia-terminale.sh --dentro   → (dentro il terminale) prepara e lascia una shell
set -u
QUI="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
if   [ -x "$QUI/cli/nyctalus-cli" ];       then CLIDIR="$QUI/cli"            # pacchetto installato
else CLIDIR="$QUI/../pc/cli"; fi                                            # progetto

if [ "${1:-}" != "--dentro" ]; then
  io="$(printf '%q' "$QUI/avvia-terminale.sh")"
  for t in x-terminal-emulator gnome-terminal mate-terminal xfce4-terminal konsole xterm; do
    command -v "$t" >/dev/null 2>&1 || continue
    case "$t" in
      gnome-terminal) exec gnome-terminal --title="Nyctalus" -- bash "$QUI/avvia-terminale.sh" --dentro ;;
      konsole|xterm)  exec "$t" -e bash "$QUI/avvia-terminale.sh" --dentro ;;
      *)              exec "$t" -e "bash $io --dentro" ;;
    esac
  done
  echo "✗ nessun terminale trovato (x-terminal-emulator, gnome-terminal, xterm…)"; exit 1
fi

# --- dentro il terminale --------------------------------------------------------------
printf '\033]0;Nyctalus · terminale\007'
cd "$HOME" 2>/dev/null
export PATH="$CLIDIR:$PATH"
MOTORE=""
acceso() { ss -ltn 2>/dev/null | grep -q ":1091\b" && [ -s "$HOME/.nyctalus-chat/controllo" ]; }
spegni() { [ -n "$MOTORE" ] && kill -TERM "$MOTORE" 2>/dev/null; MOTORE=""; }
trap spegni EXIT HUP INT TERM

if ! acceso; then
  echo "» accendo il motore di Nyctalus (senza finestre; si spegne quando chiudi questo terminale)"
  LOG="$(mktemp /tmp/nyctalus-terminale.XXXXXX)"
  NYCTALUS_INTERFACCIA=nessuna setsid bash "$QUI/avvia-chat.sh" > "$LOG" 2>&1 & MOTORE=$!
  for _ in $(seq 1 60); do grep -q "pronto" "$LOG" && break; sleep 0.5; done
  grep -q "pronto" "$LOG" || { echo "✗ il motore non è partito:"; tail -5 "$LOG"; }
  rm -f "$LOG"
fi
echo
nyctalus-cli aiuto
echo
echo "Il tuo Contact ID: $(nyctalus-cli io 2>/dev/null || echo '?')"
echo "Scrivi i comandi così come sono, es.  contatti  ·  scrivi mario ciao  ·  file -s   (exit per chiudere)"
echo
RC="$(mktemp /tmp/nyctalus-rc.XXXXXX)"
cat > "$RC" <<'EOF'
[ -f ~/.bashrc ] && . ~/.bashrc
PS1='\[\e[1;36m\]nyctalus\[\e[0m\] \w › '
# come nel pannello del telefono: i comandi si scrivono anche senza «nyctalus-cli» davanti
# (vale solo in questo terminale; il «file» di sistema resta raggiungibile con «command file»)
for c in io contatti aggiungi scrivi leggi ascolta invia file accetta rifiuta pausa riprendi annulla togli pulisci impostazioni aiuto; do
  eval "$c() { nyctalus-cli $c \"\$@\"; }"
done
unset c
EOF
bash --rcfile "$RC" -i
rm -f "$RC"
