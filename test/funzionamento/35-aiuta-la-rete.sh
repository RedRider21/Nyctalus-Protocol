#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# «🦇 AIUTA LA RETE»: il PC di un utente fa anche da nodo SOLO INTERMEDIO (--middle-only, con
# ruolo=intermedio nell'elenco firmato). Regola inviolabile: il traffico degli altri non esce MAI
# dal suo indirizzo. Qui: il nodo rifiuta di fare da uscita anche se un client lo forza; i client
# normali non lo scelgono mai come guardia o uscita; la rete funziona usandolo come intermedio.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · Aiuta la rete (nodi solo intermedi)"
assicura_binario
B=22700
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
righe=""
for p in $B $((B+1)) $((B+2)) $((B+3)) $((B+4)) $((B+5)); do
  extra=(); [ $p -ge $((B+3)) ] && extra=(--middle-only --max-rate 50)
  "$BIN" node --listen 127.0.0.1:$p ${extra[@]+"${extra[@]}"} > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
done
for p in $B $((B+1)) $((B+2)) $((B+3)) $((B+4)) $((B+5)); do
  L=$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+")
  [ -z "$L" ] && { fail "nodo $p non partito"; exit 1; }
  if [ $p -ge $((B+3)) ]; then righe="$righe${righe:+;}$L,utente$p,,middle"; else righe="$righe${righe:+;}$L"; fi
  [ $p = $((B+3)) ] && INTERMEDIO="$L"; [ $p = $B ] && PIENO="$L"
done
grep -q "MIDDLE ONLY" "$TMP/n$((B+3)).log" && grep -q "bandwidth cap: 50" "$TMP/n$((B+3)).log" \
  && pass "il PC parte come nodo solo intermedio, con tetto di banda" || fail "avvio solo intermedio: $(tail -3 "$TMP/n$((B+3)).log")"
DIR="$TMP/dir.txt"
"$BIN" directory --authority-secret "$SEC" --entries "$righe" > "$DIR" 2>/dev/null
[ "$(grep -c 'ruolo=intermedio' "$DIR")" = 3 ] && pass "nell'elenco firmato 3 nodi «ruolo=intermedio»" || fail "elenco: $(cat "$DIR")"

# 1) un client «malevolo» forza il nodo solo intermedio come uscita: rifiuta
nodo() { echo "${1%%,*},$(echo "$1" | cut -d, -f2)"; }
timeout 20 "$BIN" test-circuit --nodes "$(nodo "$PIENO");$(nodo "$INTERMEDIO")" > "$TMP/forzato.log" 2>&1
sleep 1
grep -q "refusing to act as exit" "$TMP/n$((B+3)).log" && pass "forzato come uscita, il nodo solo intermedio RIFIUTA (il traffico non esce dal suo indirizzo)" \
  || fail "nessun rifiuto: $(tail -3 "$TMP/n$((B+3)).log")"

# 2) i client normali non lo scelgono mai come guardia o uscita (8 avvii con guardie nuove)
mai=1
for i in $(seq 1 8); do
  NYCTALUS_GUARDIE="$TMP/g$i" timeout 6 "$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:$((B+30+i)) --control 127.0.0.1:$((B+50+i)) --hops 3 --circuits 2 > "$TMP/c$i.log" 2>&1
  while read -r ent usc; do
    for p in $((B+3)) $((B+4)) $((B+5)); do [ "$ent" = "127.0.0.1:$p" ] || [ "$usc" = "127.0.0.1:$p" ] && mai=0; done
  done < <(grep -oE "circuit [0-9]+: [0-9.:]+ → [0-9.:]+ → [0-9.:]+" "$TMP/c$i.log" | awk '{print $3, $7}')
done
N=$(cat "$TMP"/c*.log | grep -cE "circuit [0-9]+:")
[ "$mai" = 1 ] && [ "$N" -ge 16 ] && pass "in $N circuiti un nodo solo intermedio non è mai guardia né uscita" || fail "intermedio come guardia/uscita (circuiti $N)"
grep -qE "→ 127\.0\.0\.1:($((B+3))|$((B+4))|$((B+5))) →" "$TMP"/c*.log && pass "i nodi degli utenti vengono usati come intermedi" || fail "intermedi mai usati"

# 3) la rete funziona
mkdir -p "$TMP/www"; echo "ciao dal sito" > "$TMP/www/index.html"
python3 -m http.server $((B+20)) --bind 127.0.0.1 --directory "$TMP/www" > /dev/null 2>&1 & PIDS+=($!)
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:$((B+10)) --control 127.0.0.1:$((B+11)) --hops 3 > "$TMP/c.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/c.log" "SOCKS5 proxy" 20 > /dev/null
R=$(curl -s -m 20 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R" = "ciao dal sito" ] && pass "si naviga con i nodi degli utenti come intermedi" || fail "navigazione: $R"

# 4) PC dell'utente e gestore (HOME finte: mai i dati veri)
G="$ROOT/pc/gestore-rete/nyctalus-rete"; HU="$TMP/utente"; HG="$TMP/gestore"; mkdir -p "$HU" "$HG"
COD=$(python3 -c "import base64,json,sys;print('NYCT1:'+base64.urlsafe_b64encode(json.dumps({'nome':'prova','directory_url':'http://127.0.0.1:9/d.txt','authority':sys.argv[1]}).encode()).decode().rstrip('='))" "$PUB")
HOME="$HU" NYCTALUS_BIN="$BIN" python3 "$G" importa "$COD" > /dev/null 2>&1
HOME="$HU" NYCTALUS_BIN="$BIN" python3 "$G" aiuta si --indirizzo miacasa.ddns.net --porta 4800 --tetto 8 --stampa > "$TMP/unita.txt" 2>&1
grep -q -- "--middle-only --max-rate 8" "$TMP/unita.txt" && grep -q -- "--announce miacasa.ddns.net:4800" "$TMP/unita.txt" \
  && pass "PC: «aiuta si» prepara il servizio con il nodo SOLO intermedio e il tetto di banda" || fail "servizio aiuta: $(cat "$TMP/unita.txt")"
RIGA=$(cat "$HU/.nyctalus-rete/aiuto/riga-elenco.txt" 2>/dev/null)
[[ "$RIGA" == miacasa.ddns.net:4800,* ]] && pass "PC: identità del nodo creata (riga da dare al gestore)" || fail "riga: $RIGA"
HOME="$HU" python3 "$G" aiuta si --indirizzo "x y" 2>&1 | grep -q "indirizzo" && pass "indirizzo non valido rifiutato" || fail "indirizzo non valido accettato"
HOME="$HG" NYCTALUS_BIN="$BIN" python3 "$G" crea --nome g > /dev/null 2>&1
HOME="$HG" NYCTALUS_BIN="$BIN" python3 "$G" aggiungi "$RIGA" --intermedio --banda 8 --famiglia anna > "$TMP/agg.log" 2>&1
grep -q "^miacasa.ddns.net:4800,[0-9a-f]*,[0-9a-f]*,anna,8,middle$" "$HG/.nyctalus-rete/nodi.txt" \
  && pass "gestore: «aggiungi --intermedio --banda 8 --famiglia anna»" || fail "nodi.txt: $(cat "$HG/.nyctalus-rete/nodi.txt"; cat "$TMP/agg.log")"
HOME="$HG" NYCTALUS_BIN="$BIN" python3 "$G" famiglia miacasa.ddns.net:4800 bea > /dev/null 2>&1
grep -q ",bea,8,middle$" "$HG/.nyctalus-rete/nodi.txt" && pass "cambiare la famiglia non toglie banda e ruolo" || fail "ruolo perso: $(cat "$HG/.nyctalus-rete/nodi.txt")"
[ "$KO" -eq 0 ]
