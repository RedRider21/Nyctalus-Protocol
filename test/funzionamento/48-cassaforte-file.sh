#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# CASSAFORTE, FILE RICEVUTI: con la protezione accesa gli allegati della chat e i file grandi del
# programma File arrivano e restano CIFRATI sul disco (a pezzi, mai in chiaro); il comando APRI li
# decifra al volo in una cartella temporanea, identici all'originale; un file ricevuto si può
# inoltrare. Due utenti veri su una rete di prova locale; Bob ha la cassaforte.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · cassaforte: file ricevuti cifrati e aperti al volo"
assicura_binario
export XDG_RUNTIME_DIR="$TMP/run"; mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
rete_di_prova 23600
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:23680 --hops 3 --control 127.0.0.1:23690 > "$TMP/c.log" 2>&1 & PIDS+=($!)
sleep 2
SA=alicesegreto0123456789abcdef; SB=bobsegreto0123456789abcdef00
identita alice 23611 23621 23680 "$SA"; ALICE=$ADDR
identita bob 23612 23622 23680 "$SB"; BOB=$ADDR
[ -n "$ALICE" ] && [ -n "$BOB" ] || { fail "identità non partite"; exit 1; }
hx() { printf %s "$1" | xxd -p | tr -d '\n'; }
sleep 4
# un allegato arrivato PRIMA della cassaforte (resterà in chiaro finché non la si accende)
head -c 90000 /dev/urandom > "$TMP/prima.bin"
controllo 23622 "$SB" 240 SUB > "$TMP/bob-eventi.txt" & PIDS+=($!)
sleep 0.5
NYCT_FINO="^(OK|ERR)" controllo 23621 "$SA" 60 "SENDFILE $BOB $(hx "$TMP/prima.bin") application/octet-stream" > /dev/null
for _ in $(seq 1 60); do grep -q '^FILE ' "$TMP/bob-eventi.txt" && break; sleep 0.5; done
P0=$(find "$TMP/bob/allegati" -name "prima.bin" -print -quit)
[ -n "$P0" ] && cmp -s "$P0" "$TMP/prima.bin" && pass "senza cassaforte un allegato arriva in chiaro (come sempre)" || fail "allegato iniziale: $P0"

# Bob accende la cassaforte: il file già ricevuto viene sigillato
R=$(NYCT_FINO="^(CIFRATA|ERR)" controllo 23622 "$SB" 40 "CIFRA $(hx 'passphrase di Bob')")
echo "$R" | grep -q "^CIFRATA" && [ "$(head -c 5 "$P0")" = NYCS1 ] && ! cmp -s "$P0" "$TMP/prima.bin" \
  && pass "Bob accende la cassaforte: il file già ricevuto diventa cifrato sul disco" || fail "attivazione: $R"

# allegati nuovi (piccolo e da 3 MB): arrivano già cifrati
head -c 3145728 /dev/urandom > "$TMP/dati 3MB.bin"; head -c 5000 /dev/urandom > "$TMP/piccolo.bin"
for n in "dati 3MB.bin" "piccolo.bin"; do
  NYCT_FINO="^(OK|ERR)" controllo 23621 "$SA" 90 "SENDFILE $BOB $(hx "$TMP/$n") application/octet-stream" > /dev/null
done
for _ in $(seq 1 120); do [ "$(grep -c '^FILE ' "$TMP/bob-eventi.txt")" -ge 3 ] && break; sleep 0.5; done
apri() { NYCT_FINO="^(APERTO|NONAPERTO)" controllo 23622 "$SB" 30 "APRI $(hx "$1")" | grep -oE "^APERTO [0-9a-f]+ [0-9a-f]+" | awk '{print $3}' | xxd -r -p; }
for n in "dati 3MB.bin" "piccolo.bin"; do
  F=$(find "$TMP/bob/allegati" -name "$n" -print -quit)
  if [ -n "$F" ] && [ "$(head -c 5 "$F")" = NYCS1 ] && ! cmp -s "$F" "$TMP/$n"; then pass "«$n» arrivato e CIFRATO sul disco"; else fail "«$n» non cifrato o mancante ($F)"; fi
  C=$(apri "$F")
  [ -n "$C" ] && cmp -s "$C" "$TMP/$n" && case "$C" in "$XDG_RUNTIME_DIR"/*) true ;; *) false ;; esac \
    && pass "APRI «$n»: copia in chiaro nella cartella temporanea, identica all'originale" || fail "APRI «$n»: $C"
done
C0=$(apri "$P0"); cmp -s "$C0" "$TMP/prima.bin" && pass "anche il file ricevuto prima della cassaforte si apre al volo" || fail "apri prima.bin: $C0"
[ -z "$(find "$TMP/bob/allegati" -name '*.part')" ] && pass "nessun file parziale rimasto" || fail "file .part rimasti"
R=$(NYCT_FINO="^(APERTO|NONAPERTO)" controllo 23622 "$SB" 10 "APRI $(hx /etc/passwd)")
echo "$R" | grep -q "^NONAPERTO" && pass "APRI rifiuta i file fuori dalla cartella dei ricevuti" || fail "APRI /etc/passwd: $R"

# file GRANDE (programma File): pezzi cifrati mentre arrivano
head -c 6000000 /dev/urandom > "$TMP/grande.iso"
# i file grandi partono da soli solo fra contatti: i due si aggiungono in rubrica
NYCT_FINO="^(OK|ERR)" controllo 23621 "$SA" 10 "ADD $BOB $(hx Bob)" > /dev/null
NYCT_FINO="^(OK|ERR)" controllo 23622 "$SB" 10 "ADD $ALICE $(hx Alice)" > /dev/null
sleep 1
controllo 23621 "$SA" 200 SUB "XSEND $BOB $(hx "$TMP/grande.iso") application/octet-stream" > "$TMP/alice-x.txt" & PIDS+=($!)
for _ in $(seq 1 240); do grep -q '^XFINE ' "$TMP/bob-eventi.txt" && break; sleep 0.5; done
G=$(grep -oE '^XFINE [0-9a-f]+ [0-9a-f]+' "$TMP/bob-eventi.txt" | tail -1 | awk '{print $3}' | xxd -r -p)
[ -n "$G" ] && [ "$(head -c 5 "$G")" = NYCS1 ] && pass "file grande (6 MB) arrivato a pezzi, cifrato sul disco" || fail "file grande: $G · Bob: $(grep -E '^X' "$TMP/bob-eventi.txt" | tail -2 | cut -c1-120) · Alice: $(grep -E '^(X|ERR)' "$TMP/alice-x.txt" | tail -3 | cut -c1-160)"
CG=$(apri "$G"); cmp -s "$CG" "$TMP/grande.iso" && pass "APRI del file grande: identico all'originale" || fail "apri grande: $CG"

# inoltro: Bob rimanda ad Alice un file che ha ricevuto (cifrato da lui) → ad Alice arriva giusto
F=$(find "$TMP/bob/allegati" -name "piccolo.bin" -print -quit)
NYCT_FINO="^(OK|ERR)" controllo 23622 "$SB" 60 "SENDFILE $ALICE $(hx "$F") application/octet-stream" > /dev/null
for _ in $(seq 1 60); do A=$(find "$TMP/alice/allegati" -name "piccolo.bin" -print -quit); [ -n "$A" ] && break; sleep 0.5; done
[ -n "$A" ] && cmp -s "$A" "$TMP/piccolo.bin" && pass "inoltro di un file ricevuto: ad Alice arriva identico (e in chiaro: lei non ha la cassaforte)" || fail "inoltro: $A"
# niente in chiaro fuori dalla cartella temporanea
! grep -rlq "$(head -c 24 "$TMP/piccolo.bin" | xxd -p)" <(find "$TMP/bob/allegati" -type f -exec xxd -p -c 100000 {} + 2>/dev/null | head -c 2000000) \
  && pass "nella cartella dei dati di Bob il contenuto non compare in chiaro" || fail "contenuto in chiaro nei dati di Bob"
[ "$KO" -eq 0 ]
