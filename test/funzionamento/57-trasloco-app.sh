#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# TRASLOCO dall'app con Termux all'app «con tutto» (nyctalus app): Termux impacchetta i dati di
# Nyctalus e li consegna all'app sul telefono stesso (127.0.0.1, con il segreto). Qui due HOME
# finte: la «Termux» con identità, contatti, un allegato e la configurazione della rete; la «app».
# Si prova: il pacchetto si sistema e i servizi ripartono con la STESSA identità; il segreto serve;
# niente sovrascritture se un'identità c'è già; percorsi cattivi (../, assoluti, collegamenti,
# altri file della casa) ignorati; pacchetto senza identità rifiutato.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · trasloco dei dati da Termux all'app con tutto"
assicura_binario
command -v tar >/dev/null || { salta "tar non installato"; exit 0; }
T="$TMP/termux"; A="$TMP/app"; mkdir -p "$T/.nyctalus-chat/allegati/mario" "$T/.nyctalus-mobile" "$A/.nyctalus-mobile"
# una vera identità della chat, fatta dal motore nella casa «Termux»
HOME="$T" "$BIN" chat --data "$T/.nyctalus-chat" --backend 127.0.0.1:24910 --control 127.0.0.1:24911 --socks 127.0.0.1:24912 > "$TMP/c0.log" 2>&1 & C0=$!
ID0=$(attendi_regex "$TMP/c0.log" '[a-z2-7]{56}\.nyct' 15); kill $C0 2>/dev/null; wait $C0 2>/dev/null
[ -n "$ID0" ] || { fail "identità di prova non creata"; exit 1; }
printf 'mario@nyct\n' > "$T/.nyctalus-chat/contatti"; head -c 300000 /dev/urandom > "$T/.nyctalus-chat/allegati/mario/foto.jpg"
echo '{"directory_url": "http://203.0.113.5:18800/directory.txt", "authority": "'"$(printf 'ab%.0s' $(seq 32))"'", "nome_rete": "Rete di Mario", "porte": {"http": 1}}' > "$T/.nyctalus-mobile/config.json"
echo "SEGRETO-DI-TERMUX" > "$T/.nyctalus-mobile/token"; echo "export X=1" > "$T/.bashrc"
( cd "$T" && tar cf "$TMP/trasloco.tar" .nyctalus-chat .nyctalus-mobile/config.json )
TOK=segreto0123456789abcdef0123456789; printf %s "$TOK" > "$A/.nyctalus-mobile/token"
echo '{"porte": {"http": 24901, "controllo": 24902, "backend": 24903, "socks": 24904, "socks_locale": 24905, "monitor": 24906, "nodi": [24921,24922,24923,24924], "socks_browser": 24907}}' > "$A/.nyctalus-mobile/config.json"
HOME="$A" NYCTALUS_BIN="$BIN" NYCTALUS_AVVIO_AUTOMATICO=0 "$BIN" app > "$TMP/o.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/o.log" "Nyctalus mobile" 15 > /dev/null || { fail "nyctalus app non partito"; exit 1; }
U=http://127.0.0.1:24901
manda() { curl -s -o "$TMP/r.json" -w "%{http_code}" -X POST -H "Cookie: nyct=$1" --data-binary @"$2" "$U/api/trasloco${3:-}"; }
[ "$(manda sbagliato "$TMP/trasloco.tar")" = 403 ] && [ ! -e "$A/.nyctalus-chat/identita" ] && pass "senza il segreto dell'app il pacchetto non entra (403)" || fail "trasloco senza segreto accettato"
grep -q '"identita": false' <(curl -s -b "nyct=$TOK" "$U/api/trasloco") && pass "l'app dice che qui non c'è ancora un'identità (la domanda del trasloco)" || fail "stato del trasloco"
C=$(manda "$TOK" "$TMP/trasloco.tar")
[ "$C" = 200 ] && grep -q '"ok": true' "$TMP/r.json" && cmp -s "$T/.nyctalus-chat/identita" "$A/.nyctalus-chat/identita" && cmp -s "$T/.nyctalus-chat/allegati/mario/foto.jpg" "$A/.nyctalus-chat/allegati/mario/foto.jpg" \
  && pass "trasloco: identità, contatti e allegati arrivano identici ($(python3 -c "import json;d=json.load(open('$TMP/r.json'));print(d['file'],'file')"))" || fail "trasloco: $C $(cat "$TMP/r.json")"
[ "$(stat -c %a "$A/.nyctalus-chat/identita")" = 600 ] && [ "$(stat -c %a "$A/.nyctalus-chat")" = 700 ] && pass "file 600 e cartelle 700 (privati)" || fail "permessi: $(stat -c %a "$A/.nyctalus-chat/identita") $(stat -c %a "$A/.nyctalus-chat")"
python3 -c "import json,sys;c=json.load(open(sys.argv[1]));sys.exit(0 if c.get('nome_rete')=='Rete di Mario' and c['porte']['http']==24901 else 1)" "$A/.nyctalus-mobile/config.json" \
  && pass "la rete di prima (codice, nome) passa; le porte restano quelle dell'app" || fail "configurazione: $(cat "$A/.nyctalus-mobile/config.json")"
[ "$(cat "$A/.nyctalus-mobile/token")" = "$TOK" ] && [ ! -e "$A/.bashrc" ] && pass "il segreto di Termux e gli altri file della casa NON passano" || fail "passati file non dovuti"
# l'identità è proprio quella: il motore della chat dell'app la usa
HOME="$A" "$BIN" chat --data "$A/.nyctalus-chat" --backend 127.0.0.1:24913 --control 127.0.0.1:24914 --socks 127.0.0.1:24915 > "$TMP/c1.log" 2>&1 & C1=$!
ID1=$(attendi_regex "$TMP/c1.log" '[a-z2-7]{56}\.nyct' 15); kill $C1 2>/dev/null; wait $C1 2>/dev/null
[ "$ID1" = "$ID0" ] && pass "dopo il trasloco il Contact ID è lo stesso di prima (gli amici non devono rifare nulla)" || fail "Contact ID cambiato: $ID0 → $ID1"
[ "$(manda "$TOK" "$TMP/trasloco.tar")" = 409 ] && pass "con un'identità già presente non si sovrascrive nulla (409)" || fail "sovrascrittura senza forza"
# pacchetto cattivo: ../, percorso assoluto, collegamento, altri file; e nessuna identità → rifiutato
M="$TMP/cattivo"; mkdir -p "$M/.nyctalus-chat"; echo x > "$M/.nyctalus-chat/nota"; ln -s /etc/passwd "$M/.nyctalus-chat/link"
python3 - "$M" "$TMP/cattivo.tar" <<'PY'
import io, sys, tarfile
m, out = sys.argv[1], sys.argv[2]
with tarfile.open(out, "w", format=tarfile.GNU_FORMAT) as t:
    for nome, dati in ((".nyctalus-chat/../../fuori.txt", b"fuori"), ("/tmp/assoluto.txt", b"a"), (".bashrc", b"b")):
        i = tarfile.TarInfo(nome); i.size = len(dati); t.addfile(i, io.BytesIO(dati))
    t.add(m + "/.nyctalus-chat/link", arcname=".nyctalus-chat/link")
    t.add(m + "/.nyctalus-chat/nota", arcname=".nyctalus-chat/" + "n" * 150)   # nome lungo (GNU)
PY
A2="$TMP/app2"; mkdir -p "$A2/.nyctalus-mobile"; printf %s "$TOK" > "$A2/.nyctalus-mobile/token"
echo '{"porte": {"http": 24951, "controllo": 24952, "backend": 24953, "socks": 24954, "socks_locale": 24955, "monitor": 24956, "socks_browser": 24957}}' > "$A2/.nyctalus-mobile/config.json"
HOME="$A2" NYCTALUS_BIN="$BIN" NYCTALUS_AVVIO_AUTOMATICO=0 "$BIN" app > "$TMP/o2.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/o2.log" "Nyctalus mobile" 15 > /dev/null
C=$(curl -s -o "$TMP/r2.json" -w '%{http_code}' -X POST -H "Cookie: nyct=$TOK" --data-binary @"$TMP/cattivo.tar" "http://127.0.0.1:24951/api/trasloco")
[ "$C" = 400 ] && grep -q "identit" "$TMP/r2.json" && [ ! -e "$TMP/fuori.txt" ] && [ ! -e /tmp/assoluto.txt ] && [ ! -e "$A2/.bashrc" ] && [ ! -e "$A2/.nyctalus-chat" ] \
  && pass "pacchetto cattivo (../, assoluto, collegamento, altri file, nessuna identità): rifiutato, nulla scritto fuori" || fail "pacchetto cattivo: $C $(cat "$TMP/r2.json")"
[ "$KO" -eq 0 ]
