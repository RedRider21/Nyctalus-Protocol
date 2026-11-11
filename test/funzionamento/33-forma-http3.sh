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
R=$(timeout 40 "$BIN" browse "$ADDR" --directory "$DIR" --authority "$PUB" --obfuscation "$K" --hops 3 2>"$TMP/b.log")
echo "$R" | grep -q "ciao dal sito" && pass "sito .nyct pubblicato e visitato nella forma HTTP/3" || fail "sito .nyct: $(tail -2 "$TMP/s.log") · visita: $(tail -2 "$TMP/b.log")"
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

# 5-bis) ripiego su TCP nella forma HTTP/3: un vero HTTPS (TLS, ALPN http/1.1) con il contrassegno dentro
NYCTALUS_TCP=1 "$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$K" --socks 127.0.0.1:$((B+16)) --control 127.0.0.1:$((B+17)) --hops 3 > "$TMP/ct.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/ct.log" "SOCKS5 proxy" 30 > /dev/null
R=$(curl -s -m 20 --socks5-hostname 127.0.0.1:$((B+16)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R" = "ciao dal sito" ] && grep -q "entry over TCP" "$TMP/ct.log" && pass "ripiego su TCP nella forma HTTP/3: si naviga dentro un vero TLS" || fail "TCP+TLS: $R $(tail -3 "$TMP/ct.log")"
S=$(echo | timeout 10 openssl s_client -connect 127.0.0.1:$B -servername $NOME -alpn http/1.1 2>/dev/null)
echo "$S" | grep -q "CN *= *$NOME" && echo "$S" | grep -q "ALPN protocol: http/1.1" \
  && pass "openssl sulla porta TCP: stretta TLS normale, certificato «$NOME», ALPN http/1.1" || fail "openssl: $(echo "$S" | grep -E 'subject|ALPN|error' | head -3)"
R=$(curl -sk -m 10 --resolve $NOME:$B:127.0.0.1 "https://$NOME:$B/" -w " %{http_code}")
echo "$R" | grep -q "404 Not Found" && echo "$R" | grep -q " 404$" && pass "curl in HTTPS sulla porta TCP del nodo: «404 Not Found», come un sito" || fail "curl https: $R"
if command -v google-chrome > /dev/null; then
  PAG=$(timeout 40 google-chrome --headless=new --user-data-dir="$TMP/chrome2" --no-first-run --mute-audio --disable-gpu \
      --disable-background-networking --disable-component-update --disable-sync --no-default-browser-check \
      --host-resolver-rules="MAP $NOME 127.0.0.1" --ignore-certificate-errors --dump-dom "https://$NOME:$B/" 2>/dev/null)
  echo "$PAG" | grep -q "404 Not Found" && pass "Chrome in HTTPS (TCP) sul nodo: una normale pagina 404" || fail "Chrome https: $(echo "$PAG" | head -2)"
fi

# 6) gestore, codice di rete, PC, telefono, installatore dei server (HOME finte: mai i dati veri)
unset NYCTALUS_MIMETICO NYCTALUS_NOME_MIMETICO
G="$ROOT/pc/gestore-rete/nyctalus-rete"; HG="$TMP/gestore"; mkdir -p "$HG"
HOME="$HG" NYCTALUS_BIN="$BIN" python3 "$G" crea --nome prova > /dev/null 2>&1
HOME="$HG" NYCTALUS_BIN="$BIN" python3 "$G" protezione si --forma h3 --nome $NOME > "$TMP/prot.log" 2>&1
HOME="$HG" NYCTALUS_BIN="$BIN" python3 "$G" codice --url http://127.0.0.1:9/directory.txt > "$TMP/cod.log" 2>&1
COD=$(grep -oE 'NYCT1:[A-Za-z0-9_-]+' "$TMP/cod.log" | head -1)
leggi() { python3 -c "import base64,json,sys;c=sys.argv[1][6:];print(json.loads(base64.urlsafe_b64decode(c+'='*(-len(c)%4))).get(sys.argv[2],''))" "$1" "$2"; }
[ "$(leggi "$COD" forma)" = h3 ] && [ "$(leggi "$COD" nome_h3)" = $NOME ] && [ "$(leggi "$COD" nome)" = prova ] \
  && pass "gestore: «protezione si --forma h3 --nome $NOME» finisce nel codice di rete (e il nome della rete resta)" || fail "codice: $(cat "$TMP/prot.log" "$TMP/cod.log" | tail -4)"
HOME="$HG" NYCTALUS_BIN="$BIN" python3 "$G" protezione si --forma xyz 2>&1 | grep -q "rumore.*h3" && pass "forma sconosciuta rifiutata" || fail "forma xyz accettata"
CAM=$(leggi "$COD" camuffamento)
mkdir -p "$TMP/cliente"
HOME="$TMP/cliente" python3 "$G" importa "$COD" > /dev/null 2>&1
python3 -c "import json,sys;c=json.load(open(sys.argv[1]));sys.exit(0 if c.get('forma')=='h3' and c.get('nome_h3')==sys.argv[2] and c.get('nome_rete')=='prova' else 1)" \
  "$TMP/cliente/.nyctalus-browser/config.json" $NOME && pass "PC: «importa» salva la forma HTTP/3 (chat, browser e Firefox la passano al motore)" || fail "importa: $(cat "$TMP/cliente/.nyctalus-browser/config.json")"
O=$(cd "$TMP" && HOME="$TMP/cliente" python3 -c "import sys;sys.path.insert(0,sys.argv[1]);import nyctalus_mobile as m;print(' '.join(m.opz_prot(m.leggi_codice(sys.argv[2]))))" "$ROOT/mobile/orchestratore" "$COD" 2>&1)
[ "$O" = "--obfuscation $CAM --mimic --mimic-name $NOME --cover 0.2" ] && pass "telefono/Windows: dal codice → --mimic --mimic-name $NOME" || fail "orchestratore: $O"
RADICE="$TMP/radice"; F="$TMP/finti"; mkdir -p "$RADICE" "$F"
printf '#!/bin/sh\nexit 0\n' > "$F/systemctl"; printf '#!/bin/sh\nexit 0\n' > "$F/useradd"; chmod +x "$F"/*
PK="$TMP/Nyctalus-Server"; mkdir -p "$PK/bin"; cp "$BIN" "$PK/bin/nyctalus"
cp "$ROOT/server/installa-nodo.sh" "$PK/installa.sh"; cp "$ROOT/server/nyctalus-server" "$ROOT/server/disinstalla.sh" "$PK/"
env NYCTALUS_RADICE="$RADICE" PATH="$F:$PATH" bash "$PK/installa.sh" --ip 127.0.0.1 --nodi 1 --si --codice "$COD" > "$TMP/inst.log" 2>&1
CONF="$RADICE/etc/nyctalus/server.conf"
grep -q "NYCTALUS_OPZIONI=.*--mimic --mimic-name $NOME" "$CONF" && grep -q 'NYCTALUS_PORTE="443"' "$CONF" \
  && pass "server: «installa.sh --codice» prende la forma HTTP/3 e propone la porta 443" || fail "installatore: $(grep -E 'OPZIONI|PORTE' "$CONF" 2>&1; tail -3 "$TMP/inst.log")"
openssl x509 -inform der -in "$RADICE/var/lib/nyctalus/nodo-443/tls-cert.der" -noout -subject 2>/dev/null | grep -q "CN = $NOME" \
  && pass "server: il certificato del nodo nasce con il nome del sito" || fail "certificato del server: $(ls "$RADICE/var/lib/nyctalus/nodo-443/" 2>&1)"
R1=$(cat "$RADICE/var/lib/nyctalus/nodo-443/riga-elenco.txt")
env NYCTALUS_RADICE="$RADICE" PATH="$F:$PATH" bash "$PK/installa.sh" --ip 127.0.0.1 --nodi 1 --si --codice "$COD" --nuova-identita > /dev/null 2>&1
[ -n "$R1" ] && [ "$R1" != "$(cat "$RADICE/var/lib/nyctalus/nodo-443/riga-elenco.txt")" ] && pass "server: «--nuova-identita» rifà chiavi e certificato (riga nuova)" || fail "nuova identità"
[ "$KO" -eq 0 ]
