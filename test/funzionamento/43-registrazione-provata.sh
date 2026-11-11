#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# REGISTRAZIONE PROVATA dei servizi `.nyct` (scaletta punto 5: «descrittore firmato»). Un servizio
# si registra al punto d'incontro con una PROVA che viene da chi ha la sua chiave privata (MAC con
# il segreto Diffie-Hellman fra servizio e nodo, legato al nodo, all'ora e a un numero casuale).
# Prima chiunque poteva registrarsi con la chiave di un altro e, riempiendo la coda di registrazioni
# false, renderlo irraggiungibile. Qui un IMPOSTORE ci prova in due modi e non ci riesce.
# Tutto in locale (127.0.0.1), con identità di prova.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · registrazione provata dei servizi .nyct"
assicura_binario
B=23220
K=$(python3 -c "import os;print(os.urandom(32).hex())")
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
righe=""
for i in 0 1 2 3; do
  p=$((B+i)); "$BIN" node --listen 127.0.0.1:$p --data "$TMP/n$p" --obfuscation "$K" > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
done
for i in 0 1 2 3; do
  p=$((B+i)); L=$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+")
  [ -z "$L" ] && { fail "nodo $p non partito"; exit 1; }
  righe="$righe${righe:+;}$L"
done
DIR="$TMP/dir.txt"
"$BIN" directory --authority-secret "$SEC" --entries "$righe" > "$DIR" 2>/dev/null
export NYCTALUS_GUARDIE="$TMP/guardie"
RETE=(--directory "$DIR" --authority "$PUB" --obfuscation "$K" --hops 3)
visita() { timeout 45 "$BIN" browse "$1" "${RETE[@]}" 2>/dev/null; }

# il servizio vero
mkdir -p "$TMP/vero" "$TMP/falso"; echo "sono il servizio VERO" > "$TMP/vero/index.html"; echo "sono l'IMPOSTORE" > "$TMP/falso/index.html"
ADDR=$("$BIN" address --identity "$TMP/id-vero" | grep -oE '[a-z2-7]{56}\.nyct' | tail -1)
"$BIN" site --dir "$TMP/vero" --identity "$TMP/id-vero" "${RETE[@]}" > "$TMP/vero.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/vero.log" "online" 40 > /dev/null || { fail "servizio vero non online: $(tail -3 "$TMP/vero.log")"; exit 1; }
visita "$ADDR" | grep -q "servizio VERO" && pass "il servizio vero si registra con la prova e si visita" || fail "servizio vero non raggiunto: $(tail -3 "$TMP/vero.log")"

# identità contraffatta: una chiave privata qualunque + la chiave PUBBLICA del servizio vero
python3 - "$TMP/id-vero" "$TMP/id-falso" <<'PY'
import os, sys
vero = open(sys.argv[1], "rb").read()
dati = vero if len(vero) == 64 else bytes.fromhex(vero.decode().strip())
falso = os.urandom(32) + dati[32:64]
open(sys.argv[2], "wb").write(falso if len(vero) == 64 else falso.hex().encode())
os.chmod(sys.argv[2], 0o600)
PY
[ "$("$BIN" address --identity "$TMP/id-falso" | grep -oE '[a-z2-7]{56}\.nyct' | tail -1)" = "$ADDR" ] \
  && pass "l'impostore si presenta con lo STESSO indirizzo .nyct (chiave pubblica copiata, privata diversa)" || fail "identità contraffatta non accettata dal programma"

# 1) impostore con una prova fatta con la chiave sbagliata: rifiutato dai punti d'incontro
"$BIN" site --dir "$TMP/falso" --identity "$TMP/id-falso" "${RETE[@]}" > "$TMP/falso1.log" 2>&1 & F1=$!; PIDS+=($F1)
sleep 8
grep -q "refused the registration" "$TMP/falso1.log" && ! grep -q "arriva un client" "$TMP/falso1.log" \
  && pass "impostore con prova falsa: i punti d'incontro rifiutano la registrazione" || fail "impostore 1: $(tail -3 "$TMP/falso1.log")"
kill $F1 2>/dev/null
# 2) impostore «alla vecchia maniera» (senza prova): rifiutato da TUTTI i punti d'incontro candidati
NYCTALUS_REGISTRA_VECCHIA=1 "$BIN" site --dir "$TMP/falso" --identity "$TMP/id-falso" "${RETE[@]}" > "$TMP/falso2.log" 2>&1 & F2=$!; PIDS+=($F2)
sleep 8
grep -q "refused the registration" "$TMP/falso2.log" \
  && pass "impostore senza prova: rifiutato anche dagli altri punti d'incontro (di serie i nodi vogliono la prova)" || fail "impostore 2: $(tail -3 "$TMP/falso2.log")"
# con gli impostori che insistono, chi visita trova SEMPRE il servizio vero
BUONE=0
for _ in 1 2 3 4 5 6; do R=$(visita "$ADDR"); echo "$R" | grep -q "servizio VERO" && BUONE=$((BUONE+1)); echo "$R" | grep -q "IMPOSTORE" && fail "risposta dell'impostore!"; done
[ "$BUONE" = 6 ] && pass "6 visite su 6 arrivano al servizio vero mentre l'impostore insiste" || fail "solo $BUONE visite su 6 riuscite"
kill $F2 2>/dev/null

# 3) di serie anche un servizio onesto di prima della 0.3.19 (senza prova) va aggiornato
ADDR2=$("$BIN" address --identity "$TMP/id-vecchio" | grep -oE '[a-z2-7]{56}\.nyct' | tail -1)
NYCTALUS_REGISTRA_VECCHIA=1 "$BIN" site --dir "$TMP/falso" --identity "$TMP/id-vecchio" "${RETE[@]}" > "$TMP/vecchio.log" 2>&1 & PIDS+=($!)
sleep 8
grep -q "refused the registration" "$TMP/vecchio.log" && pass "un servizio vecchio (senza prova) viene rifiutato: va aggiornato insieme ai nodi" || fail "servizio vecchio: $(tail -3 "$TMP/vecchio.log")"

# 4) nodi in modo compatibile (passaggio di versione): il servizio vecchio entra, l'impostore di un servizio che prova no
C=$((B+40)); righe2=""
for i in 0 1 2 3; do
  p=$((C+i)); NYCTALUS_REGISTRA_COMPATIBILE=1 "$BIN" node --listen 127.0.0.1:$p --data "$TMP/c$p" --obfuscation "$K" > "$TMP/c$p.log" 2>&1 & PIDS+=($!)
done
for i in 0 1 2 3; do p=$((C+i)); L=$(attendi_regex "$TMP/c$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+"); righe2="$righe2${righe2:+;}$L"; done
"$BIN" directory --authority-secret "$SEC" --entries "$righe2" > "$TMP/dir2.txt" 2>/dev/null
RETE=(--directory "$TMP/dir2.txt" --authority "$PUB" --obfuscation "$K" --hops 3)
echo "servizio vecchio" > "$TMP/falso/index.html"
NYCTALUS_REGISTRA_VECCHIA=1 "$BIN" site --dir "$TMP/falso" --identity "$TMP/id-vecchio" "${RETE[@]}" > "$TMP/vecchio2.log" 2>&1 & PIDS+=($!)
sleep 6
visita "$ADDR2" | grep -q "servizio vecchio" && pass "nodi in modo compatibile: il servizio vecchio resta raggiungibile durante il passaggio di versione" || fail "compatibile: $(tail -3 "$TMP/vecchio2.log")"
[ "$KO" -eq 0 ]
