#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Installa uno o più NODI Nyctalus su un server Linux (VPS o PC sempre acceso),
# come servizi di sistema che ripartono da soli. Niente grafica, solo il motore.
#
#   sudo ./installa.sh                     chiede IP pubblico, quanti nodi, porte
#   sudo ./installa.sh --ip 203.0.113.5 --nodi 4 --porta 4601 --si
#   ./installa.sh --utente                 senza root: servizi dell'utente (systemd --user)
#
# Opzioni: --ip IP  --nodi N (1-8)  --porta PRIMA_PORTA  --camuffamento HEX
#          --rumore HZ  --utente  --si (nessuna domanda: valori indicati o predefiniti)
#
# Alla fine stampa le RIGHE D'ELENCO dei nodi: vanno mandate a chi gestisce la rete,
# che le firma (nyctalus-rete aggiungi … && nyctalus-rete firma).
set -euo pipefail
QUI="$(cd "$(dirname "$(readlink -f "$0")")" && pwd)"
R="${NYCTALUS_RADICE:-}"            # solo per le prove: radice finta al posto di /
IP=""; NODI=""; PORTA=""; CAMUFFA=""; RUMORE="0"; UTENTE=0; SI=0
while [ $# -gt 0 ]; do
  case "$1" in
    --ip) IP="$2"; shift ;;
    --nodi) NODI="$2"; shift ;;
    --porta) PORTA="$2"; shift ;;
    --camuffamento) CAMUFFA="$2"; shift ;;
    --rumore) RUMORE="$2"; shift ;;
    --utente) UTENTE=1 ;;
    --si) SI=1 ;;
    *) echo "opzione sconosciuta: $1"; exit 2 ;;
  esac
  shift
done

say() { printf '\033[1;36m» %s\033[0m\n' "$*"; }
ok()  { printf '\033[1;32m✓ %s\033[0m\n' "$*"; }
avv() { printf '\033[1;33m! %s\033[0m\n' "$*"; }
err() { printf '\033[1;31m✗ %s\033[0m\n' "$*" >&2; exit 1; }
chiedi() { # chiedi "domanda" predefinito → valore
  local r; if [ "$SI" = 1 ]; then echo "$2"; return; fi
  printf '%s [%s]: ' "$1" "$2" >&2; read -r r; echo "${r:-$2}"
}

MOTORE="$QUI/bin/nyctalus"; [ -x "$MOTORE" ] || MOTORE="$QUI/../bin/nyctalus"      # pacchetto server o pacchetto PC
[ -x "$MOTORE" ] || err "manca bin/nyctalus: lancia lo script dalla cartella scompattata"
"$MOTORE" help >/dev/null 2>&1 || err "il motore non parte su questo server (architettura diversa? serve x86_64 Linux)"
if [ "$UTENTE" = 0 ] && [ "$(id -u)" != 0 ] && [ -z "$R" ]; then
  err "serve root (sudo ./installa.sh), oppure --utente per installare solo per questo utente"
fi
command -v systemctl >/dev/null || err "systemd non trovato: avvia i nodi a mano (vedi MANUALE-SERVER.md)"

# --- dove ---------------------------------------------------------------------------
if [ "$UTENTE" = 1 ]; then
  BASE="${XDG_DATA_HOME:-$HOME/.local/share}/nyctalus-server"; STATO="$BASE/nodi"; CONF="$BASE/server.conf"
  UNITA="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"; COMANDO="$HOME/.local/bin/nyctalus-server"
  SC=(systemctl --user)
else
  BASE="$R/opt/nyctalus"; STATO="$R/var/lib/nyctalus"; CONF="$R/etc/nyctalus/server.conf"
  UNITA="$R/etc/systemd/system"; COMANDO="$R/usr/local/bin/nyctalus-server"
  SC=(systemctl)
fi

# --- domande --------------------------------------------------------------------------
SUGG="$(ip -4 route get 192.0.2.1 2>/dev/null | grep -oE 'src [0-9.]+' | cut -d' ' -f2 || true)"
[ -n "$IP" ] || IP="$(chiedi "IP PUBBLICO del server (quello del pannello del fornitore; questo vede: ${SUGG:-?})" "${SUGG:-}")"
[[ "$IP" =~ ^[0-9]{1,3}(\.[0-9]{1,3}){3}$|^[0-9a-fA-F:]+$|^[A-Za-z0-9.-]+$ ]] && [ -n "$IP" ] || err "IP non valido: $IP"
case "$IP" in 10.*|192.168.*|172.1[6-9].*|172.2[0-9].*|172.3[01].*)
  avv "$IP è un indirizzo PRIVATO: i nodi saranno raggiungibili solo dalla rete locale." ;;
esac
[ -n "$NODI" ] || NODI="$(chiedi "Quanti nodi su questo server (1 se hai più server; 3-4 se è l'unico)" 4)"
[[ "$NODI" =~ ^[1-8]$ ]] || err "numero di nodi fra 1 e 8"
[ -n "$PORTA" ] || PORTA="$(chiedi "Prima porta UDP (le altre a seguire)" 4601)"
[[ "$PORTA" =~ ^[0-9]+$ ]] && [ "$PORTA" -ge 1024 ] && [ "$((PORTA + NODI - 1))" -le 65535 ] || err "porta non valida"
[ -z "$CAMUFFA" ] || [[ "$CAMUFFA" =~ ^[0-9a-f]{64}$ ]] || err "chiave di camuffamento: 64 caratteri esadecimali"
PORTE=(); for i in $(seq 0 $((NODI - 1))); do PORTE+=($((PORTA + i))); done

# --- programma e utente di servizio -------------------------------------------------
say "Installo il motore in $BASE"
mkdir -p "$BASE/bin" "$STATO" "$(dirname "$CONF")" "$UNITA" "$(dirname "$COMANDO")"
install -m 755 "$MOTORE" "$BASE/bin/nyctalus"
install -m 755 "$QUI/nyctalus-server" "$BASE/bin/nyctalus-server"
[ -f "$QUI/disinstalla.sh" ] && install -m 755 "$QUI/disinstalla.sh" "$BASE/disinstalla.sh"
ln -sf "$BASE/bin/nyctalus-server" "$COMANDO"
PROPRIETARIO=""
if [ "$UTENTE" = 0 ]; then
  if ! id nyctalus >/dev/null 2>&1; then
    useradd --system --no-create-home --home-dir /var/lib/nyctalus --shell /usr/sbin/nologin nyctalus 2>/dev/null \
      || adduser --system --no-create-home --home /var/lib/nyctalus --shell /usr/sbin/nologin nyctalus 2>/dev/null || true
  fi
  id nyctalus >/dev/null 2>&1 && PROPRIETARIO="nyctalus"
fi

OPZIONI="--cover $RUMORE"
[ -n "$CAMUFFA" ] && OPZIONI="$OPZIONI --obfuscation $CAMUFFA"
umask 077
cat > "$CONF" <<EOF
# Nyctalus server: letto dai servizi nyctalus-nodo@PORTA (modifica e poi: nyctalus-server riavvia)
# (valori tra virgolette: li leggono sia systemd sia bash)
NYCTALUS_IP="$IP"
NYCTALUS_OPZIONI="$OPZIONI"
NYCTALUS_PORTE="${PORTE[*]}"
NYCTALUS_STATO="${STATO#$R}"
NYCTALUS_SERVIZIO="$([ "$UTENTE" = 1 ] && echo utente || echo sistema)"
EOF
chmod 644 "$CONF"
umask 022

# --- identità dei nodi (stabili: la riga d'elenco non cambia ai riavvii) -------------
say "Preparo l'identità di ${#PORTE[@]} nodi"
for p in "${PORTE[@]}"; do
  D="$STATO/nodo-$p"; mkdir -p "$D"; chmod 700 "$D"
  if [ ! -s "$D/riga-elenco.txt" ] || ! grep -q "^$IP:$p," "$D/riga-elenco.txt"; then
    rm -f "$D/riga-elenco.txt"
    "$BASE/bin/nyctalus" node --listen 127.0.0.1:0 --data "$D" --announce "$IP:$p" > /dev/null 2>&1 & NP=$!
    for _ in $(seq 1 50); do [ -s "$D/riga-elenco.txt" ] && break; sleep 0.1; done
    kill "$NP" 2>/dev/null || true; wait "$NP" 2>/dev/null || true
    [ -s "$D/riga-elenco.txt" ] || err "identità del nodo $p non creata"
  fi
done
[ -n "$PROPRIETARIO" ] && chown -R "$PROPRIETARIO:" "$STATO"

# --- servizio systemd ------------------------------------------------------------------
UTENTE_UNITA=""; [ -n "$PROPRIETARIO" ] && UTENTE_UNITA="User=$PROPRIETARIO"
cat > "$UNITA/nyctalus-nodo@.service" <<EOF
# Nyctalus: un nodo-relè per porta (nyctalus-nodo@4601, @4602, …)
[Unit]
Description=Nyctalus, nodo-relè sulla porta UDP %i
After=network-online.target
Wants=network-online.target

[Service]
$UTENTE_UNITA
EnvironmentFile=${CONF#$R}
ExecStart=${BASE#$R}/bin/nyctalus node --listen 0.0.0.0:%i --data \${NYCTALUS_STATO}/nodo-%i --announce \${NYCTALUS_IP}:%i \$NYCTALUS_OPZIONI
Restart=always
RestartSec=3
NoNewPrivileges=true
PrivateTmp=true
ProtectHome=true
ProtectSystem=strict
ReadWritePaths=${STATO#$R}

[Install]
WantedBy=$([ "$UTENTE" = 1 ] && echo default.target || echo multi-user.target)
EOF
[ "$UTENTE" = 1 ] && sed -i '/^ProtectHome=/d;/^ProtectSystem=/d;/^ReadWritePaths=/d' "$UNITA/nyctalus-nodo@.service"
"${SC[@]}" daemon-reload
for p in "${PORTE[@]}"; do "${SC[@]}" enable --now "nyctalus-nodo@$p" >/dev/null 2>&1 || avv "servizio nyctalus-nodo@$p non avviato (vedi: nyctalus-server stato)"; done
ok "Servizi avviati: ${PORTE[*]/#/nyctalus-nodo@}"
[ "$UTENTE" = 1 ] && avv "Servizi utente: perché restino accesi anche senza login esegui una volta: sudo loginctl enable-linger $USER"

# --- firewall ---------------------------------------------------------------------------
if command -v ufw >/dev/null && ufw status 2>/dev/null | grep -q "Status: active"; then
  for p in "${PORTE[@]}"; do ufw allow "$p/udp" >/dev/null; done; ok "Firewall (ufw): aperte le porte UDP ${PORTE[*]}"
elif command -v firewall-cmd >/dev/null && firewall-cmd --state >/dev/null 2>&1; then
  for p in "${PORTE[@]}"; do firewall-cmd --permanent --add-port="$p/udp" >/dev/null; done; firewall-cmd --reload >/dev/null; ok "Firewall (firewalld): aperte le porte UDP ${PORTE[*]}"
else
  avv "Nessun firewall attivo rilevato. Se il fornitore ha un firewall nel pannello, apri lì le porte UDP ${PORTE[*]}."
fi

# --- le righe da mandare al gestore -----------------------------------------------------
RIGHE="$(dirname "$CONF")/righe-nodi.txt"
cat "$STATO"/nodo-*/riga-elenco.txt > "$RIGHE"
echo
ok "Nodi installati. Manda queste righe a chi gestisce la rete (sono pubbliche, non segrete):"
echo
sed 's/^/   /' "$RIGHE"
echo
echo "   (salvate anche in $RIGHE · ristampale con: nyctalus-server righe)"
echo "   Gestione: nyctalus-server stato | righe | riavvia | ferma | avvia | log"
