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

# --rete: il terminale del GESTORE DELLA RETE (stato, protezione, comandi di nyctalus-rete)
MODO=""; [ "${1:-}" = "--rete" ] && { MODO="--rete"; shift; }
if [ "${1:-}" != "--dentro" ]; then
  io="$(printf '%q' "$QUI/avvia-terminale.sh")"
  for t in x-terminal-emulator gnome-terminal mate-terminal xfce4-terminal konsole xterm; do
    command -v "$t" >/dev/null 2>&1 || continue
    case "$t" in
      gnome-terminal) exec gnome-terminal --title="Nyctalus" -- bash "$QUI/avvia-terminale.sh" $MODO --dentro ;;
      konsole|xterm)  exec "$t" -e bash "$QUI/avvia-terminale.sh" $MODO --dentro ;;
      *)              exec "$t" -e "bash $io $MODO --dentro" ;;
    esac
  done
  echo "✗ nessun terminale trovato (x-terminal-emulator, gnome-terminal, xterm…)"; exit 1
fi

if [ "$MODO" = "--rete" ]; then
  if [ -f "$QUI/gestore-rete/nyctalus-rete" ]; then GDIR="$QUI/gestore-rete"; else GDIR="$QUI/../pc/gestore-rete"; fi
  printf '\033]0;Nyctalus · gestore della rete\007'
  cd "$HOME" 2>/dev/null
  export PATH="$GDIR:$CLIDIR:$PATH"
  echo "🦇 Gestore della rete Nyctalus"
  echo
  echo "── La rete che usa questo PC"
  python3 "$GDIR/nyctalus-rete" stato 2>&1 | sed 's/^/  /'
  if [ -f "$HOME/.nyctalus-rete/autorita.json" ] || [ -d "$HOME/.nyctalus-rete/internet" ] || [ -d "$HOME/.nyctalus-rete/casa" ]; then
    echo
    echo "── Protezione della rete che gestisci (forma del traffico, rumore di fondo)"
    for q in "":"la tua rete" --internet:"la rete raggiungibile da Internet (casa --internet)" --casa:"la rete di casa (casa)"; do
      O="$(python3 "$GDIR/nyctalus-rete" protezione ${q%%:*} 2>/dev/null | head -6)"
      [ -n "$O" ] && { echo "  · ${q#*:}"; printf '%s\n' "$O" | sed 's/^/    /'; }
    done
  fi
  cat <<'AIUTO'

── Comandi utili (scrivili qui sotto)
  nyctalus-rete stato                               a quale rete è collegato questo PC
  nyctalus-rete protezione [--internet|--casa]      com'è la protezione della rete che gestisci
  nyctalus-rete protezione si --forma rumore|h3|webrtc [--rumore 0.5] [--internet|--casa]
                                                    cambia la forma del traffico o il rumore di fondo
                                                    (poi riaccendi la rete e ridai il codice a tutti)
  nyctalus-rete ip · porte · duckdns · vicini       IP e porte dal router, DuckDNS, rete in casa
  nyctalus-rete reti · usa N · importa CODICE       le reti salvate, cambiare rete
  nyctalus-rete --help                              tutti i comandi    ·   exit per chiudere
AIUTO
  echo
  RC="$(mktemp /tmp/nyctalus-rc.XXXXXX)"
  printf '%s\n' '[ -f ~/.bashrc ] && . ~/.bashrc' "PS1='\[\e[1;36m\]nyctalus-rete\[\e[0m\] \w › '" "export PATH=\"$GDIR:$CLIDIR:\$PATH\"" > "$RC"
  bash --rcfile "$RC" -i
  rm -f "$RC"
  exit 0
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
