#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# nyctalus-sito: un sito .nyct creato con un comando, messo online sulla rete di casa e
# visitato attraverso la rete (pagina d'esempio, pagina modificata, sito dinamico dietro un
# server web locale); la chiave esportata riprende lo stesso indirizzo; servizio di sistema;
# rimozione con copia della chiave.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · siti .nyct (nyctalus-sito)"
assicura_binario
G="$TMP/gestore"; mkdir -p "$G"
NYCTALUS_RUBRICA_PORTE=19930,19931,19932,19933,19934 HOME="$G" NYCTALUS_BIN="$BIN" \
  python3 "$ROOT/pc/gestore-rete/nyctalus-rete" casa --ip 127.0.0.1 --porta 19920 --porta-base 19921 --non-importare \
  < /dev/null > "$TMP/casa.log" 2>&1 & PIDS+=($!)
for _ in $(seq 1 240); do grep -q "Rete di casa attiva" "$TMP/casa.log" && break; sleep 0.5; done
grep -q "Rete di casa attiva" "$TMP/casa.log" || { fail "rete di casa: $(tail -3 "$TMP/casa.log")"; exit 1; }
COD=$(grep -oE 'NYCT1:[A-Za-z0-9_-]+' "$TMP/casa.log" | head -1)
PUB=$(python3 -c "import json;print(json.load(open('$G/.nyctalus-rete/casa/autorita.json'))['pubblica'])")
curl -s http://127.0.0.1:19920/directory.txt -o "$TMP/dir.txt"
H="$TMP/autore"; mkdir -p "$H"
SITO() { HOME="$H" NYCTALUS_BIN="$BIN" python3 "$ROOT/pc/gestore-rete/nyctalus-sito" "$@"; }
HOME="$H" NYCTALUS_BIN="$BIN" python3 "$ROOT/pc/gestore-rete/nyctalus-rete" importa "$COD" > /dev/null

SITO crea blog > "$TMP/crea.log" 2>&1
ADDR=$(grep -oE '[a-z2-7]{56}\.nyct' "$TMP/crea.log" | head -1)
[ -n "$ADDR" ] && [ "$(SITO indirizzo blog)" = "$ADDR" ] && pass "crea: indirizzo .nyct subito, sulla rete del PC" || fail "crea: $(cat "$TMP/crea.log")"
D="$H/.nyctalus-siti/blog"
[ -f "$D/www/index.html" ] && grep -q "$ADDR" "$D/www/index.html" && [ "$(stat -c %a "$D")" = 700 ] && [ "$(stat -c %a "$D/identita")" = 600 ] \
  && pass "pagina d'esempio pronta; cartella privata e chiave 600" || fail "file del sito"
! grep -qE 'https?://' "$D/www/index.html" "$D/www/stile.css" && pass "la pagina d'esempio non carica nulla da altri siti" || fail "risorse esterne nella pagina d'esempio"
SITO crea blog > /dev/null 2>&1 && fail "un secondo «crea blog» non deve sovrascrivere" || pass "un sito già esistente non si sovrascrive"
SITO crea 'Nome Brutto' > /dev/null 2>&1 && fail "nome non valido accettato" || pass "nome non valido rifiutato"
SITO crea x --backend 10.0.0.5:80 > /dev/null 2>&1 && fail "backend esterno accettato" || pass "--backend solo su questo computer (127.0.0.1)"

HOME="$H" NYCTALUS_BIN="$BIN" python3 "$ROOT/pc/gestore-rete/nyctalus-sito" avvia blog > "$TMP/avvia.log" 2>&1 & PIDS+=($!); AVVIO=$!
attendi_regex "$TMP/avvia.log" "ONLINE" 60 > /dev/null
grep -q "ONLINE" "$TMP/avvia.log" && pass "avvia: il sito è online" || fail "avvia: $(tail -3 "$TMP/avvia.log")"
SITO elenco | grep -q "online" && pass "elenco: indirizzo e stato «online»" || fail "elenco: $(SITO elenco)"
visita() { timeout 90 "$BIN" browse "$ADDR" --directory "$TMP/dir.txt" --authority "$PUB" --path "${1:-/}" 2>/dev/null; }
visita / | grep -q "Benvenuto sul mio sito" && pass "visitato attraverso la rete: la pagina d'esempio" || fail "visita: $(visita / | head -3)"
echo '<h1>Pagina mia</h1>' > "$D/www/mia.html"
visita /mia.html | grep -q "Pagina mia" && pass "una pagina aggiunta alla cartella si vede subito" || fail "pagina aggiunta"
kill $AVVIO; sleep 2
! kill -0 $AVVIO 2>/dev/null && [ ! -f "$D/pid" ] && pass "Ctrl-C/TERM: si ferma e non lascia nulla acceso" || fail "arresto"
pgrep -f -- "--identity $D/identita" > /dev/null && fail "processo del sito rimasto acceso" || pass "nessun processo del sito rimasto"

# sito dinamico dietro un server web locale
mkdir -p "$TMP/app"; echo "risposta del server web" > "$TMP/app/index.html"
python3 -m http.server 19950 --bind 127.0.0.1 --directory "$TMP/app" > /dev/null 2>&1 & PIDS+=($!)
SITO crea app --backend 127.0.0.1:19950 > "$TMP/crea2.log" 2>&1
ADDR=$(SITO indirizzo app)
HOME="$H" NYCTALUS_BIN="$BIN" python3 "$ROOT/pc/gestore-rete/nyctalus-sito" avvia app > "$TMP/avvia2.log" 2>&1 & PIDS+=($!); AVVIO=$!
attendi_regex "$TMP/avvia2.log" "ONLINE" 60 > /dev/null
visita / | grep -q "risposta del server web" && pass "sito dinamico: Nyctalus porta i visitatori al server web locale" || fail "backend: $(tail -2 "$TMP/avvia2.log")"
kill $AVVIO

# la chiave riporta lo stesso indirizzo; servizio; rimozione
SITO esporta blog "$TMP/chiave" > /dev/null && [ "$(stat -c %a "$TMP/chiave")" = 600 ] && pass "esporta: copia della chiave (600)" || fail "esporta"
VECCHIO=$(SITO indirizzo blog)
SITO togli blog --si > "$TMP/togli.log" 2>&1
[ ! -d "$D" ] && ls "$H/.nyctalus-siti/.chiavi-tolte/" | grep -q "^blog-" && pass "togli: sito cancellato, copia della chiave conservata" || fail "togli: $(cat "$TMP/togli.log")"
SITO crea blog2 --chiave "$TMP/chiave" > /dev/null 2>&1
[ "$(SITO indirizzo blog2)" = "$VECCHIO" ] && pass "con la chiave esportata il sito riprende lo stesso indirizzo" || fail "chiave importata"
U=$(SITO servizio blog2 --stampa)
echo "$U" | grep -q "^ExecStart=.*nyctalus-sito avvia blog2$" && echo "$U" | grep -q "^Restart=on-failure" && echo "$U" | grep -q "WantedBy=default.target" \
  && pass "servizio: unità systemd dell'utente (si riavvia da sola, parte all'accensione)" || fail "unità: $U"
SITO crea altro --codice "NYCT1:rotto" > /dev/null 2>&1 && fail "codice rotto accettato" || pass "codice di rete rovinato rifiutato"
[ "$KO" -eq 0 ]
