#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# CAPACITÀ (scaletta punto 3): i PC che «aiutano la rete» (nodi solo intermedi) diventano
# GUARDIE (nodi d'entrata) quando il gestore li vede STABILI nel tempo, come il contrassegno
# «Guard» di Tor; mai uscite né punti d'incontro. Il gestore prova chi risponde
# («stabilita --sonda»), promuove i più stabili e retrocede chi sparisce («--applica»), rifirmando
# l'elenco (`ruolo=intermedio guardia=si`, compatibile con i motori vecchi). Tutto in locale, in
# una HOME finta (mai i dati veri). Soglie accorciate per la prova: 0 giorni, 3 prove.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · guardie promosse (PC stabili che aiutano la rete)"
assicura_binario
B=22820
G="$ROOT/pc/gestore-rete/nyctalus-rete"; HG="$TMP/gestore"; mkdir -p "$HG"
export NYCTALUS_STAB_GIORNI=0 NYCTALUS_STAB_PROVE=3 NYCTALUS_STAB_OGNI=0
gestore() { HOME="$HG" NYCTALUS_BIN="$BIN" python3 "$G" "$@"; }
gestore crea --nome prova > "$TMP/crea.log" 2>&1 || { fail "crea: $(tail -2 "$TMP/crea.log")"; exit 1; }
K=$(python3 -c "import json,sys;print(json.load(open(sys.argv[1]))['camuffamento'])" "$HG/.nyctalus-rete/protezione.json")

# 3 nodi pieni (banda 1) e 2 PC che aiutano (solo intermedi; il primo con molta banda)
for i in 0 1 2; do
  p=$((B+i)); "$BIN" node --listen 127.0.0.1:$p --data "$TMP/n$p" --obfuscation "$K" > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
done
for i in 3 4; do
  p=$((B+i)); "$BIN" node --listen 127.0.0.1:$p --data "$TMP/n$p" --obfuscation "$K" --middle-only > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
  [ "$i" = 4 ] && AIUTO2=$!
done
riga() { attendi_regex "$TMP/n$1.log" "127\.0\.0\.1:$1,[0-9a-f]+,[0-9a-f]+"; }
PIENE=(); for i in 0 1 2; do PIENE+=("$(riga $((B+i)))"); done
H1=$(riga $((B+3))); H2=$(riga $((B+4)))
[ -n "$H1" ] && [ -n "$H2" ] && [ -n "${PIENE[2]}" ] || { fail "nodi non partiti"; exit 1; }
gestore aggiungi "${PIENE[@]}" --banda 1 > /dev/null 2>&1
gestore aggiungi "$H1" --intermedio --banda 1000 > /dev/null 2>&1
gestore aggiungi "$H2" --intermedio --banda 1 > /dev/null 2>&1
gestore firma > /dev/null 2>&1
DIR="$HG/.nyctalus-rete/directory.txt"
grep -q "ruolo=intermedio" "$DIR" && ! grep -q "guardia=si" "$DIR" && pass "all'inizio i due PC che aiutano sono solo intermedi" || fail "elenco iniziale: $(grep '^v' "$DIR")"

# 1) tre prove: rispondono tutti → con --applica entrambi promossi a guardia
for _ in 1 2 3; do gestore stabilita --sonda > "$TMP/stab.log" 2>&1; done
grep -q "2 rispondono" "$TMP/stab.log" && pass "il gestore prova i PC che aiutano: rispondono tutti e due" || fail "sonda: $(cat "$TMP/stab.log")"
grep -q "da PROMUOVERE" "$TMP/stab.log" && pass "dopo 3 prove al 100% propone la promozione" || fail "proposta: $(cat "$TMP/stab.log")"
gestore stabilita --applica > "$TMP/app.log" 2>&1
[ "$(grep -c 'guardia=si' "$DIR")" = 2 ] && pass "--applica: due guardie promosse e l'elenco è rifirmato (guardia=si)" || fail "applica: $(cat "$TMP/app.log"); $(grep '^v' "$DIR")"
gestore elenco | grep -q "🛡 GUARDIA" && pass "«elenco» mostra le guardie promosse" || fail "elenco senza guardie"

# 2) il client entra da una guardia promossa (pesata per banda: H1) ed esce da un nodo pieno
AUT=$(python3 -c "import json,sys;print(json.load(open(sys.argv[1]))['pubblica'])" "$HG/.nyctalus-rete/autorita.json")
mkdir -p "$TMP/www"; echo "ciao dal sito" > "$TMP/www/index.html"
python3 -m http.server $((B+20)) --bind 127.0.0.1 --directory "$TMP/www" > /dev/null 2>&1 & PIDS+=($!)
export NYCTALUS_GUARDIE="$TMP/guardie"
"$BIN" circuit --directory "$DIR" --authority "$AUT" --obfuscation "$K" --socks 127.0.0.1:$((B+10)) --control 127.0.0.1:$((B+11)) --hops 3 > "$TMP/c.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/c.log" "SOCKS5 proxy" 25 > /dev/null
R=$(curl -s -m 20 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/ 2>&1)
SCELTA=$(cat "$TMP/guardie"/* 2>/dev/null | awk '{print $1}' | head -1)
X1=$(echo "$H1" | cut -d, -f2)
[ "$R" = "ciao dal sito" ] && [ "$SCELTA" = "$X1" ] && pass "il client entra dalla guardia promossa (un PC che aiuta) e naviga: uscita da un nodo pieno" \
  || fail "client: risposta «$R», guardia $SCELTA (attesa $X1) · $(tail -3 "$TMP/c.log")"
grep -q "refus\|rifiut" "$TMP/n$((B+3)).log" && fail "la guardia promossa ha dovuto rifiutare un passo d'uscita" || pass "la guardia promossa non fa mai da uscita (nessun rifiuto nel suo registro)"

# 3) il secondo PC sparisce: dopo 3 prove a vuoto torna solo intermedio
kill "$AIUTO2" 2>/dev/null; wait "$AIUTO2" 2>/dev/null
for _ in 1 2 3; do gestore stabilita --sonda > "$TMP/stab2.log" 2>&1; done
grep -q "da RIPORTARE" "$TMP/stab2.log" && pass "un PC che sparisce: proposto il ritorno a intermedio" || fail "retrocessione: $(cat "$TMP/stab2.log")"
gestore stabilita --applica > /dev/null 2>&1
[ "$(grep -c 'guardia=si' "$DIR")" = 1 ] && pass "--applica: resta guardia solo il PC stabile" || fail "dopo la retrocessione: $(grep '^v' "$DIR")"

# 4) scelta a mano del gestore
gestore guardia "$(echo "$H1" | cut -d, -f1)" no > /dev/null 2>&1
[ "$(grep -c 'guardia=si' "$DIR")" = 0 ] && pass "«guardia IP no»: il gestore toglie la promozione a mano (e rifirma)" || fail "a mano: $(grep '^v' "$DIR")"
gestore guardia "$(echo "$H1" | cut -d, -f1)" auto > /dev/null 2>&1
[ "$(grep -c 'guardia=si' "$DIR")" = 1 ] && pass "«guardia IP auto»: torna a decidere la stabilità misurata" || fail "auto: $(grep '^v' "$DIR")"

# 5) compatibilità: la riga promossa resta un intermedio per chi non conosce «guardia=si»
grep -E "^v 127\.0\.0\.1:$((B+3)) " "$DIR" | grep -q " ruolo=intermedio guardia=si" \
  && pass "nell'elenco: «ruolo=intermedio guardia=si» (i motori vecchi lo usano solo come intermedio)" || fail "formato: $(grep "$((B+3))" "$DIR")"
[ "$KO" -eq 0 ]
