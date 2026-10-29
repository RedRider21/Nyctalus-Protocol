#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Demo locale "tutto in uno": avvia una piccola rete Nyctalus di prova (4 nodi),
# pubblica un sito .nyct dimostrativo, avvia il demone con proxy SOCKS, e apre il
# Nyctalus Browser già puntato sul sito. Chiudendo il browser (o con Ctrl-C) si
# ferma e pulisce tutto.
#
#   ./strumenti/demo-locale.sh
#
# Richiede: il binario `nyctalus` (lo compila se manca) e, per il browser,
# python3 + PyGObject (GTK3) + WebKit2GTK. Senza il browser puoi comunque usare
# curl --socks5-hostname 127.0.0.1:1080 http://<ADDR>.nyct/
set -u

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/nyctalus"
BROWSER_DIR="$ROOT/pc/browser"
SOCKS_PORT="${SOCKS_PORT:-1080}"
WORK="$(mktemp -d /tmp/nyctalus-demo.XXXXXX)"
PIDS=()

say() { printf '\033[1;36m» %s\033[0m\n' "$*"; }
err() { printf '\033[1;31m✗ %s\033[0m\n' "$*" >&2; }

cleanup() {
  say "chiusura, pulizia in corso…"
  for p in "${PIDS[@]:-}"; do kill "$p" 2>/dev/null; done
  rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

# --- binario -----------------------------------------------------------------
if [ ! -x "$BIN" ]; then
  say "compilo nyctalus (release)…"
  ( cd "$ROOT" && cargo build --release -p nyctalus ) || { err "compilazione fallita"; exit 1; }
fi

# --- porta SOCKS libera ------------------------------------------------------
if ss -ltn 2>/dev/null | grep -q ":$SOCKS_PORT\b"; then
  err "la porta SOCKS $SOCKS_PORT è occupata. Chiudi il processo o usa SOCKS_PORT=1081 ./strumenti/demo-locale.sh"
  exit 1
fi

# --- sito dimostrativo -------------------------------------------------------
mkdir -p "$WORK/sito"
cat > "$WORK/sito/index.html" <<'HTML'
<!doctype html><html lang="it"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1"><title>Sito .nyct</title>
<style>:root{color-scheme:dark}body{margin:0;font-family:system-ui,sans-serif;
background:#0b1020;color:#e6ecff;display:flex;min-height:100vh;align-items:center;justify-content:center}
.card{max-width:640px;padding:48px 40px;text-align:center}.bat{font-size:64px}
h1{margin:.3em 0;font-size:30px}.addr{font-family:monospace;color:#5aa9ff;word-break:break-all;
background:#131a33;padding:10px 14px;border-radius:10px;display:inline-block;margin:12px 0}
p{color:#9fb0d9;line-height:1.6}.badge{display:inline-block;margin-top:18px;padding:6px 14px;
border-radius:999px;background:#12351f;color:#7ff0a8;font-size:14px}</style></head>
<body><div class="card"><div class="bat">🦇</div>
<h1>Benvenuto in un sito .nyct</h1>
<p>Questa pagina è servita da un <b>servizio nascosto Nyctalus</b> e ti è arrivata
attraverso un <b>circuito a più salti</b> fino a un punto d'incontro. Né tu né il
sito conoscete l'IP dell'altro.</p>
<div class="addr" id="a">…</div>
<div class="badge">● cifrato end-to-end · Noise NK</div></div>
<script>fetch('/addr.txt').then(r=>r.text()).then(t=>document.getElementById('a').textContent=t.trim());</script>
</body></html>
HTML

cd "$WORK"

# --- autorità + nodi + elenco firmato ---------------------------------------
say "creo l'autorità e 4 nodi di prova…"
"$BIN" authority > auth.txt 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' auth.txt | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' auth.txt | awk '{print $2}')
if [ -z "$SEC" ] || [ -z "$PUB" ]; then
  err "non riesco a creare l'autorità:"; cat auth.txt; exit 1
fi

NODI=(5810 5811 5812 5813)
for p in "${NODI[@]}"; do
  if ss -ltn 2>/dev/null | grep -q "127.0.0.1:$p\b"; then
    err "la porta del nodo $p è occupata. Chiudi il processo che la usa e riprova."; exit 1
  fi
  "$BIN" node --listen 127.0.0.1:"$p" > "n$p.log" 2>&1 & PIDS+=($!)
done

# Aspetta che OGNI nodo abbia stampato la sua riga d'elenco (fino a ~15s).
LINES=""
for p in "${NODI[@]}"; do
  L=""
  for _ in $(seq 1 60); do
    L=$(grep -oE "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+" "n$p.log" | head -1)
    [ -n "$L" ] && break
    sleep 0.25
  done
  if [ -z "$L" ]; then
    err "il nodo $p non è partito:"; cat "n$p.log"; exit 1
  fi
  LINES="$LINES${LINES:+;}$L"
done

"$BIN" directory --authority-secret "$SEC" --entries "$LINES" > dir.txt 2>dir.err
if ! grep -q '^firma ' dir.txt 2>/dev/null; then
  err "non riesco a firmare l'elenco dei nodi:"; cat dir.err 2>/dev/null; exit 1
fi

# --- sito .nyct (rendezvous onion automatico, 3 salti) ----------------------
say "pubblico il sito .nyct…"
"$BIN" site --dir sito --identity svc.key --directory dir.txt --authority "$PUB" --hops 3 > site.log 2>&1 & PIDS+=($!)
ADDR=""
for _ in $(seq 1 40); do
  ADDR=$(grep -oE '[a-z2-7]{56}\.nyct' site.log | head -1)
  [ -n "$ADDR" ] && break
  sleep 0.25
done
if [ -z "$ADDR" ]; then err "il sito non è partito:"; cat site.log; exit 1; fi
echo "$ADDR" > sito/addr.txt

# --- demone con proxy SOCKS --------------------------------------------------
say "avvio il demone (proxy SOCKS su 127.0.0.1:$SOCKS_PORT, 3 salti)…"
"$BIN" circuit --directory dir.txt --authority "$PUB" --socks 127.0.0.1:"$SOCKS_PORT" --hops 3 --control 127.0.0.1:1591 > daemon.log 2>&1 & PIDS+=($!)
sleep 3

echo
say "Rete di prova pronta."
echo "   Sito .nyct:   http://$ADDR/"
echo "   Proxy SOCKS:  127.0.0.1:$SOCKS_PORT  (per curl: curl --socks5-hostname 127.0.0.1:$SOCKS_PORT http://$ADDR/)"
echo "   Nota: è una rete LOCALE di 4 nodi solo per provare; non dà anonimato reale."
echo

# --- browser -----------------------------------------------------------------
if ! python3 -c 'import gi; gi.require_version("Gtk","3.0")' 2>/dev/null; then
  err "PyGObject/GTK3 non disponibile: non posso aprire il browser."
  echo "   Installa:  sudo apt install python3-gi gir1.2-gtk-3.0 gir1.2-webkit2-4.1"
  echo "   Intanto puoi provare il sito con:  curl --socks5-hostname 127.0.0.1:$SOCKS_PORT http://$ADDR/"
  echo "   Premi Invio per terminare la rete di prova."
  read -r _
  exit 0
fi

if [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
  err "nessun display grafico (DISPLAY/WAYLAND_DISPLAY non impostati): il browser non può aprirsi."
  echo "   Usa curl --socks5-hostname 127.0.0.1:$SOCKS_PORT http://$ADDR/ . Invio per terminare."
  read -r _
  exit 0
fi

say "apro il Nyctalus Browser sul sito .nyct (chiudilo per fermare la demo)…"
PYTHONPATH="$BROWSER_DIR" python3 -m nyctalus_browser "http://$ADDR/"
