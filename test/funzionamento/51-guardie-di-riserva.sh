#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# GUARDIE DI RISERVA (guardie primarie, come Tor): il client ricorda 3 entrate fisse in ordine e
# usa sempre la prima. Se la prima è spenta il tunnel parte lo stesso dalla SECONDA (mai da un
# nodo a caso), e anche un sito .nyct si raggiunge; quando la prima torna, al prossimo avvio si
# rientra da lei. Con tutte e tre spente il client non ripiega su altri nodi: si ferma.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · guardie di riserva (entrata fissa spenta)"
assicura_binario
B=23800
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
declare -A PIDN
avvia_nodo() { "$BIN" node --listen 127.0.0.1:$1 --data "$TMP/d$1" > "$TMP/n$1.log" 2>&1 & PIDS+=($!); PIDN[$1]=$!; }
righe=""
for p in $(seq $B $((B+6))); do avvia_nodo $p; done
for p in $(seq $B $((B+6))); do
  L=$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+"); [ -z "$L" ] && { fail "nodo $p non partito"; exit 1; }
  righe="$righe${righe:+;}$L"
done
DIR="$TMP/dir.txt"; "$BIN" directory --authority-secret "$SEC" --entries "$righe" > "$DIR" 2>/dev/null
mkdir -p "$TMP/www"; head -c 300000 /dev/urandom > "$TMP/www/f.bin"
python3 -m http.server $((B+20)) --bind 127.0.0.1 --directory "$TMP/www" > /dev/null 2>&1 & PIDS+=($!)

tunnel() {   # tunnel N → avvia, scarica, stampa l'entrata usata; il processo resta in $CPID
  "$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:$((B+10)) --control 127.0.0.1:$((B+11)) --hops 3 > "$TMP/c$1.log" 2>&1 & CPID=$!; PIDS+=($CPID)
  attendi_regex "$TMP/c$1.log" "SOCKS5 proxy|error" 40 > /dev/null
  rm -f "$TMP/giu.bin"
  curl -s -m 40 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/f.bin -o "$TMP/giu.bin"
}
entrata() { grep -oE "circuit 0: 127\.0\.0\.1:[0-9]+" "$1" | tail -1 | grep -oE "[0-9]+$"; }
ferma() { kill "$CPID" 2>/dev/null; wait "$CPID" 2>/dev/null; sleep 0.5; }

# 1) prima volta: si scelgono 3 guardie primarie, si entra dalla prima
tunnel 1
G=$(cat "$NYCTALUS_GUARDIE"/* 2>/dev/null | wc -l)
E1=$(entrata "$TMP/c1.log")
cmp -s "$TMP/www/f.bin" "$TMP/giu.bin" && [ "$G" -eq 3 ] && [ -n "$E1" ] \
  && pass "il client ricorda 3 guardie primarie ed entra dalla prima ($E1); lo scaricamento arriva" || fail "avvio: $G guardie, entrata «$E1» · $(tail -2 "$TMP/c1.log")"
ferma
# le tre primarie, per porta (dall'ordine del file e dalle righe dell'elenco)
PRIM=$(python3 - "$NYCTALUS_GUARDIE" "$DIR" <<'EOF'
import glob, re, sys
# righe dell'elenco firmato: «v 127.0.0.1:PORTA X25519 IMPRONTA»
voci = re.findall(r"127\.0\.0\.1:(\d+) ([0-9a-f]{64})", open(sys.argv[2]).read())
chiavi = [l.split()[0] for f in glob.glob(sys.argv[1] + "/*") for l in open(f) if l.strip()]
print(" ".join(next((p for p, x in voci if x == k), "?") for k in chiavi))
EOF
)
read -r P1 P2 P3 <<< "$PRIM"
[ "$P1" = "$E1" ] && pass "la prima guardia del file è quella usata ($P1; riserve $P2 e $P3)" || fail "primarie «$PRIM», entrata $E1"

# 2) la prima è spenta: si entra dalla SECONDA, non da un nodo qualunque
kill "${PIDN[$P1]}"; wait "${PIDN[$P1]}" 2>/dev/null; sleep 1
tunnel 2
E2=$(entrata "$TMP/c2.log")
cmp -s "$TMP/www/f.bin" "$TMP/giu.bin" && [ "$E2" = "$P2" ] && grep -q "trying the next primary guard" "$TMP/c2.log" \
  && pass "prima guardia spenta: il tunnel parte dalla SECONDA ($P2) e lo scaricamento arriva" || fail "riserva: entrata $E2 (attesa $P2) · $(grep -E 'entry|error' "$TMP/c2.log" | tail -2)"
[ "$(cat "$NYCTALUS_GUARDIE"/* | wc -l)" -eq 3 ] && [ "$(head -1 "$NYCTALUS_GUARDIE"/* | cut -d' ' -f1)" = "$(echo "$righe" | tr ';' '\n' | grep "127.0.0.1:$P1," | cut -d, -f2)" ] \
  && pass "le guardie ricordate non cambiano: la prima resta prima (nessuna rotazione per un guasto)" || fail "guardie cambiate dopo il guasto"
ferma

# 3) anche un sito .nyct si raggiunge con la prima guardia spenta
"$BIN" site --dir "$TMP/www" --identity "$TMP/sito.id" --directory "$DIR" --authority "$PUB" --hops 3 > "$TMP/site.log" 2>&1 & PIDS+=($!)
ADDR=$(attendi_regex "$TMP/site.log" '[a-z2-7]{56}\.nyct' 20)
attendi_regex "$TMP/site.log" 'online' 60 > /dev/null
timeout 120 "$BIN" browse "$ADDR" --directory "$DIR" --authority "$PUB" --path /f.bin > "$TMP/sito.bin" 2>"$TMP/browse.err"
# browse stampa la risposta HTTP intera: si confronta il corpo, dopo le intestazioni
python3 -c "import sys; d=open(sys.argv[1],'rb').read(); sys.stdout.buffer.write(d.split(b'\r\n\r\n',1)[-1])" "$TMP/sito.bin" > "$TMP/corpo.bin"
cmp -s "$TMP/corpo.bin" "$TMP/www/f.bin" && head -c 15 "$TMP/sito.bin" | grep -q "200" \
  && pass "sito .nyct pubblicato e visitato con la prima guardia spenta (stesso punto d'incontro, file identico)" || fail "sito: $(tail -n 2 "$TMP/browse.err" | tr '\n' ' ' | cut -c1-300)"

# 4) la prima torna: al prossimo avvio si rientra da lei
avvia_nodo "$P1"; attendi_regex "$TMP/n$P1.log" "127\.0\.0\.1:$P1," 20 > /dev/null; sleep 1
tunnel 4
E4=$(entrata "$TMP/c4.log")
cmp -s "$TMP/www/f.bin" "$TMP/giu.bin" && [ "$E4" = "$P1" ] && pass "la prima guardia è tornata: il client rientra da lei ($P1)" || fail "ritorno: entrata $E4 (attesa $P1)"

# 4-bis) la guardia si spegne A TUNNEL ACCESO: il tunnel si ricostruisce da solo, stessa porta SOCKS
kill "${PIDN[$P1]}"; wait "${PIDN[$P1]}" 2>/dev/null
for _ in $(seq 1 120); do grep -q "Tunnel rebuilt" "$TMP/c4.log" && break; sleep 0.5; done
rm -f "$TMP/giu.bin"; curl -s -m 40 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/f.bin -o "$TMP/giu.bin"
NUOVA=$(grep -oE "Tunnel rebuilt: entry 127\.0\.0\.1:[0-9]+" "$TMP/c4.log" | grep -oE "[0-9]+$" | tail -1)
cmp -s "$TMP/www/f.bin" "$TMP/giu.bin" && [ "$NUOVA" = "$P2" ] && grep -q "entry guard 127.0.0.1:$P1 lost" "$TMP/c4.log" \
  && pass "guardia spenta a tunnel acceso: ricostruito da solo dalla seconda ($P2), stessa porta SOCKS, scaricamento identico" \
  || fail "a tunnel acceso: nuova entrata «$NUOVA» (attesa $P2) · $(grep -E 'lost|rebuil' "$TMP/c4.log" | tail -2 | tr '\n' ' ')"
ferma

# 5) tutte e tre spente: nessun ripiego su altri nodi
for p in $P2 $P3; do kill "${PIDN[$p]}" 2>/dev/null; wait "${PIDN[$p]}" 2>/dev/null; done; sleep 1
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:$((B+10)) --control 127.0.0.1:$((B+11)) --hops 3 > "$TMP/c5.log" 2>&1 & CPID=$!; PIDS+=($CPID)
for _ in $(seq 1 120); do kill -0 $CPID 2>/dev/null || break; sleep 0.5; done
USATE=$(grep -oE "circuit 0: 127\.0\.0\.1:[0-9]+" "$TMP/c5.log" | grep -oE "[0-9]+$" | sort -u | tr '\n' ' ')
! kill -0 $CPID 2>/dev/null && ! grep -q "SOCKS5 proxy" "$TMP/c5.log" && [ "$(grep -c 'trying the next primary guard' "$TMP/c5.log")" -eq 2 ] \
  && ! echo "$USATE" | grep -qvE "^($P1|$P2|$P3| )+$" \
  && pass "tutte e 3 spente: provate solo le primarie ($USATE), poi il client si ferma invece di entrare da un nodo a caso" || fail "tutte spente: entrate «$USATE» · $(tail -2 "$TMP/c5.log")"
ferma
[ "$KO" -eq 0 ]
