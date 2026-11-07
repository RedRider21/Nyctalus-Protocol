#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Nodi di persone diverse (PIANO-ANONIMATO, Tappa 3): alla rete di casa (4 nodi su questo PC) si
# aggiungono i nodi di due amici, «mario» (2 nodi) e «luca» (1 nodo), con `nyctalus-rete ospite`.
# L'elenco pubblicato li include subito, firmato, con le famiglie (i nodi del PC = «casa»); ogni
# percorso usa nodi di 3 gestori diversi; il traffico passa davvero dai nodi ospiti; si tolgono;
# restano dopo un riavvio; senza ospiti l'elenco resta quello di sempre. Tutto in 127.0.0.1.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · nodi di persone diverse nella rete di casa (ospiti con famiglia)"
assicura_binario
H="$TMP/gestore"; mkdir -p "$H"
G="$ROOT/pc/gestore-rete/nyctalus-rete"
avvia_casa() {
  NYCTALUS_RUBRICA_PORTE=24630,24631,24632,24633,24634 HOME="$H" NYCTALUS_BIN="$BIN" \
    python3 "$G" casa --ip 127.0.0.1 --porta 24620 --porta-base 24621 --non-importare < /dev/null > "$TMP/casa$1.log" 2>&1 &
  CASA=$!; PIDS+=($CASA)
  for _ in $(seq 1 240); do grep -q "Rete di casa attiva" "$TMP/casa$1.log" && break; sleep 0.5; done
}
avvia_casa 1
grep -q "Rete di casa attiva" "$TMP/casa1.log" || { fail "rete di casa: $(tail -3 "$TMP/casa1.log")"; exit 1; }
PUB=$(python3 -c "import json;print(json.load(open('$H/.nyctalus-rete/casa/autorita.json'))['pubblica'])")
curl -sf http://127.0.0.1:24620/directory.txt -o "$TMP/d0.txt"
[ "$(grep -c '^v ' "$TMP/d0.txt")" = 4 ] && ! grep -q famiglia= "$TMP/d0.txt" \
  && pass "senza ospiti l'elenco è quello di sempre (4 nodi, nessuna famiglia: leggibile anche dai client vecchi)" || fail "elenco iniziale: $(cat "$TMP/d0.txt")"

# i nodi degli amici (come li crea il loro installa.sh): ognuno stampa la sua riga d'elenco
nodo_amico() { # porta → riga in $RIGA
  mkdir -p "$TMP/amico-$1"
  "$BIN" node --listen 127.0.0.1:$1 --data "$TMP/amico-$1" --announce 127.0.0.1:$1 > "$TMP/amico-$1.log" 2>&1 & PIDS+=($!)
  for _ in $(seq 1 100); do [ -s "$TMP/amico-$1/riga-elenco.txt" ] && break; sleep 0.1; done
  RIGA=$(cat "$TMP/amico-$1/riga-elenco.txt")
}
nodo_amico 24661; M1=$RIGA; nodo_amico 24662; M2=$RIGA; nodo_amico 24663; L1=$RIGA
OUT=$(HOME="$H" NYCTALUS_BIN="$BIN" python3 "$G" ospite aggiungi "$M1" "$M2" --famiglia mario 2>&1)
echo "$OUT" | grep -q "2 nodi di «mario» aggiunti" && pass "«ospite aggiungi … --famiglia mario»: i 2 nodi di Mario entrano nella rete" || fail "aggiungi: $OUT"
HOME="$H" NYCTALUS_BIN="$BIN" python3 "$G" ospite aggiungi "$L1" --famiglia luca > /dev/null 2>&1
OUT=$(HOME="$H" NYCTALUS_BIN="$BIN" python3 "$G" ospite aggiungi "$L1" --famiglia casa 2>&1)
echo "$OUT" | grep -q "famiglia" && pass "la famiglia «casa» è riservata ai nodi del PC: rifiutata per un ospite" || fail "famiglia casa accettata"
curl -sf http://127.0.0.1:24620/directory.txt -o "$TMP/d1.txt"
[ "$(grep -c 'famiglia=casa' "$TMP/d1.txt")" = 4 ] && [ "$(grep -c 'famiglia=mario' "$TMP/d1.txt")" = 2 ] && [ "$(grep -c 'famiglia=luca' "$TMP/d1.txt")" = 1 ] \
  && pass "l'elenco pubblicato (senza riavviare) ha 7 nodi: 4 «casa», 2 «mario», 1 «luca»" || fail "elenco con ospiti: $(grep -o 'famiglia=[a-z]*' "$TMP/d1.txt" | sort | uniq -c | tr '\n' ' ')"
"$BIN" nodes --directory "$TMP/d1.txt" --authority "$PUB" > "$TMP/n1.txt" 2>&1 && pass "elenco firmato e valido per la chiave della rete" || fail "firma: $(cat "$TMP/n1.txt")"
OUT=$(HOME="$H" NYCTALUS_BIN="$BIN" python3 "$G" ospite elenco 2>&1)
echo "$OUT" | grep -q "mario" && echo "$OUT" | grep -q "luca" && echo "$OUT" | grep -q "Famiglie in rete: 3" \
  && pass "«ospite elenco»: gli ospiti per famiglia e quante famiglie ci sono (3)" || fail "elenco ospiti: $OUT"

# ogni percorso: 3 nodi di 3 gestori diversi
fam_di() { grep -E "^v $1 " "$TMP/d1.txt" | grep -oE 'famiglia=[a-z]+' | cut -d= -f2; }
: > "$TMP/percorsi.txt"
for i in $(seq 1 15); do
  rm -rf "$NYCTALUS_GUARDIE"
  (timeout 3 "$BIN" circuit --directory "$TMP/d1.txt" --authority "$PUB" --socks 127.0.0.1:0 --control 127.0.0.1:0 --circuits 2 --hops 3 2>&1 || true) >> "$TMP/percorsi.txt"
done
BUONI=0; TOT=0
while read -r riga; do
  TOT=$((TOT+1))
  f=$(echo "$riga" | grep -oE "127\.0\.0\.1:[0-9]+" | while read -r a; do fam_di "$a"; done | sort -u | wc -l)
  [ "$f" = 3 ] && BUONI=$((BUONI+1))
done < <(grep "circuit [0-9]:" "$TMP/percorsi.txt")
[ "$TOT" -ge 20 ] && [ "$BUONI" = "$TOT" ] && ! grep -q WARNING "$TMP/percorsi.txt" \
  && pass "in $TOT percorsi (guardie diverse) sempre 3 gestori diversi, nessun avviso" || fail "percorsi: $BUONI/$TOT buoni $(grep -m1 WARNING "$TMP/percorsi.txt")"

# il traffico passa davvero: si scarica l'elenco stesso attraverso la rete, con nodi ospiti nei percorsi
"$BIN" start --directory "$TMP/d1.txt" --authority "$PUB" --socks 127.0.0.1:24670 --control 127.0.0.1:24671 --circuits 3 > "$TMP/start.log" 2>&1 & PIDS+=($!)
for _ in $(seq 1 60); do grep -q "circuit 2:" "$TMP/start.log" && break; sleep 0.3; done
sleep 3
OK=0; for _ in $(seq 1 10); do curl -sf --max-time 20 --socks5-hostname 127.0.0.1:24670 http://127.0.0.1:24620/directory.txt -o "$TMP/via.txt" && cmp -s "$TMP/via.txt" "$TMP/d1.txt" && OK=1 && break; sleep 2; done
OSP=$(grep "circuit [0-9]:" "$TMP/start.log" | grep -cE "2466[123]")
[ "$OK" = 1 ] && [ "$OSP" -ge 1 ] && pass "traffico vero attraverso 3 salti con nodi ospiti nei percorsi ($OSP circuiti su 3)" || fail "traffico: ok=$OK ospiti=$OSP $(tail -3 "$TMP/start.log")"

# togliere un ospite; riavvio: gli ospiti restano
HOME="$H" NYCTALUS_BIN="$BIN" python3 "$G" ospite togli 127.0.0.1:24663 > /dev/null 2>&1
curl -sf http://127.0.0.1:24620/directory.txt -o "$TMP/d2.txt"
! grep -q "127.0.0.1:24663" "$TMP/d2.txt" && "$BIN" nodes --directory "$TMP/d2.txt" --authority "$PUB" > /dev/null 2>&1 \
  && pass "«ospite togli»: il nodo esce dall'elenco, che resta firmato" || fail "togli"
kill -INT "$CASA" 2>/dev/null; wait "$CASA" 2>/dev/null; sleep 1
avvia_casa 2
curl -sf http://127.0.0.1:24620/directory.txt -o "$TMP/d3.txt"
grep -q "Rete di casa attiva" "$TMP/casa2.log" && grep -q "nodi ospiti nella rete: 2" "$TMP/casa2.log" && [ "$(grep -c 'famiglia=mario' "$TMP/d3.txt")" = 2 ] && [ "$(grep -c 'famiglia=casa' "$TMP/d3.txt")" = 4 ] \
  && pass "dopo il riavvio della rete di casa gli ospiti ci sono ancora (con le famiglie)" || fail "riavvio: $(grep -o 'famiglia=[a-z]*' "$TMP/d3.txt" | sort | uniq -c | tr '\n' ' ') $(tail -2 "$TMP/casa2.log")"
[ "$KO" -eq 0 ]
