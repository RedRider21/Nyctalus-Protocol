#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# NYCTALUS-COLLAUDO, lo strumento che controlla una rete vera: qui su una rete di prova locale
# CAMUFFATA (4 nodi con la chiave della rete, un servizio .nyct che fa da rubrica), senza
# toccare Internet (--solo-rete). Deve: leggere la configurazione, verificare l'elenco firmato,
# trovare tutti i nodi vivi, accorgersi che senza chiave i nodi restano muti, accendere un tunnel
# temporaneo e spegnerlo alla fine, raggiungere la rubrica dentro la rete, salvare il rapporto;
# poi accorgersi di un nodo spento e di una rete non configurata. HOME finta (mai i dati veri).
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · nyctalus-collaudo (collaudo di una rete)"
assicura_binario
COLLAUDO="$ROOT/pc/cli/nyctalus-collaudo"
H="$TMP/home"; mkdir -p "$H/.nyctalus-browser"
K=$(head -c 32 /dev/urandom | xxd -p -c 64)
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
righe=""; declare -A PIDNODO
for p in 23700 23701 23702 23703; do
  "$BIN" node --listen 127.0.0.1:$p --obfuscation "$K" > "$TMP/n$p.log" 2>&1 & PIDS+=($!); PIDNODO[$p]=$!
done
for p in 23700 23701 23702 23703; do
  L=$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+"); [ -z "$L" ] && { fail "nodo $p non partito"; exit 1; }
  righe="$righe${righe:+;}$L"
done
DIR="$TMP/dir.txt"; "$BIN" directory --authority-secret "$SEC" --entries "$righe" > "$DIR" 2>/dev/null
mkdir -p "$TMP/www"; echo "rubrica di prova" > "$TMP/www/index.html"
"$BIN" site --dir "$TMP/www" --identity "$TMP/rubrica.id" --directory "$DIR" --authority "$PUB" --obfuscation "$K" --hops 3 > "$TMP/site.log" 2>&1 & PIDS+=($!)
RUB=$(attendi_regex "$TMP/site.log" '[a-z2-7]{56}\.nyct' 20)
attendi_regex "$TMP/site.log" 'online' 40 > /dev/null
cat > "$H/.nyctalus-browser/config.json" <<EOF
{"directory_url": "$DIR", "authority": "$PUB", "nome_rete": "Rete di prova", "camuffamento": "$K", "rubrica": "$RUB"}
EOF

HOME="$H" NYCTALUS_BIN="$BIN" NO_COLOR=1 timeout 240 python3 "$COLLAUDO" --solo-rete --rapporto "$TMP/rapporto.txt" --json > "$TMP/c1.txt" 2>&1; E1=$?
g() { grep -q "$1" "$TMP/c1.txt"; }
g "✓ rete configurata: Rete di prova" && g "✓ forma del traffico: camuffamento" && pass "legge la configurazione della rete (nome, forma del traffico)" || fail "configurazione: $(head -8 "$TMP/c1.txt" | tr '\n' '|')"
g "✓ elenco firmato dall'autorità della rete: 4 nodi" && g "✓ tutti i 4 nodi rispondono" && pass "elenco firmato verificato, 4 nodi su 4 rispondono (con i tempi)" || fail "nodi: $(grep -A6 '2 · ' "$TMP/c1.txt" | tr '\n' '|')"
g "✓ a chi non ha la chiave della rete i nodi non rispondono (4 su 4 muti)" && pass "sonde senza chiave: i nodi restano muti" || fail "sonde: $(grep -i 'chiave' "$TMP/c1.txt")"
grep -qE "[✓!] tunnel temporaneo acceso" "$TMP/c1.txt" && pass "tunnel spento → ne accende uno temporaneo" || fail "tunnel: $(grep -A3 '3 · ' "$TMP/c1.txt" | tr '\n' '|')"
g "✓ rubrica della rete raggiungibile dentro la rete" && pass "il servizio .nyct della rubrica si raggiunge attraverso la rete" || fail "rubrica: $(grep -A6 '7 · ' "$TMP/c1.txt" | tr '\n' '|')"
g "– prove verso Internet" && pass "--solo-rete: nessuna prova verso Internet" || fail "solo-rete non rispettato"
[ "$E1" -eq 0 ] && g "COLLAUDO SUPERATO" && grep -q "Esito: .* 0 fallite" "$TMP/rapporto.txt" && tail -1 "$TMP/c1.txt" | python3 -c "import json,sys; json.loads(sys.stdin.read())" 2>/dev/null \
  && pass "esito finale «superato» (codice 0), rapporto salvato su file e in JSON" || fail "esito: codice $E1 · $(tail -4 "$TMP/c1.txt" | head -3 | tr '\n' '|')"
sleep 1; ! pgrep -f "nyctalus start --directory /tmp/nyctalus-collaudo-" > /dev/null && [ -z "$(ls -d /tmp/nyctalus-collaudo-* 2>/dev/null)" ] \
  && pass "alla fine il tunnel temporaneo è spento e non resta nulla in giro" || fail "resti del collaudo"

# un nodo spento: lo dice, ma con 3 la rete regge
kill "${PIDNODO[23703]}" 2>/dev/null; sleep 6      # il servizio si ri-registra sui nodi rimasti
# (senza la rubrica nella configurazione: in una rete di soli 4 nodi, con uno morto ancora in
# elenco, i circuiti verso un servizio interno non sempre si chiudono; qui si guarda la diagnosi dei nodi)
printf '{"directory_url": "%s", "authority": "%s", "camuffamento": "%s"}' "$DIR" "$PUB" "$K" > "$TMP/senza-rubrica.json"
HOME="$H" NYCTALUS_BIN="$BIN" NO_COLOR=1 timeout 240 python3 "$COLLAUDO" --solo-rete --config "$TMP/senza-rubrica.json" > "$TMP/c2.txt" 2>&1; E2=$?
# esito giusto in entrambi i casi: la rete regge (codice 0) oppure il nodo spento era proprio
# l'ENTRATA FISSA del client (guardia ricordata, come Tor) e il collaudo lo dice per nome
if [ "$E2" -eq 0 ] || grep -q "✗ il tunnel non parte: le tue ENTRATE FISSE (guardie primarie) sono spente" "$TMP/c2.txt"; then g2=1; else g2=0; fi
grep -q "! 3 nodi su 4 rispondono" "$TMP/c2.txt" && grep -q "127.0.0.1:23703" "$TMP/c2.txt" && [ "$g2" -eq 1 ] \
  && pass "un nodo spento: segnalato per nome (e, se era l'entrata fissa, spiegato)" || fail "nodo spento: codice $E2 · $(grep -A4 '2 · ' "$TMP/c2.txt" | tr '\n' '|')"
# due nodi spenti: non basta più
kill "${PIDNODO[23702]}" 2>/dev/null; sleep 1
HOME="$H" NYCTALUS_BIN="$BIN" NO_COLOR=1 timeout 120 python3 "$COLLAUDO" --solo-rete > "$TMP/c3.txt" 2>&1; E3=$?
grep -q "✗ solo 2 nodi su 4 rispondono" "$TMP/c3.txt" && grep -q "COLLAUDO NON SUPERATO" "$TMP/c3.txt" && [ "$E3" -eq 1 ] \
  && pass "due nodi spenti: collaudo NON superato (codice 1), con il motivo" || fail "due spenti: codice $E3 · $(tail -4 "$TMP/c3.txt" | tr '\n' '|')"
# elenco manomesso: la firma non torna
sed 's/23700/23709/' "$DIR" > "$TMP/falso.txt"
printf '{"directory_url": "%s", "authority": "%s"}' "$TMP/falso.txt" "$PUB" > "$TMP/falso.json"
HOME="$H" NYCTALUS_BIN="$BIN" NO_COLOR=1 timeout 60 python3 "$COLLAUDO" --solo-rete --config "$TMP/falso.json" > "$TMP/c4.txt" 2>&1; E4=$?
grep -q "✗ elenco dei nodi non valido" "$TMP/c4.txt" && [ "$E4" -eq 1 ] && pass "elenco dei nodi manomesso: rifiutato" || fail "elenco manomesso: codice $E4 · $(grep '2 · ' -A3 "$TMP/c4.txt" | tr '\n' '|')"
# nessuna rete configurata
mkdir -p "$TMP/vuota"
HOME="$TMP/vuota" NYCTALUS_BIN="$BIN" NO_COLOR=1 timeout 30 python3 "$COLLAUDO" --solo-rete > "$TMP/c5.txt" 2>&1; E5=$?
grep -q "✗ nessuna rete pubblica configurata" "$TMP/c5.txt" && [ "$E5" -eq 1 ] && pass "senza rete configurata: lo dice e spiega cosa fare" || fail "senza rete: codice $E5"
[ "$KO" -eq 0 ]
