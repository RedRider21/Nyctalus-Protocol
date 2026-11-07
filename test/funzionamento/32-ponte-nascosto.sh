#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# PONTI NASCOSTI (PIANO-ANONIMATO, invisibilità punto 1) e RESISTENZA ALLE SONDE.
# Un ponte è un nodo che NON sta nell'elenco pubblico e ha una sua chiave di camuffamento: chi lo
# usa entra sempre da lì; chi osserva vede un indirizzo qualunque; chi lo sonda senza la chiave
# non riceve nessuna risposta. Lo stesso vale ora per ogni nodo camuffato.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · ponti nascosti e resistenza alle sonde"
assicura_binario
B=22400
K=$(python3 -c "import os;print(os.urandom(32).hex())")
"$BIN" authority > "$TMP/auth.txt" 2>&1
SEC=$(grep -oE 'secret:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
PUB=$(grep -oE 'public:[[:space:]]+[0-9a-f]+' "$TMP/auth.txt" | awk '{print $2}')
righe=""
for p in $B $((B+1)) $((B+2)); do
  "$BIN" node --listen 127.0.0.1:$p --obfuscation "$K" > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
done
for p in $B $((B+1)) $((B+2)); do
  L=$(attendi_regex "$TMP/n$p.log" "127\.0\.0\.1:$p,[0-9a-f]+,[0-9a-f]+")
  [ -z "$L" ] && { fail "nodo $p non partito"; exit 1; }
  righe="$righe${righe:+;}$L"
done
DIR="$TMP/dir.txt"
"$BIN" directory --authority-secret "$SEC" --entries "$righe" > "$DIR" 2>/dev/null
# il ponte: fuori dall'elenco, con la sua chiave; verso gli altri nodi usa quella della rete
mkdir -p "$TMP/ponte"
"$BIN" node --listen 127.0.0.1:$((B+5)) --data "$TMP/ponte" --obfuscation "$K" --directory "$DIR" --authority "$PUB" \
    --hidden-bridge > "$TMP/ponte.log" 2>&1 & PIDS+=($!)
PONTE=$(attendi_regex "$TMP/ponte.log" "127\.0\.0\.1:$((B+5)),[0-9a-f]{64},[0-9a-f]{64},[0-9a-f]{64}")
[ -n "$PONTE" ] && pass "il nodo-ponte stampa la sua riga (indirizzo, chiave, impronta, chiave del ponte)" \
  || { fail "riga del ponte assente: $(tail -3 "$TMP/ponte.log")"; exit 1; }
verifica "la riga del ponte è salvata solo per il proprietario (600) e NON c'è una riga per l'elenco" \
  bash -c "[ \"\$(stat -c %a '$TMP/ponte/riga-ponte.txt')\" = 600 ] && [ ! -e '$TMP/ponte/riga-elenco.txt' ] && [ \"\$(stat -c %a '$TMP/ponte/chiave-ponte')\" = 600 ]"
grep -q ":$((B+5))" "$DIR" && fail "il ponte è finito nell'elenco pubblico" || pass "il ponte non è nell'elenco pubblico"

# un sito qualunque
mkdir -p "$TMP/www"; echo "ciao dal sito" > "$TMP/www/index.html"
python3 -m http.server $((B+20)) --bind 127.0.0.1 --directory "$TMP/www" > /dev/null 2>&1 & PIDS+=($!)

# 1) navigare entrando dal ponte
"$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$K" --bridge "$PONTE" \
    --socks 127.0.0.1:$((B+10)) --control 127.0.0.1:$((B+11)) --hops 3 > "$TMP/c.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/c.log" "SOCKS5 proxy" 20 > /dev/null
R=$(curl -s -m 20 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R" = "ciao dal sito" ] && pass "si naviga attraverso la rete entrando dal ponte nascosto" || fail "navigazione dal ponte: $R · $(tail -3 "$TMP/c.log")"
N=$(grep -cE "circuit [0-9]+: 127\.0\.0\.1:$((B+5)) →" "$TMP/c.log")
[ "$N" -ge 2 ] && grep -q "hidden bridge" "$TMP/c.log" && pass "tutti i circuiti ($N) entrano dal ponte, nessuno da un nodo dell'elenco" \
  || fail "circuiti dal ponte: $N · $(grep circuit "$TMP/c.log")"

# 2) con la chiave del ponte sbagliata non si entra
SBAGLIATA="${PONTE%,*},$K"
"$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$K" --bridge "$SBAGLIATA" \
    --socks 127.0.0.1:$((B+12)) --control 127.0.0.1:$((B+13)) --hops 3 > "$TMP/c2.log" 2>&1 & PIDS+=($!)
sleep 12
R=$(curl -s -m 5 --socks5-hostname 127.0.0.1:$((B+12)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R" != "ciao dal sito" ] && pass "con la chiave della rete ma non quella del ponte il ponte resta muto: non si entra" \
  || fail "il ponte ha accettato una chiave sbagliata"

# 3) sito .nyct raggiunto da entrambi i lati attraverso il ponte
ADDR=$("$BIN" address --identity "$TMP/servizio" | grep -oE '[a-z2-7]{56}\.nyct' | tail -1)
"$BIN" site --dir "$TMP/www" --identity "$TMP/servizio" --directory "$DIR" --authority "$PUB" --obfuscation "$K" \
    --bridge "$PONTE" --hops 3 > "$TMP/s.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/s.log" "online" 30 > /dev/null
R=$(timeout 40 "$BIN" browse "$ADDR" --directory "$DIR" --authority "$PUB" --obfuscation "$K" --bridge "$PONTE" --hops 3 2>/dev/null)
echo "$R" | grep -q "ciao dal sito" && pass "sito .nyct pubblicato e visitato entrando dal ponte" || fail "sito .nyct via ponte: $(echo "$R" | tail -2) · $(tail -2 "$TMP/s.log")"

# 4) le sonde: un nodo senza camuffamento risponde (la sonda funziona), quelli camuffati e il ponte no
"$BIN" node --listen 127.0.0.1:$((B+6)) > "$TMP/scoperto.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/scoperto.log" "directory entry" 10 > /dev/null
S=$(python3 "$STRUMENTI/sonda_quic.py" $((B+6)) 40 | awk '{print $2}')
[ "${S:-0}" -gt 0 ] && pass "controllo: un nodo SENZA camuffamento risponde alle sonde ($S risposte su 40): si scopre" \
  || fail "la sonda non ottiene risposta nemmeno dal nodo scoperto (sonda guasta?)"
for p in $((B+5)) $B; do
  S=$(python3 "$STRUMENTI/sonda_quic.py" $p 200 | awk '{print $2}')
  cosa=$([ $p = $((B+5)) ] && echo "il ponte nascosto" || echo "un nodo camuffato dell'elenco")
  [ "$S" = 0 ] && pass "$cosa non risponde a 200 sonde: da fuori sembra un indirizzo spento" || fail "$cosa ha risposto $S volte alle sonde"
done
R=$(curl -s -m 20 --socks5-hostname 127.0.0.1:$((B+10)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R" = "ciao dal sito" ] && pass "dopo le sonde il ponte serve ancora i suoi utenti" || fail "dopo le sonde: $R"

# 5) PC: «nyctalus-rete ponte» (HOME finta: mai i dati veri)
G="$ROOT/pc/gestore-rete/nyctalus-rete"; HC="$TMP/cliente"; mkdir -p "$HC"
COD=$(python3 -c "import base64,json,sys;print('NYCT1:'+base64.urlsafe_b64encode(json.dumps({'nome':'prova','directory_url':'http://127.0.0.1:9/directory.txt','authority':sys.argv[1],'camuffamento':sys.argv[2],'rumore':0.2}).encode()).decode().rstrip('='))" "$PUB" "$K")
HOME="$HC" python3 "$G" importa "$COD" > /dev/null 2>&1
HOME="$HC" python3 "$G" ponte aggiungi "$PONTE" > "$TMP/g.log" 2>&1
CC="$HC/.nyctalus-browser/config.json"
python3 -c "import json,sys;c=json.load(open(sys.argv[1]));sys.exit(0 if c['ponti'][sys.argv[2]]==[sys.argv[3]] else 1)" "$CC" "$PUB" "$PONTE" \
  && [ "$(stat -c %a "$CC")" = 600 ] && pass "PC: «nyctalus-rete ponte aggiungi» salva il ponte per QUESTA rete, in un file privato (600)" || fail "PC ponte aggiungi: $(cat "$TMP/g.log")"
HOME="$HC" python3 "$G" ponte aggiungi "127.0.0.1:1,abc" > /dev/null 2>&1 && fail "accettata una riga di ponte rovinata" || pass "PC: una riga di ponte rovinata viene rifiutata"
HOME="$HC" python3 "$G" ponte elenco | grep -q "127.0.0.1:$((B+5))" && ! HOME="$HC" python3 "$G" ponte elenco | grep -q "${PONTE##*,}" \
  && pass "PC: «ponte elenco» mostra l'indirizzo ma non la chiave del ponte" || fail "PC ponte elenco"
# la chat del PC passa i ponti al motore per AMBIENTE, non sulla riga di comando
grep -q 'export NYCTALUS_PONTI=' "$ROOT/strumenti/avvia-chat.sh" && grep -q 'export NYCTALUS_PONTI=' "$ROOT/strumenti/avvia-firefox.sh" \
  && pass "chat e Nyctalus Firefox danno i ponti al motore per variabile d'ambiente (non visibile con «ps»)" || fail "avvii senza ponti"
# il motore li prende dall'ambiente
NYCTALUS_PONTI="$PONTE" "$BIN" circuit --directory "$DIR" --authority "$PUB" --obfuscation "$K" \
    --socks 127.0.0.1:$((B+14)) --control 127.0.0.1:$((B+15)) --hops 3 > "$TMP/c3.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/c3.log" "SOCKS5 proxy" 20 > /dev/null
R=$(curl -s -m 20 --socks5-hostname 127.0.0.1:$((B+14)) http://127.0.0.1:$((B+20))/ 2>&1)
[ "$R" = "ciao dal sito" ] && grep -q "hidden bridge" "$TMP/c3.log" && pass "motore: ponti da NYCTALUS_PONTI, si naviga entrando dal ponte" || fail "ponti da ambiente: $R"
HOME="$HC" python3 "$G" ponte togli tutti > /dev/null 2>&1
python3 -c "import json,sys;c=json.load(open(sys.argv[1]));sys.exit(0 if not c.get('ponti') else 1)" "$CC" && pass "PC: «ponte togli tutti» li toglie" || fail "PC ponte togli"

# 6) telefono/Windows: l'API della rete (orchestratore con HOME finta, senza avvio dei servizi)
HM="$TMP/mobile"; mkdir -p "$HM/.nyctalus-mobile"; TOK=$(python3 -c 'import secrets;print(secrets.token_hex(16))')
printf %s "$TOK" > "$HM/.nyctalus-mobile/token"; echo '{"porte": {"http": 19743}}' > "$HM/.nyctalus-mobile/config.json"
HOME="$HM" NYCTALUS_AVVIO_AUTOMATICO=0 NYCTALUS_BIN=/bin/false python3 "$ROOT/mobile/orchestratore/nyctalus_mobile.py" > "$TMP/o.log" 2>&1 & PIDS+=($!)
api() { python3 - "$TOK" "$@" <<'PY'
import json, sys, urllib.request, urllib.error
tok, metodo, via = sys.argv[1:4]
corpo = sys.argv[4].encode() if len(sys.argv) > 4 else None
r = urllib.request.Request("http://127.0.0.1:19743" + via, data=corpo, method=metodo,
    headers={"Cookie": "nyct=" + tok, "Origin": "http://127.0.0.1:19743", "Content-Type": "application/json"})
try: print(urllib.request.urlopen(r, timeout=5).read().decode())
except urllib.error.HTTPError as e: print(e.read().decode())
except Exception as e: print("ERR", e)
PY
}
for _ in $(seq 1 50); do api GET /api/rete | grep -q condivisa && break; sleep 0.2; done
api POST /api/rete "$(python3 -c "import json,sys;print(json.dumps({'codice':sys.argv[1]}))" "$COD")" > /dev/null
api POST /api/rete "$(python3 -c "import json,sys;print(json.dumps({'ponte':sys.argv[1]}))" "$PONTE")" | grep -q '"ok": true' \
  && pass "telefono: ponte aggiunto dalle impostazioni (API della rete)" || fail "telefono ponte: $(tail -2 "$TMP/o.log")"
R=$(api GET /api/rete)
echo "$R" | grep -q "127.0.0.1:$((B+5))" && ! echo "$R" | grep -q "${PONTE##*,}" \
  && pass "telefono: la pagina vede l'indirizzo del ponte, mai la sua chiave" || fail "telefono elenco ponti: $R"
api POST /api/rete '{"ponte": "rovinata"}' | grep -q errore && pass "telefono: riga rovinata rifiutata" || fail "telefono: riga rovinata accettata"
api POST /api/rete '{"ponte_togli": "tutti"}' > /dev/null
api GET /api/rete | grep -q '"ponti": \[\]' && pass "telefono: ponti tolti" || fail "telefono: ponti non tolti"
[ "$KO" -eq 0 ]
