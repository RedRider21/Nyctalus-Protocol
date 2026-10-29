# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Funzioni comuni alle prove (da includere con:  . "$(dirname "$0")/../lib.sh").
# Regole: cartelle temporanee sempre ripulite, porte dedicate 19xxx (diverse
# da sviluppo e telefono), nessun dato reale toccato, mai la webcam.
set -u
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/release/nyctalus"
STRUMENTI="$ROOT/test/strumenti"
TMP="$(mktemp -d /tmp/nyctalus-test.XXXXXX)"
PIDS=()
OK=0; KO=0; SALTATI=0
NOME_PROVA="$(basename "$0" .sh)"

pass() { printf '  \033[1;32mPASS\033[0m  %s\n' "$*"; OK=$((OK+1)); }
fail() { printf '  \033[1;31mFAIL\033[0m  %s\n' "$*"; KO=$((KO+1)); }
salta() { printf '  \033[1;33mSALTA\033[0m %s\n' "$*"; SALTATI=$((SALTATI+1)); }
titolo() { printf '\033[1;36m== %s ==\033[0m\n' "$*"; }
verifica() { # verifica "descrizione" comando...  → PASS se il comando riesce
  local d="$1"; shift
  if "$@" >/dev/null 2>&1; then pass "$d"; else fail "$d"; fi
}

pulizia() {
  for p in "${PIDS[@]:-}"; do [ -n "$p" ] && kill "$p" 2>/dev/null; done
  sleep 0.3
  for p in "${PIDS[@]:-}"; do [ -n "$p" ] && kill -9 "$p" 2>/dev/null; done
  # rete di sicurezza: qualunque processo rimasto che usa la NOSTRA cartella temporanea
  case "$TMP" in /tmp/nyctalus-test.*) pkill -f -- "$TMP" 2>/dev/null; sleep 0.2 ;; esac
  if [ -n "${NYCT_TIENI:-}" ]; then echo "  (log conservati in $TMP)"
  else case "$TMP" in /tmp/nyctalus-test.*) rm -rf "$TMP" ;; esac; fi
  # risultati per esegui-tutti.sh
  [ -n "${NYCT_RISULTATI:-}" ] && echo "$NOME_PROVA $OK $KO $SALTATI" >> "$NYCT_RISULTATI"
}
trap pulizia EXIT
trap 'exit 130' INT TERM

assicura_binario() {
  if [ ! -x "$BIN" ] || [ -n "$(find "$ROOT/motore" -name '*.rs' -newer "$BIN" -print -quit)" ]; then
    ( cd "$ROOT" && cargo build --release -p nyctalus >/dev/null 2>&1 ) || { fail "compilazione del motore"; exit 1; }
  fi
}

# attendi_regex FILE REGEX [secondi]  → stampa la prima corrispondenza
attendi_regex() {
  local f="$1" re="$2" t="${3:-15}" x="" i
  for i in $(seq 1 $((t * 5))); do
    x=$(grep -oE "$re" "$f" 2>/dev/null | head -1); [ -n "$x" ] && break; sleep 0.2
  done
  echo "$x"
}

porta_occupata() { python3 -c "import socket,sys;s=socket.socket();sys.exit(0 if s.connect_ex(('127.0.0.1',$1))==0 else 1)"; }

# rete_di_prova BASE  → autorità + 4 nodi su BASE..BASE+3; imposta SEC PUB DIR
rete_di_prova() {
  local base="$1" p L righe=""
  "$BIN" authority > "$TMP/auth.txt" 2>&1
  SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
  PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
  for p in $base $((base+1)) $((base+2)) $((base+3)); do
    porta_occupata "$p" && { fail "porta $p già occupata da un altro programma"; exit 1; }
    "$BIN" node --listen 127.0.0.1:"$p" > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
  done
  for p in $base $((base+1)) $((base+2)) $((base+3)); do
    L=$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+")
    [ -z "$L" ] && { fail "nodo di prova $p non partito"; exit 1; }
    righe="$righe${righe:+;}$L"
  done
  DIR="$TMP/dir.txt"
  "$BIN" directory --authority-secret "$SEC" --entries "$righe" > "$DIR" 2>/dev/null
}

# identita NOME BACKEND CONTROLLO SOCKS [SEGRETO]  → motore chat + servizio .nyct; indirizzo in $ADDR
# NB: NON chiamarla dentro $(...): i processi avviati lì non finirebbero in PIDS.
identita() {
  local nome="$1" be="$2" ctl="$3" socks="$4" seg="${5:-}" addr
  mkdir -p "$TMP/$nome"
  NYCTALUS_CHAT_TOKEN="$seg" "$BIN" chat --data "$TMP/$nome" --backend 127.0.0.1:"$be" --control 127.0.0.1:"$ctl" \
      --socks 127.0.0.1:"$socks" > "$TMP/$nome.chat.log" 2>&1 & PIDS+=($!)
  addr=$(attendi_regex "$TMP/$nome.chat.log" '[a-z2-7]{56}\.nyct' 10)
  "$BIN" site --backend 127.0.0.1:"$be" --identity "$TMP/$nome/identita" --directory "$DIR" --authority "$PUB" \
      --hops 3 > "$TMP/$nome.site.log" 2>&1 & PIDS+=($!)
  ADDR="$addr"
}

# controllo PORTA [SEGRETO] [ATTESA] COMANDI...  → stampa le righe ricevute dal motore
controllo() {
  python3 "$STRUMENTI/controllo.py" "$@"
}
