#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# NODI SENZA USCITA (--no-exit, «uscita=no» nell'elenco firmato): per i nodi di casa. Fanno da
# guardia, intermedio e punto d'incontro, ma la navigazione degli altri non esce MAI dal loro
# indirizzo. Rete mista: le uscite sono solo i nodi che lo permettono. Rete tutta senza uscita:
# il tunnel parte lo stesso, i siti .nyct funzionano, Internet viene rifiutato con un errore chiaro.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · nodi senza uscita verso Internet (nodi di casa)"
assicura_binario
B=23900
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
declare -A RIGA
for p in $(seq $B $((B+4))); do
  opz=(); [ $p -le $((B+2)) ] && opz=(--no-exit)          # 3 nodi «di casa», 2 con uscita
  "$BIN" node --listen 127.0.0.1:$p "${opz[@]}" > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
done
for p in $(seq $B $((B+4))); do
  RIGA[$p]=$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+"); [ -z "${RIGA[$p]}" ] && { fail "nodo $p non partito"; exit 1; }
done
grep -q "NO EXIT" "$TMP/n$B.log" && pass "il nodo con --no-exit si presenta come «senza uscita» (punto d'incontro sì)" || fail "messaggio --no-exit assente"
# elenco firmato: i 3 di casa con il ruolo «noexit» (→ uscita=no)
voci=""; for p in $(seq $B $((B+4))); do r=${RIGA[$p]}; [ $p -le $((B+2)) ] && r="$r,,,noexit"; voci="$voci${voci:+;}$r"; done
"$BIN" directory --authority-secret "$SEC" --entries "$voci" > "$TMP/dir.txt" 2>/dev/null
[ "$(grep -c 'uscita=no' "$TMP/dir.txt")" -eq 3 ] && pass "elenco firmato: 3 nodi marcati «uscita=no»" || fail "elenco: $(cat "$TMP/dir.txt")"
mkdir -p "$TMP/www"; head -c 300000 /dev/urandom > "$TMP/www/f.bin"; echo "sito di casa" > "$TMP/www/index.html"
python3 -m http.server $((B+20)) --bind 127.0.0.1 --directory "$TMP/www" > /dev/null 2>&1 & PIDS+=($!)
for _ in $(seq 1 50); do porta_occupata $((B+20)) && break; sleep 0.2; done

# 1) rete mista: le uscite sono SEMPRE i due nodi che lo permettono
ok=1
for giro in 1 2 3 4 5; do
  rm -rf "$NYCTALUS_GUARDIE"
  "$BIN" circuit --directory "$TMP/dir.txt" --authority "$PUB" --socks 127.0.0.1:$((B+10)) --control 127.0.0.1:$((B+11)) --circuits 2 --hops 3 > "$TMP/c$giro.log" 2>&1 & CP=$!; PIDS+=($CP)
  attendi_regex "$TMP/c$giro.log" "SOCKS5 proxy" 30 > /dev/null
  for u in $(grep -oE "circuit [0-9]+: .*" "$TMP/c$giro.log" | awk '{print $NF}' | grep -oE "[0-9]+$"); do
    [ "$u" -le $((B+2)) ] && ok=0
  done
  [ "$giro" = 1 ] && { rm -f "$TMP/giu.bin"; curl -s -m 30 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/f.bin -o "$TMP/giu.bin"; }
  kill $CP; wait $CP 2>/dev/null
done
[ "$ok" = 1 ] && cmp -s "$TMP/www/f.bin" "$TMP/giu.bin" && pass "rete mista, 5 avvii: l'uscita è sempre un nodo che lo permette; lo scaricamento arriva" || fail "uscita su un nodo senza uscita o scaricamento fallito"

# 2) un client che (per un elenco senza il segno) prova a uscire da un nodo --no-exit: il nodo rifiuta
"$BIN" directory --authority-secret "$SEC" --entries "${RIGA[$B]};${RIGA[$((B+1))]};${RIGA[$((B+2))]}" > "$TMP/dir-vecchio.txt" 2>/dev/null
"$BIN" circuit --directory "$TMP/dir-vecchio.txt" --authority "$PUB" --socks 127.0.0.1:$((B+12)) --control 127.0.0.1:$((B+13)) --hops 3 > "$TMP/cv.log" 2>&1 & CP=$!; PIDS+=($CP)
attendi_regex "$TMP/cv.log" "SOCKS5 proxy" 30 > /dev/null
R=$(curl -s -m 15 -o /dev/null -w '%{http_code}' --socks5-hostname 127.0.0.1:$((B+12)) http://127.0.0.1:$((B+20))/index.html)
kill $CP; wait $CP 2>/dev/null
[ "$R" != 200 ] && grep -q "refusing to act as Internet exit" "$TMP"/n$B.log "$TMP"/n$((B+1)).log "$TMP"/n$((B+2)).log \
  && pass "anche se un client ci prova (elenco senza il segno), il nodo di casa RIFIUTA di uscire su Internet" || fail "il nodo senza uscita ha fatto uscire il traffico (codice $R)"

# 3) rete TUTTA senza uscita: il tunnel parte, i siti .nyct funzionano, Internet è rifiutato
voci=""; for p in $B $((B+1)) $((B+2)); do voci="$voci${voci:+;}${RIGA[$p]},,,noexit"; done
"$BIN" directory --authority-secret "$SEC" --entries "$voci" > "$TMP/dir-casa.txt" 2>/dev/null
"$BIN" site --dir "$TMP/www" --identity "$TMP/sito.id" --directory "$TMP/dir-casa.txt" --authority "$PUB" --hops 3 > "$TMP/site.log" 2>&1 & PIDS+=($!)
ADDR=$(attendi_regex "$TMP/site.log" '[a-z2-7]{56}\.nyct' 20); attendi_regex "$TMP/site.log" 'online' 60 > /dev/null
"$BIN" circuit --directory "$TMP/dir-casa.txt" --authority "$PUB" --socks 127.0.0.1:$((B+14)) --control 127.0.0.1:$((B+15)) --hops 3 > "$TMP/cc.log" 2>&1 & CP=$!; PIDS+=($CP)
attendi_regex "$TMP/cc.log" "SOCKS5 proxy" 30 > /dev/null
grep -q "only .nyct sites" "$TMP/cc.log" && pass "rete senza uscite: il tunnel parte lo stesso e lo dice («only .nyct sites»)" || fail "tunnel: $(tail -3 "$TMP/cc.log")"
S=$(curl -s -m 60 --socks5-hostname 127.0.0.1:$((B+14)) "http://$ADDR/index.html")
[ "$S" = "sito di casa" ] && pass "sito .nyct raggiunto attraverso la rete di soli nodi di casa" || fail "sito .nyct: «$S»"
E=$(curl -s -m 15 -o /dev/null -w '%{http_code}' --socks5-hostname 127.0.0.1:$((B+14)) http://127.0.0.1:$((B+20))/index.html; echo " $?")
case "$E" in *" 97") true ;; *) false ;; esac && grep -q "no Internet exit" "$TMP/cc.log" \
  && pass "navigazione su Internet: rifiutata subito con un errore chiaro (niente esce da casa)" || fail "Internet non rifiutato: $E"
kill $CP; wait $CP 2>/dev/null

# 4) il gestore: «nyctalus-rete casa --senza-uscita» (HOME finta), scelta ricordata, ospiti, ritorno
H="$TMP/gestore"; mkdir -p "$H"; G="$ROOT/pc/gestore-rete/nyctalus-rete"
avvia_casa() { # avvia_casa N [opzioni] → rete di casa accesa, elenco in $TMP/casaN.txt
  local n="$1"; shift
  NYCTALUS_RUBRICA_PORTE=24730,24731,24732,24733,24734 HOME="$H" NYCTALUS_BIN="$BIN" \
    python3 "$G" casa --ip 127.0.0.1 --porta 24720 --porta-base 24721 --non-importare "$@" < /dev/null > "$TMP/casa$n.log" 2>&1 &
  CASA=$!; PIDS+=($CASA)
  for _ in $(seq 1 240); do grep -q "Rete di casa attiva" "$TMP/casa$n.log" && break; sleep 0.5; done
  curl -sf http://127.0.0.1:24720/directory.txt -o "$TMP/casa$n.txt"
}
ferma_casa() { kill -INT $CASA 2>/dev/null; for _ in $(seq 1 40); do kill -0 $CASA 2>/dev/null || break; sleep 0.25; done; sleep 1; }
mkdir -p "$TMP/aiuto"
HOME="$TMP/aiuto" NYCTALUS_BIN="$BIN" timeout 15 python3 "$G" casa --senza-uscita --help < /dev/null > "$TMP/aiuto.txt" 2>&1 \
  && grep -q "nyctalus-rete" "$TMP/aiuto.txt" && [ ! -e "$TMP/aiuto/.nyctalus-rete" ] \
  && pass "«casa … --help» mostra l'aiuto e non accende (né riscrive) la rete" || fail "casa --help: $(head -2 "$TMP/aiuto.txt")"
avvia_casa 1 --senza-uscita
[ "$(grep -c '^v .*uscita=no' "$TMP/casa1.txt")" = 4 ] && grep -q "SENZA USCITA" "$TMP/casa1.log" \
  && grep -q "NO EXIT" "$H/.nyctalus-rete/casa/nodo-24721.log" \
  && pass "«casa --senza-uscita»: i 4 nodi partono con --no-exit e l'elenco firmato li segna «uscita=no»" || fail "casa senza uscita: $(grep -c uscita=no "$TMP/casa1.txt") segnati · $(tail -2 "$TMP/casa1.log")"
ferma_casa
avvia_casa 2
[ "$(grep -c '^v .*uscita=no' "$TMP/casa2.txt")" = 4 ] && pass "riaccesa senza opzioni: la scelta «senza uscita» è ricordata" || fail "scelta non ricordata"
# un amico con un nodo senza uscita: la riga porta il segno e resta tale
"$BIN" node --listen 127.0.0.1:24760 --data "$TMP/amico" --announce 127.0.0.1:24760 --no-exit > "$TMP/amico.log" 2>&1 & PIDS+=($!)
for _ in $(seq 1 100); do [ -s "$TMP/amico/riga-elenco.txt" ] && break; sleep 0.1; done
HOME="$H" NYCTALUS_BIN="$BIN" python3 "$G" ospite aggiungi "$(cat "$TMP/amico/riga-elenco.txt"),,,noexit" --famiglia mario > /dev/null 2>&1
curl -sf http://127.0.0.1:24720/directory.txt -o "$TMP/casa2b.txt"
grep -q "127.0.0.1:24760 .*famiglia=mario.*uscita=no" "$TMP/casa2b.txt" && [ "$(grep -c '^v .*uscita=no' "$TMP/casa2b.txt")" = 5 ] \
  && pass "nodo ospite senza uscita: resta «uscita=no» (con la sua famiglia); i nodi di casa pure" || fail "ospite: $(grep 24760 "$TMP/casa2b.txt")"
ferma_casa
avvia_casa 3 --con-uscita
! grep -q "127.0.0.1:2472[1-4] .*uscita=no" "$TMP/casa3.txt" && ! grep -q "SENZA USCITA" "$TMP/casa3.log" && grep -q "Rete di casa attiva" "$TMP/casa3.log" \
  && pass "«--con-uscita»: i nodi di casa tornano a fare da uscita (elenco senza «uscita=no»)" || fail "con uscita: $(grep 2472 "$TMP/casa3.txt" | cut -c1-60)"
ferma_casa
[ "$KO" -eq 0 ]
