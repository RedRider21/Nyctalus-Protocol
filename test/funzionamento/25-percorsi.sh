#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Percorsi (PIANO-ANONIMATO, Tappa 2), con il motore vero e un elenco firmato con famiglie:
# la guardia è sempre la stessa (ricordata su file privato), l'uscita cambia, in nessun percorso
# compaiono due nodi della stessa famiglia; famiglia nell'elenco protetta dalla firma; regola
# impossibile → avviso, oppure rifiuto con NYCTALUS_PARENTELA=rigida. Tutto in 127.0.0.1.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · scelta dei percorsi (guardia, uscita a caso, regola di parentela)"
assicura_binario
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
chiave() { printf '%064x' "$1"; }
# 6 nodi finti (non servono accesi: il motore stampa i percorsi prima di collegarsi);
# famiglie: «mario» = nodi 1 e 2, «luca» = nodi 3 e 4, il 5 e il 6 senza famiglia
VOCI="127.0.0.1:22001,$(chiave 1),$(chiave 101),mario;127.0.0.1:22002,$(chiave 2),$(chiave 102),mario;127.0.0.1:22003,$(chiave 3),$(chiave 103),luca;127.0.0.1:22004,$(chiave 4),$(chiave 104),luca;127.0.0.1:22005,$(chiave 5),$(chiave 105);127.0.0.1:22006,$(chiave 6),$(chiave 106)"
"$BIN" directory --authority-secret "$SEC" --entries "$VOCI" > "$TMP/dir.txt" 2>"$TMP/dir.err"
grep -c "famiglia=mario" "$TMP/dir.txt" | grep -q '^2$' && grep -q "famiglia=luca" "$TMP/dir.txt" \
  && pass "elenco firmato con le famiglie dichiarate dal gestore" || fail "elenco: $(cat "$TMP/dir.err")"

percorsi() {   # avvia il demone, raccoglie i percorsi stampati, lo ferma
  timeout 3 "$BIN" circuit --directory "$1" --authority "$PUB" --socks 127.0.0.1:0 --control 127.0.0.1:0 \
    --circuits 3 --hops 3 2>&1 | grep -E "circuit [0-9]+:|WARNING|related|Error|error" || true
}
: > "$TMP/tutti.txt"
for i in $(seq 1 25); do percorsi "$TMP/dir.txt" >> "$TMP/tutti.txt"; done
nome() { case "$1" in *22001) echo m1;; *22002) echo m2;; *22003) echo l3;; *22004) echo l4;; *22005) echo n5;; *22006) echo n6;; *) echo "?";; esac; }
GUARDIE=$(grep -oE "circuit [0-9]+: [0-9.:]+" "$TMP/tutti.txt" | awk '{print $3}' | sort -u | wc -l)
USCITE=$(grep -oE "→ [0-9.:]+$" "$TMP/tutti.txt" | awk '{print $2}' | sort -u | wc -l)
N=$(grep -c "circuit [0-9]*:" "$TMP/tutti.txt")
[ "$N" -eq 75 ] && pass "25 avvii × 3 circuiti: $N percorsi" || fail "percorsi stampati: $N ($(head -3 "$TMP/tutti.txt"))"
[ "$GUARDIE" -eq 1 ] && pass "la guardia (entrata) è sempre la stessa, anche fra un avvio e l'altro" || fail "guardie diverse: $GUARDIE"
[ "$USCITE" -ge 3 ] && pass "l'uscita è scelta a caso ($USCITE nodi diversi in 25 avvii; prima era sempre il 2° dell'elenco)" || fail "uscite: $USCITE"
PARENTI=0
while read -r riga; do
  fam=$(echo "$riga" | grep -oE "127\.0\.0\.1:2200[0-9]" | while read -r a; do nome "$a"; done | cut -c1 | grep -v n | sort | uniq -d)
  [ -n "$fam" ] && PARENTI=$((PARENTI+1)) && echo "parenti: $riga" >> "$TMP/parenti.txt"
done < <(grep "circuit [0-9]*:" "$TMP/tutti.txt")
[ "$PARENTI" -eq 0 ] && pass "in nessun percorso due nodi della stessa famiglia" || fail "$PARENTI percorsi con parenti: $(head -2 "$TMP/parenti.txt")"
! grep -q WARNING "$TMP/tutti.txt" && pass "regola possibile: nessun avviso" || fail "avviso inatteso"
F=$(ls "$NYCTALUS_GUARDIE"/* 2>/dev/null | head -1)
[ -n "$F" ] && [ "$(stat -c %a "$F")" = "600" ] && [ "$(stat -c %a "$NYCTALUS_GUARDIE")" = "700" ] \
  && pass "guardia ricordata in un file privato (600, cartella 700), una per rete" || fail "file della guardia: $(ls -la "$NYCTALUS_GUARDIE" 2>&1)"

# la famiglia è coperta dalla firma: toglierla rende l'elenco invalido
sed 's/ famiglia=mario//' "$TMP/dir.txt" > "$TMP/manomesso.txt"
percorsi "$TMP/manomesso.txt" | grep -qiE "firma|signature|error" && ! percorsi "$TMP/manomesso.txt" | grep -q "circuit 0:" \
  && pass "togliere una famiglia dall'elenco ne rompe la firma: rifiutato" || fail "elenco manomesso accettato"

# tutti nella stessa famiglia (come i nodi tutti su un PC): avviso, oppure rifiuto se rigida
TUTTI="127.0.0.1:22011,$(chiave 11),$(chiave 111),casa;127.0.0.1:22012,$(chiave 12),$(chiave 112),casa;127.0.0.1:22013,$(chiave 13),$(chiave 113),casa"
"$BIN" directory --authority-secret "$SEC" --entries "$TUTTI" > "$TMP/casa.txt" 2>/dev/null
OUT=$(percorsi "$TMP/casa.txt")
echo "$OUT" | grep -q "WARNING.*NOT anonymous" && echo "$OUT" | grep -q "circuit 0:" \
  && pass "regola impossibile (tutti dello stesso gestore): funziona ma AVVISA «cifrato ma NON anonimo»" || fail "avviso mancante: $OUT"
OUT=$(NYCTALUS_PARENTELA=rigida percorsi "$TMP/casa.txt")
! echo "$OUT" | grep -q "circuit 0:" && echo "$OUT" | grep -q "related nodes" \
  && pass "NYCTALUS_PARENTELA=rigida: nessun percorso con parenti, il motore si rifiuta" || fail "rigida: $OUT"
[ ! -e "$HOME/.nyctalus/guardie" ] || [ -z "$(find "$HOME/.nyctalus/guardie" -newer "$TMP/auth.txt" 2>/dev/null)" ] \
  && pass "nessun file scritto nella cartella personale (~/.nyctalus)" || fail "scritto in ~/.nyctalus/guardie"
[ "$KO" -eq 0 ]
