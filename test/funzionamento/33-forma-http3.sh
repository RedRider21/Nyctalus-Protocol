#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# FORMA «HTTP/3» (PIANO-ANONIMATO, invisibilità punto 2: trasporto mimetico). Invece del rumore
# puro i nodi parlano QUIC come un normale sito HTTP/3 (ALPN h3, nome scelto dal gestore); la
# chiave della rete diventa un contrassegno dentro TLS. Chi si collega senza contrassegno riceve
# una pagina 404 HTTP/3: lo verifica un VERO browser (Chrome) che fa la parte del censore.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · forma HTTP/3 (trasporto mimetico)"
assicura_binario
B=22520; NOME=www.esempio.it
K=$(python3 -c "import os;print(os.urandom(32).hex())")
export NYCTALUS_MIMETICO=1 NYCTALUS_NOME_MIMETICO=$NOME
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
righe=""
for p in $B $((B+1)) $((B+2)) $((B+3)); do
  "$BIN" node --listen 127.0.0.1:$p --data "$TMP/n$p" --obfuscation "$K" > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
done
for p in $B $((B+1)) $((B+2)) $((B+3)); do
  L=$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+")
  [ -z "$L" ] && { fail "nodo $p non partito: $(tail -2 "$TMP/n$p.log")"; exit 1; }
  righe="$righe${righe:+;}$L"
done
grep -q "transport form: HTTP/3" "$TMP/n$B.log" && pass "i nodi partono nella forma HTTP/3 (ALPN h3, nome $NOME)" || fail "forma HTTP/3 non attiva"
DIR="$TMP/dir.txt"
"$BIN" directory --authority-secret "$SEC" --entries "$righe" > "$DIR" 2>/dev/null
mkdir -p "$TMP/www"; echo "ciao dal sito" > "$TMP/www/index.html"
python3 -m http.server $((B+20)) --bind 127.0.0.1 --directory "$TMP/www" > /dev/null 2>&1 & PIDS+=($!)

# 1) navigare attraverso 3 salti nella forma HTTP/3
"$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$K" --socks 127.0.0.1:$((B+10)) --control 127.0.0.1:$((B+11)) --hops 3 > "$TMP/c.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/c.log" "SOCKS5 proxy" 20 > /dev/null
R=$(curl -s -m 20 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R" = "ciao dal sito" ] && pass "si naviga attraverso 3 salti con i nodi in forma HTTP/3" || fail "navigazione: $R · $(tail -3 "$TMP/c.log")"
# 2) con un'altra chiave (contrassegno sbagliato) non si entra
"$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$(python3 -c "import os;print(os.urandom(32).hex())")" \
    --socks 127.0.0.1:$((B+12)) --control 127.0.0.1:$((B+13)) --hops 3 > "$TMP/c2.log" 2>&1 & PIDS+=($!)
sleep 6
R=$(curl -s -m 8 --socks5-hostname 127.0.0.1:$((B+12)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R" != "ciao dal sito" ] && pass "senza il contrassegno giusto non si entra nella rete" || fail "entrato con il contrassegno sbagliato"
# 3) sito .nyct
ADDR=$("$BIN" address --identity "$TMP/servizio" | grep -oE '[a-z2-7]{56}\.nyct' | tail -1)
"$BIN" site --dir "$TMP/www" --identity "$TMP/servizio" --directory "$DIR" --authority "$PUB" --obfuscation "$K" --hops 3 > "$TMP/s.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/s.log" "online" 30 > /dev/null
R=$(timeout 40 "$BIN" browse "$ADDR" --directory "$DIR" --authority "$PUB" --obfuscation "$K" --hops 3 2>/dev/null)
echo "$R" | grep -q "ciao dal sito" && pass "sito .nyct pubblicato e visitato nella forma HTTP/3" || fail "sito .nyct: $(tail -2 "$TMP/s.log")"
# 4) ponte nascosto nella forma HTTP/3 (contrassegno del ponte)
"$BIN" node --listen 127.0.0.1:$((B+5)) --data "$TMP/ponte" --obfuscation "$K" --directory "$DIR" --authority "$PUB" --hidden-bridge > "$TMP/ponte.log" 2>&1 & PIDS+=($!)
PONTE=$(attendi_regex "$TMP/ponte.log" "127\.0\.0\.1:$((B+5)),[0-9a-f]{64},[0-9a-f]{64},[0-9a-f]{64}")
NYCTALUS_PONTI="$PONTE" "$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$K" --socks 127.0.0.1:$((B+14)) --control 127.0.0.1:$((B+15)) --hops 3 > "$TMP/c3.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/c3.log" "SOCKS5 proxy" 20 > /dev/null
R=$(curl -s -m 20 --socks5-hostname 127.0.0.1:$((B+14)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R" = "ciao dal sito" ] && grep -q "hidden bridge" "$TMP/c3.log" && pass "ponte nascosto nella forma HTTP/3: si entra dal ponte" || fail "ponte HTTP/3: $R"
# 5) un vero browser fa il censore: visita un nodo come un sito HTTP/3
CERT="$TMP/n$B/tls-cert.der"
openssl x509 -inform der -in "$CERT" -noout -subject -issuer 2>/dev/null | grep -q "CN = $NOME" && ! grep -q "rcgen" "$CERT" \
  && pass "certificato del nodo con dati normali (CN = $NOME, non «rcgen self signed cert»)" || fail "certificato: $(openssl x509 -inform der -in "$CERT" -noout -subject 2>&1)"
if command -v google-chrome > /dev/null && command -v openssl > /dev/null; then
  SPKI=$(openssl x509 -inform der -in "$CERT" -pubkey -noout | openssl pkey -pubin -outform der | openssl dgst -sha256 -binary | base64)
  PAG=$(timeout 40 google-chrome --headless=new --user-data-dir="$TMP/chrome" --no-first-run --mute-audio --disable-gpu \
      --disable-background-networking --disable-component-update --disable-sync --no-default-browser-check \
      --host-resolver-rules="MAP $NOME 127.0.0.1" --origin-to-force-quic-on=$NOME:$B --ignore-certificate-errors-spki-list="$SPKI" \
      --dump-dom "https://$NOME:$B/" 2>/dev/null)
  echo "$PAG" | grep -q "404 Not Found" && pass "Chrome visita il nodo in HTTP/3 e vede una normale pagina 404: per un censore è un sito web" \
    || fail "Chrome non ha visto la pagina: $(echo "$PAG" | head -2)"
  R=$(curl -s -m 20 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/ 2>&1)
  [ "$R" = "ciao dal sito" ] && pass "dopo la visita del browser la rete funziona ancora" || fail "dopo la visita: $R"
else
  salta "Chrome o openssl non installati: niente prova con il browser"
fi
[ "$KO" -eq 0 ]
