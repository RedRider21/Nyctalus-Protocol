#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# CASSAFORTE DEI DATI LOCALI (piano di sicurezza, punto 7): identità, contatti, cronologia… della
# chat cifrati sul disco con una chiave che nasce dalla passphrase (Argon2id + XChaCha20-Poly1305).
# Il motore riparte CHIUSO e non legge niente finché non riceve la passphrase; il demone `site`
# prende l'identità dal motore (niente file in chiaro). Tutto in locale, con dati di prova.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · cassaforte dei dati locali (passphrase)"
assicura_binario
B=23420; D="$TMP/chat"; SEG=segretodiprova0123456789abcdef; C=$((B+30))
PP="una passphrase di prova"; PP2="la passphrase nuova di prova"
K=$(python3 -c "import os;print(os.urandom(32).hex())")
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
righe=""
for i in 0 1 2 3; do
  p=$((B+i)); "$BIN" node --listen 127.0.0.1:$p --data "$TMP/n$p" --obfuscation "$K" > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
done
for i in 0 1 2 3; do p=$((B+i)); L=$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+"); righe="$righe${righe:+;}$L"; done
"$BIN" directory --authority-secret "$SEC" --entries "$righe" > "$TMP/dir.txt" 2>/dev/null
motore() { NYCTALUS_CHAT_TOKEN="$SEG" "$BIN" chat --data "$D" --backend 127.0.0.1:$((C+1)) --control 127.0.0.1:$C --socks 127.0.0.1:$((C+2)) > "$TMP/chat$1.log" 2>&1 & MOT=$!; PIDS+=($MOT); }
ctl() { controllo $C "$SEG" "$@"; }
hex() { printf '%s' "$1" | xxd -p | tr -d '\n'; }

# 1) chat in chiaro, con un contatto e una cronologia (come dopo un po' di uso)
X=$("$BIN" address --identity "$TMP/x" | grep -oE '[a-z2-7]{56}\.nyct' | tail -1)
mkdir -p "$D/cronologia" "$D/stati"
printf 'in 1700000001 %s\nout 1700000002 %s\n' "$(hex 'ciao dalla cassaforte')" "$(hex 'secondo messaggio')" > "$D/cronologia/$X.log"
printf '1700000002 3 1700000003\n' > "$D/stati/$X.log"
motore 1
ME=$(attendi_regex "$TMP/chat1.log" '[a-z2-7]{56}\.nyct' 15)
ctl 3 "ADD $X $(hex mario)" > /dev/null
echo '{"finestra": 1}' > "$D/ui.json"
R=$(NYCT_FINO="^ENDH" ctl 5 "HIST $X")
echo "$R" | grep -q "$(hex 'ciao dalla cassaforte')" && grep -q "mario\|$(hex mario)" "$D/contatti" && pass "chat in chiaro: contatto e messaggio salvati" || fail "preparazione: $R"

# 2) si accende la cassaforte: tutto cifrato sul disco
R=$(NYCT_FINO="^(CIFRATA|ERR)" ctl 30 "CIFRA $(hex "$PP")")
echo "$R" | grep -q "^CIFRATA" && pass "CIFRA: cassaforte accesa ($(echo "$R" | grep -oE 'CIFRATA [0-9]+' | awk '{print $2}') file cifrati)" || fail "CIFRA: $R"
head -c 5 "$D/identita" | grep -q NYCF1 && ! grep -q "$(hex mario)" "$D/contatti" && head -c 5 "$D/contatti" | grep -q NYCF1 \
  && pass "sul disco identità e contatti sono cifrati (niente testo leggibile)" || fail "file in chiaro: $(head -c 40 "$D/contatti" | xxd | head -2)"
F="$D/cronologia/$X.log"
[ "$(grep -c "^NYCR1 " "$F")" = 2 ] && ! grep -qv "^NYCR1 " "$F" && [ "$(grep -c "^NYCR1 " "$D/stati/$X.log")" = 1 ] \
  && pass "cronologia e spunte cifrate riga per riga" || fail "cronologia: $(head -2 "$F")"
[ "$(cat "$D/ui.json")" = '{"finestra": 1}' ] && pass "i file delle interfacce (ui.json) restano intatti" || fail "ui.json toccato"
R=$(NYCT_FINO="^ENDH" ctl 5 "HIST $X")
echo "$R" | grep -q "$(hex 'ciao dalla cassaforte')" && pass "con la cassaforte aperta la chat legge tutto come prima" || fail "HIST dopo CIFRA: $R"

# 3) riavvio: il motore parte CHIUSO
kill $MOT; wait $MOT 2>/dev/null; sleep 0.5
motore 2
attendi_regex "$TMP/chat2.log" "aspetto la passphrase" 10 > /dev/null
R=$(NYCT_FINO="BLOCCATA" ctl 5 "WHOAMI")
echo "$R" | grep -q "CASSAFORTE chiusa" && echo "$R" | grep -q BLOCCATA && ! echo "$R" | grep -q "^ME " && pass "riavviato: chiuso, non risponde nemmeno con il suo indirizzo" || fail "chiuso: $R"
# il demone del .nyct chiede l'identità al motore: aspetta lo sblocco
mkdir -p "$TMP/www"; echo "sito della chat" > "$TMP/www/index.html"
NYCTALUS_CHAT_TOKEN="$SEG" "$BIN" site --dir "$TMP/www" --identity-chat 127.0.0.1:$C --directory "$TMP/dir.txt" --authority "$PUB" --obfuscation "$K" --hops 3 > "$TMP/site.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/site.log" "Aspetto l'identità" 10 > /dev/null && pass "il demone del .nyct aspetta che la chat venga sbloccata" || fail "site: $(tail -2 "$TMP/site.log")"
T0=$(date +%s.%N)
R=$(NYCT_FINO="^ERR" ctl 10 "SBLOCCA $(hex 'passphrase sbagliata')")
DT=$(python3 -c "print(round($(date +%s.%N)-$T0,1))")
echo "$R" | grep -q "^ERR passphrase sbagliata" && python3 -c "import sys;sys.exit(0 if $DT >= 1.0 else 1)" && pass "passphrase sbagliata: rifiutata, dopo un'attesa (${DT} s)" || fail "sbagliata: $R ($DT s)"
# un'altra interfaccia collegata mentre è chiusa (es. le notifiche del telefono)
( T1=$(date +%s); ctl 25 "CASSAFORTE" > "$TMP/altra.txt"; echo $(( $(date +%s) - T1 )) > "$TMP/altra.durata" ) &
ALTRA=$!; sleep 1
R=$(NYCT_FINO="SBLOCCATA|^ERR" ctl 30 "SBLOCCA $(hex "$PP")")
echo "$R" | grep -q SBLOCCATA && pass "passphrase giusta: cassaforte aperta" || fail "sblocco: $R"
wait $ALTRA
[ "$(cat "$TMP/altra.durata")" -lt 20 ] && pass "allo sblocco le altre interfacce collegate vengono scollegate (si ricollegano alla chat vera)" || fail "connessione chiusa rimasta appesa"
sleep 1
R=$(NYCT_FINO="^ME " ctl 10 "WHOAMI")
echo "$R" | grep -q "^ME $ME" && pass "dopo lo sblocco la chat riparte con la stessa identità ($ME)" || fail "dopo lo sblocco: $R"
R=$(NYCT_FINO="^ENDH" ctl 5 "HIST $X")
echo "$R" | grep -q "$(hex 'ciao dalla cassaforte')" && echo "$R" | grep -q "$(hex 'secondo messaggio')" && pass "cronologia intatta dopo il riavvio" || fail "cronologia: $R"
attendi_regex "$TMP/site.log" "online" 40 > /dev/null
R=$(timeout 45 "$BIN" browse "$ME" --directory "$TMP/dir.txt" --authority "$PUB" --obfuscation "$K" --hops 3 2>/dev/null)
echo "$R" | grep -q "sito della chat" && pass "il demone ha preso l'identità dal motore: il .nyct della chat ($ME) si raggiunge" || fail "browse: $R · $(tail -3 "$TMP/site.log")"

# 4) cambio della passphrase e rimozione della cassaforte
R=$(NYCT_FINO="^(CAMBIATA|ERR)" ctl 30 "CAMBIA $(hex 'non è quella') $(hex "$PP2")")
echo "$R" | grep -q "^ERR" && pass "CAMBIA con la passphrase attuale sbagliata: rifiutato" || fail "CAMBIA sbagliato: $R"
R=$(NYCT_FINO="^(CAMBIATA|ERR)" ctl 30 "CAMBIA $(hex "$PP") $(hex "$PP2")")
echo "$R" | grep -q "^CAMBIATA" && pass "CAMBIA: passphrase cambiata (tutto ricifrato)" || fail "CAMBIA: $R"
R=$(NYCT_FINO="^(DECIFRATA|ERR)" ctl 30 "DECIFRA $(hex "$PP2")")
echo "$R" | grep -q "^DECIFRATA" && ! head -c 5 "$D/contatti" | grep -q NYCF1 && grep -q "$(hex mario)" "$D/contatti" && ! grep -q "^NYCR1 " "$F" && [ ! -e "$D/cassaforte.json" ] \
  && pass "DECIFRA: cassaforte tolta, i dati tornano in chiaro" || fail "DECIFRA: $R"
# 5) lo stesso dal terminale (nyctalus-cli), passphrase letta dallo standard input
CLI() { NYCTALUS_CHAT_PORT=$C NYCTALUS_CHAT_TOKEN="$SEG" python3 "$ROOT/pc/cli/nyctalus-cli" "$@" 2>&1; }
CLI cassaforte | grep -q "spenta" && pass "nyctalus-cli cassaforte: «spenta»" || fail "cli stato: $(CLI cassaforte)"
printf 'corta\ncorta\n' | CLI cassaforte attiva | grep -q "troppo corta" && pass "cli: passphrase troppo corta rifiutata" || fail "cli corta"
printf '%s\n%s\n' "$PP" "$PP" | CLI cassaforte attiva | grep -q "protezione accesa" && CLI cassaforte | grep -q "ACCESA" \
  && pass "nyctalus-cli cassaforte attiva: protezione accesa" || fail "cli attiva: $(CLI cassaforte)"
kill $MOT; wait $MOT 2>/dev/null; sleep 0.5
motore 3
attendi_regex "$TMP/chat3.log" "aspetto la passphrase" 10 > /dev/null
CLI io | grep -q "prima sbloccali" && pass "cli a dati chiusi: ogni comando dice di sbloccare prima" || fail "cli chiuso: $(CLI io)"
printf 'non è lei\n' | CLI sblocca | grep -q "passphrase sbagliata" && pass "nyctalus-cli sblocca: passphrase sbagliata rifiutata" || fail "cli sblocca sbagliata"
printf '%s\n' "$PP" | CLI sblocca | grep -q "dati sbloccati" && sleep 1 && [ "$(CLI io)" = "$ME" ] && pass "nyctalus-cli sblocca: dati aperti, «io» risponde con l'indirizzo" || fail "cli sblocca: $(CLI io)"
printf '%s\n%s\n%s\n' "$PP" "$PP2" "$PP2" | CLI cassaforte cambia | grep -q "passphrase cambiata" && pass "nyctalus-cli cassaforte cambia" || fail "cli cambia"
printf '%s\n' "$PP2" | CLI cassaforte togli | grep -q "protezione tolta" && [ ! -e "$D/cassaforte.json" ] && pass "nyctalus-cli cassaforte togli: dati di nuovo in chiaro" || fail "cli togli"
[ "$KO" -eq 0 ]
