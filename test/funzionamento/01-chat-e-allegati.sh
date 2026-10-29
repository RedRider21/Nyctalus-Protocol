#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# La chat dal vivo su una rete di prova (4 nodi, circuiti a 3 salti, punto
# d'incontro): Alice e Bob si scrivono nei due sensi e si mandano allegati, che
# devono arrivare identici al byte.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · chat e allegati su rete di prova"
assicura_binario
rete_di_prova 19300
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:19380 --hops 3 --control 127.0.0.1:19390 > "$TMP/c.log" 2>&1 & PIDS+=($!)
sleep 2
SA=alicesegreto0123456789abcdef; SB=bobsegreto0123456789abcdef00
identita alice 19311 19321 19380 "$SA"; ALICE=$ADDR
identita bob 19312 19322 19380 "$SB"; BOB=$ADDR
[ -n "$ALICE" ] && [ -n "$BOB" ] || { fail "identità non partite"; exit 1; }
pass "due identità con il loro servizio .nyct"
sleep 4

# --- testo nei due sensi ---
controllo 19322 "$SB" 180 SUB > "$TMP/bob-eventi.txt" & PIDS+=($!)
sleep 0.5
R=$(NYCT_FINO="^(OK|ERR)" controllo 19321 "$SA" 30 "SEND $BOB {hex:ciao Bob, sono Alice}")
echo "$R" | grep -q '^OK' && pass "Alice invia a Bob" || fail "invio di Alice fallito: $R"
R=$(NYCT_FINO="^(OK|ERR)" controllo 19322 "$SB" 30 "SEND $ALICE {hex:ricevuto, ti leggo}")
echo "$R" | grep -q '^OK' && pass "Bob risponde ad Alice" || fail "risposta di Bob fallita: $R"

# --- allegati: binario da 3 MB e un'immagine ---
head -c 3145728 /dev/urandom > "$TMP/dati 3MB.bin"
python3 -c "import zlib,struct;w=h=64;raw=b''.join(b'\x00'+bytes([x*4,y*4,128])*1 for y in range(h) for x in range(w));
def ch(t,d): return struct.pack('>I',len(d))+t+d+struct.pack('>I',zlib.crc32(t+d))
open('$TMP/foto.png','wb').write(b'\x89PNG\r\n\x1a\n'+ch(b'IHDR',struct.pack('>IIBBBBB',w,h,8,2,0,0,0))+ch(b'IDAT',zlib.compress(b''.join(b'\x00'+b''.join(bytes([x*4,y*4,128]) for x in range(w)) for y in range(h))))+ch(b'IEND',b''))"
for f in "dati 3MB.bin:application/octet-stream" "foto.png:image/png"; do
  n=${f%%:*}; m=${f#*:}
  R=$(NYCT_FINO="^(OK|ERR)" controllo 19321 "$SA" 60 "SENDFILE $BOB $(printf %s "$TMP/$n" | xxd -p | tr -d '\n') $m")
  echo "$R" | grep -q '^OK' && pass "allegato «$n» inviato" || fail "invio di «$n» fallito: $R"
done
sleep 3
wait_eventi() { for _ in $(seq 1 40); do [ "$(grep -c '^FILE ' "$TMP/bob-eventi.txt")" -ge 2 ] && break; sleep 0.5; done; }
wait_eventi

# --- verifiche ---
grep -q "^ST $ALICE 1" "$TMP/bob-eventi.txt" && pass "Bob vede Alice in linea (identità autenticata)" || fail "Bob non vede Alice in linea"
grep -q "^MSG $ALICE .* $(printf 'ciao Bob, sono Alice' | xxd -p | tr -d '\n')$" "$TMP/bob-eventi.txt" && pass "Bob riceve il testo esatto" || fail "testo non ricevuto da Bob"
grep -q "^in .* $(printf 'ricevuto, ti leggo' | xxd -p | tr -d '\n')$" "$TMP/alice/cronologia/$BOB.log" && pass "Alice riceve la risposta (in cronologia)" || fail "risposta non ricevuta da Alice"
grep -q "^PROG $ALICE in " "$TMP/bob-eventi.txt" && pass "avanzamento del trasferimento segnalato" || fail "nessun evento di avanzamento"
for n in "dati 3MB.bin" "foto.png"; do
  R=$(find "$TMP/bob/allegati" -name "$n" -print -quit)
  if [ -n "$R" ] && [ "$(sha256sum < "$TMP/$n")" = "$(sha256sum < "$R")" ]; then pass "«$n» arrivato identico (SHA-256)"; else fail "«$n» mancante o DIVERSO"; fi
done
[ -z "$(find "$TMP/bob/allegati" -name '*.part')" ] && pass "nessun file parziale rimasto" || fail "file .part rimasti"
grep -q "^fin " "$TMP/bob/cronologia/$ALICE.log" && pass "allegati registrati in cronologia" || fail "allegati non in cronologia"
[ "$KO" -eq 0 ]
