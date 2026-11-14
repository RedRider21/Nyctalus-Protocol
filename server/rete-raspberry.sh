#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# LA TUA RETE NYCTALUS SU UN RASPBERRY PI (o qualunque Linux sempre acceso), IN UN COLPO.
# Dalla cartella scompattata del pacchetto server (Nyctalus-Server-Linux-arm64.zip sul Raspberry):
#
#   ./rete-raspberry.sh --nome miacasa.duckdns.org       (o l'IP pubblico, se non cambia)
#
# Fa tutto: controlla processore e motore, copia il programma in ~/.local/share/nyctalus-server,
# apre il firewall (se c'è ufw, chiede la password), tiene i servizi accesi anche senza login,
# accende la RETE come servizio che riparte da solo (nodi, elenco firmato, rubrica, casella,
# pagina del codice) e stampa il CODICE DI RETE da dare agli amici e le porte da inoltrare.
#
# Di serie i nodi NON fanno da uscita verso Internet (la navigazione degli altri non esce dal tuo
# indirizzo; chat, chiamate, file e siti .nyct funzionano): --con-uscita per permetterla.
#
# Opzioni: --nome NOME  --nodi N (di serie 4)  --porta-base 4701  --porta 18800
#          --con-uscita  --si (nessuna domanda)  --togli (spegne e toglie il servizio della rete)
# Poi: nyctalus-rete casa --internet NOME --servizio --stampa   (rivede codice e stato)
#      journalctl --user -u nyctalus-casa -f                     (i messaggi della rete)
set -euo pipefail
QUI="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
DEST="${XDG_DATA_HOME:-$HOME/.local/share}/nyctalus-server"
BIN_DIR="$HOME/.local/bin"
NOME=""; NODI=4; BASE=4701; WEB=18800; USCITA=0; SI=0; TOGLI=0
while [ $# -gt 0 ]; do
  case "$1" in
    --nome) NOME="$2"; shift ;;
    --nodi) NODI="$2"; shift ;;
    --porta-base) BASE="$2"; shift ;;
    --porta) WEB="$2"; shift ;;
    --con-uscita) USCITA=1 ;;
    --si) SI=1 ;;
    --togli) TOGLI=1 ;;
    -h|--help) sed -n '4,22p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "opzione sconosciuta: $1 (vedi --help)"; exit 2 ;;
  esac
  shift
done
say() { printf '\033[1;36m» %s\033[0m\n' "$*"; }
ok()  { printf '\033[1;32m✓ %s\033[0m\n' "$*"; }
avv() { printf '\033[1;33m! %s\033[0m\n' "$*"; }
err() { printf '\033[1;31m✗ %s\033[0m\n' "$*" >&2; exit 1; }
[ "$(id -u)" != 0 ] || err "lancialo come utente normale (non con sudo): la password la chiede solo per il firewall"

if [ "$TOGLI" = 1 ]; then
  [ -x "$DEST/gestore-rete/nyctalus-rete" ] || err "nessuna rete installata da questo script"
  python3 "$DEST/gestore-rete/nyctalus-rete" casa --internet x --servizio --togli
  exit 0
fi

# --- 1) programma ----------------------------------------------------------------------
PAC="$QUI"; [ -x "$PAC/bin/nyctalus" ] || PAC="$QUI/.."
[ -x "$PAC/bin/nyctalus" ] && [ -f "$PAC/gestore-rete/nyctalus-rete" ] || err "lancialo dalla cartella scompattata del pacchetto server di Nyctalus"
"$PAC/bin/nyctalus" help >/dev/null 2>&1 || err "il motore non parte su questo processore ($(uname -m)): sul Raspberry Pi serve il pacchetto «Nyctalus-Server-Linux-arm64.zip» e un sistema a 64 bit"
command -v python3 >/dev/null || err "manca python3 (sudo apt install python3)"
# integrità del pacchetto (se c'è l'elenco firmato)
if [ -f "$PAC/INTEGRITA" ] && [ -f "$PAC/cli/nyctalus-integrita" ]; then
  E=0; python3 "$PAC/cli/nyctalus-integrita" verifica "$PAC" >/dev/null 2>&1 || E=$?
  [ "$E" = 1 ] && err "controllo d'integrità fallito: il pacchetto NON è quello pubblicato (riscaricalo dalla release)"
  [ "$E" = 0 ] && ok "Integrità del pacchetto confermata"
fi
say "Copio il programma in $DEST"
mkdir -p "$DEST" "$BIN_DIR"
for x in bin gestore-rete cli Manuali LICENSE INTEGRITA INTEGRITA.sig nyctalus-sito; do
  [ -e "$PAC/$x" ] || continue
  rm -rf "${DEST:?}/$x"; cp -r "$PAC/$x" "$DEST/"
done
for c in rete:gestore-rete/nyctalus-rete sito:nyctalus-sito collaudo:cli/nyctalus-collaudo integrita:cli/nyctalus-integrita; do
  n="${c%%:*}"; f="$DEST/${c#*:}"; L="$BIN_DIR/nyctalus-$n"
  [ -f "$f" ] || continue
  if [ -e "$L" ] && [ "$(readlink "$L" 2>/dev/null)" != "$f" ]; then avv "$L esiste già e non è di Nyctalus: non lo tocco"
  else chmod +x "$f"; ln -sf "$f" "$L"; fi
done
ok "Programma installato (comandi: nyctalus-rete, nyctalus-sito, nyctalus-collaudo, nyctalus-integrita)"
case ":$PATH:" in *":$BIN_DIR:"*) ;; *) avv "aggiungi $BIN_DIR al PATH (di solito basta uscire e rientrare)";; esac

# --- 2) il nome pubblico -----------------------------------------------------------------
if [ -z "$NOME" ]; then
  [ "$SI" = 1 ] && err "indica --nome (nome DuckDNS/No-IP o IP pubblico)"
  echo "Con quale NOME si raggiunge casa tua da Internet? Un nome che segue il tuo IP (DuckDNS, No-IP)"
  printf "oppure l'IP pubblico se non cambia: "; read -r NOME
fi
[[ "$NOME" =~ ^[A-Za-z0-9.-]{3,253}$ ]] || err "nome non valido: $NOME"
ULTIMA=$((BASE + NODI - 1))

# --- 3) firewall e servizi sempre accesi ---------------------------------------------------
if command -v ufw >/dev/null && systemctl is-active ufw >/dev/null 2>&1; then
  say "Apro il firewall (ufw): UDP e TCP $BASE-$ULTIMA, TCP $WEB (serve la password)"
  sudo ufw allow "$BASE:$ULTIMA/udp" >/dev/null && sudo ufw allow "$BASE:$ULTIMA/tcp" >/dev/null && sudo ufw allow "$WEB/tcp" >/dev/null \
    && ok "Firewall aperto" || avv "firewall non aperto: fallo a mano (sudo ufw allow $BASE:$ULTIMA/udp …)"
fi
if ! loginctl show-user "$USER" -p Linger 2>/dev/null | grep -q "=yes"; then
  say "Servizi accesi anche senza login (loginctl enable-linger)"
  loginctl enable-linger "$USER" 2>/dev/null || sudo loginctl enable-linger "$USER" || avv "a mano: sudo loginctl enable-linger $USER"
fi

# --- 4) la rete come servizio ------------------------------------------------------------
OPZ=(casa --internet "$NOME" --nodi "$NODI" --porta-base "$BASE" --porta "$WEB" --servizio)
[ "$USCITA" = 1 ] && OPZ+=(--con-uscita) || OPZ+=(--senza-uscita)
say "Accendo la rete come servizio (riparte da solo dopo un riavvio)"
NYCTALUS_BIN="$DEST/bin/nyctalus" python3 "$DEST/gestore-rete/nyctalus-rete" "${OPZ[@]}"

IPL="$(hostname -I 2>/dev/null | awk '{print $1}')"
echo
echo "Sul ROUTER inoltra verso questo Raspberry (${IPL:-il suo indirizzo in casa}) — una volta sola:"
echo "  • UDP $BASE-$ULTIMA  e  TCP $BASE-$ULTIMA   (i nodi)"
echo "  • TCP $WEB          (l'elenco dei nodi e la pagina del codice)"
echo "  e nel router dai al Raspberry un indirizzo fisso (DHCP statico)."
echo "Poi, da un telefono con i DATI MOBILI (non la Wi-Fi di casa): http://$NOME:$WEB/"
echo "Controllo della rete: nyctalus-collaudo --solo-rete  ·  messaggi: journalctl --user -u nyctalus-casa -f"
