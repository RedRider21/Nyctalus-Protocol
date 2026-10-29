#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Il percorso completo di una rete condivisa, tutto su questo PC:
#  1) SERVER: installatore dei nodi in una radice finta (systemctl finto), servizio
#     systemd scritto bene, nodi avviati col comando ESATTO del servizio, riga stabile
#     ai riavvii;
#  2) GESTORE: nyctalus-rete crea / aggiungi / firma / verifica / codice / servi;
#  3) CLIENT: il codice importato sul PC e sul telefono (orchestratore), che passa
#     alla rete condivisa; codici rovinati rifiutati.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · server, gestore della rete e client"
assicura_binario
RADICE="$TMP/radice"; F="$TMP/finti"; mkdir -p "$RADICE" "$F"
printf '#!/bin/sh\necho "$*" >> "%s/systemctl.txt"\n' "$TMP" > "$F/systemctl"
printf '#!/bin/sh\necho "$*" >> "%s/useradd.txt"\n' "$TMP" > "$F/useradd"
chmod +x "$F"/*
PK="$TMP/Nyctalus-Server"; mkdir -p "$PK/bin"
cp "$BIN" "$PK/bin/nyctalus"
cp "$ROOT/server/installa-nodo.sh" "$PK/installa.sh"; cp "$ROOT/server/nyctalus-server" "$ROOT/server/disinstalla.sh" "$PK/"
S() { env NYCTALUS_RADICE="$RADICE" PATH="$F:$PATH" "$@"; }

# --- 1) server ---------------------------------------------------------------------------
S bash "$PK/installa.sh" --ip 127.0.0.1 --nodi 4 --porta 19971 --si > "$TMP/inst.log" 2>&1 \
  && pass "installatore dei nodi completato" || { fail "installatore: $(tail -3 "$TMP/inst.log")"; exit 1; }
U="$RADICE/etc/systemd/system/nyctalus-nodo@.service"
grep -q "^ExecStart=/opt/nyctalus/bin/nyctalus node --listen 0.0.0.0:%i --data \${NYCTALUS_STATO}/nodo-%i --announce \${NYCTALUS_IP}:%i" "$U" \
  && grep -q "^Restart=always" "$U" && grep -q "^NoNewPrivileges=true" "$U" && grep -q "^EnvironmentFile=/etc/nyctalus/server.conf" "$U" \
  && pass "servizio systemd: riparte da solo, privilegi ridotti, percorsi reali" || fail "unità systemd: $(cat "$U")"
[ "$(grep -c 'enable --now nyctalus-nodo@' "$TMP/systemctl.txt")" = 4 ] && pass "4 servizi abilitati e avviati" || fail "servizi: $(cat "$TMP/systemctl.txt")"
[ "$(stat -c %a "$RADICE/var/lib/nyctalus/nodo-19971/segreto")" = 600 ] && [ "$(stat -c %a "$RADICE/var/lib/nyctalus/nodo-19971/tls-chiave.der")" = 600 ] \
  && pass "chiavi dei nodi leggibili solo dal proprietario" || fail "permessi delle chiavi"
RIGHE="$RADICE/etc/nyctalus/righe-nodi.txt"
[ "$(grep -cE '^127\.0\.0\.1:1997[1-4],[0-9a-f]{64},[0-9a-f]{64}$' "$RIGHE")" = 4 ] && pass "4 righe d'elenco col l'IP annunciato" || fail "righe: $(cat "$RIGHE")"
[ "$(S bash "$PK/nyctalus-server" righe | wc -l)" = 4 ] && pass "nyctalus-server righe" || fail "nyctalus-server righe"

# avvio dei nodi col comando del servizio (come farebbe systemd)
. "$RADICE/etc/nyctalus/server.conf"
for p in 19971 19972 19973 19974; do
  CMD=$(grep '^ExecStart=' "$U" | cut -d= -f2- | sed -e "s#%i#$p#g" -e "s#\${NYCTALUS_STATO}#$NYCTALUS_STATO#g" \
        -e "s#\${NYCTALUS_IP}#$NYCTALUS_IP#g" -e "s#\$NYCTALUS_OPZIONI#$NYCTALUS_OPZIONI#g" -e "s#^/opt#$RADICE/opt#" \
        -e "s# /var/lib# $RADICE/var/lib#g")
  $CMD > "$TMP/n$p.log" 2>&1 & PIDS+=($!)
done
sleep 1.5
R1=$(grep -oE 'directory entry: +[^ ]+' "$TMP/n19971.log" | awk '{print $3}')
[ -n "$R1" ] && [ "$R1" = "$(head -1 "$RIGHE")" ] && pass "il nodo avviato dal servizio ha la stessa riga creata all'installazione" || fail "riga diversa dopo l'avvio: $R1"

# --- 2) gestore --------------------------------------------------------------------------
G() { env HOME="$TMP/gestore" NYCTALUS_BIN="$BIN" python3 "$ROOT/pc/gestore-rete/nyctalus-rete" "$@"; }
G crea --nome "Rete di prova del test" > "$TMP/g.log" 2>&1 && pass "gestore: rete creata" || fail "crea: $(cat "$TMP/g.log")"
[ "$(stat -c %a "$TMP/gestore/.nyctalus-rete/autorita.json")" = 600 ] && [ "$(stat -c %a "$TMP/gestore/.nyctalus-rete")" = 700 ] \
  && pass "chiave segreta dell'autorità protetta (600, cartella 700)" || fail "permessi dell'autorità"
G crea > /dev/null 2>&1 && fail "seconda «crea» ha sovrascritto la rete" || pass "seconda «crea» rifiutata (la rete non si perde per sbaglio)"
G aggiungi "rotta,riga" > /dev/null 2>&1 && fail "riga non valida accettata" || pass "riga d'elenco non valida rifiutata"
G aggiungi --file "$RIGHE" > /dev/null 2>&1 && pass "gestore: righe dei nodi aggiunte" || fail "aggiungi"
G firma > /dev/null 2>&1 && grep -q '^firma ' "$TMP/gestore/.nyctalus-rete/directory.txt" && pass "elenco firmato" || fail "firma"
G verifica > "$TMP/ver.txt" 2>&1; grep -q "4/4" "$TMP/ver.txt" && pass "verifica: 4 nodi su 4 raggiungibili" || fail "verifica: $(tail -3 "$TMP/ver.txt")"
env HOME="$TMP/gestore" NYCTALUS_BIN="$BIN" python3 "$ROOT/pc/gestore-rete/nyctalus-rete" servi --porta 19975 --ip 127.0.0.1 \
  > "$TMP/servi.log" 2>&1 & PIDS+=($!)      # (diretto, non dentro una funzione: così la pulizia lo chiude)
for _ in $(seq 1 30); do grep -q "Codice di rete" "$TMP/servi.log" && break; sleep 0.2; done
COD=$(grep -oE 'NYCT1:[A-Za-z0-9_-]+' "$TMP/servi.log" | head -1)
[ -n "$COD" ] && pass "codice di rete creato" || fail "codice non creato: $(cat "$TMP/servi.log")"
python3 -c "import base64,json,sys;c=sys.argv[1][6:];d=json.loads(base64.urlsafe_b64decode(c+'='*(-len(c)%4)));sys.exit(0 if set(d)=={'v','nome','directory_url','authority'} else 1)" "$COD" \
  && ! grep -q "$(python3 -c "import json;print(json.load(open('$TMP/gestore/.nyctalus-rete/autorita.json'))['segreto'])")" <<< "$COD" \
  && pass "il codice contiene solo nome, URL e chiave PUBBLICA (mai la segreta)" || fail "contenuto del codice"
[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:19975/directory.txt)" = 200 ] \
  && [ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:19975/autorita.json)" = 404 ] \
  && pass "pubblicato solo l'elenco (la chiave dell'autorità non è raggiungibile)" || fail "servitore dell'elenco"

# --- 3) client -----------------------------------------------------------------------------
C() { env HOME="$TMP/pc" python3 "$ROOT/pc/gestore-rete/nyctalus-rete" "$@"; }
C importa "NYCT1:rovinato" > /dev/null 2>&1 && fail "codice rovinato accettato" || pass "PC: codice rovinato rifiutato"
C importa "$COD" > /dev/null 2>&1 && python3 -c "import json,sys;c=json.load(open(sys.argv[1]));sys.exit(0 if c['directory_url'].endswith(':19975/directory.txt') and len(c['authority'])==64 else 1)" "$TMP/pc/.nyctalus-browser/config.json" \
  && pass "PC: codice importato (chat e browser useranno la rete)" || fail "importazione sul PC"
C importa --prova > /dev/null 2>&1 && ! grep -q directory_url "$TMP/pc/.nyctalus-browser/config.json" && pass "PC: ritorno alla rete di prova" || fail "importa --prova"

H="$TMP/tel"; mkdir -p "$H/.nyctalus-mobile"
echo '{"porte": {"http": 19976, "controllo": 19977, "backend": 19978, "socks": 19979, "socks_locale": 19980, "monitor": 19981, "nodi": [19982,19983,19984,19985]}}' > "$H/.nyctalus-mobile/config.json"
printf %s "segreto0123456789abcdef0123456789" > "$H/.nyctalus-mobile/token"
HOME="$H" NYCTALUS_BIN="$BIN" python3 "$ROOT/mobile/orchestratore/nyctalus_mobile.py" > "$TMP/o.log" 2>&1 & PIDS+=($!)
attendi_regex "$TMP/o.log" 'servizi pronti' 40 > /dev/null
api() { python3 - "$@" <<'PY'
import json, sys, urllib.request, urllib.error
metodo, percorso = sys.argv[1], sys.argv[2]
corpo = sys.argv[3].encode() if len(sys.argv) > 3 else None
r = urllib.request.Request("http://127.0.0.1:19976" + percorso, data=corpo, method=metodo,
                           headers={"Cookie": "nyct=segreto0123456789abcdef0123456789", "Content-Type": "application/json",
                                    "Origin": "http://127.0.0.1:19976"})
try: print(urllib.request.urlopen(r, timeout=5).read().decode())
except urllib.error.HTTPError as e: print(e.code, e.read().decode())
PY
}
api POST /api/rete '{"codice":"NYCT1:xx"}' | grep -q '^400' && pass "telefono: codice rovinato rifiutato" || fail "telefono: codice rovinato accettato"
api POST /api/rete "{\"codice\":\"$COD\"}" | grep -q '"ok": true' && pass "telefono: codice di rete importato" || fail "telefono: importazione"
for _ in $(seq 1 120); do api GET /api/stato | grep -q '"stato": "pronto", .*"modalita": "pubblica"' && break; sleep 0.5; done
api GET /api/stato | grep -q '"modalita": "pubblica"' && api GET /api/stato | grep -q '"rete": "Rete di prova del test"' \
  && pass "telefono: servizi riavviati sulla rete condivisa «Rete di prova del test»" || fail "telefono: $(api GET /api/stato) $(tail -2 "$TMP/o.log")"
api GET /api/rete | grep -q '"condivisa": true' && pass "telefono: /api/rete dice «condivisa»" || fail "/api/rete"
[ "$(stat -c %a "$H/.nyctalus-mobile/config.json")" = 600 ] && grep -q '"porte"' "$H/.nyctalus-mobile/config.json" \
  && pass "telefono: configurazione privata e porte personalizzate conservate" || fail "config del telefono"

# --- disinstallazione del server ----------------------------------------------------------
S bash "$RADICE/opt/nyctalus/disinstalla.sh" > /dev/null 2>&1
[ ! -e "$U" ] && [ ! -e "$RADICE/opt/nyctalus/bin" ] && [ -s "$RADICE/var/lib/nyctalus/nodo-19971/segreto" ] \
  && pass "disinstallazione: servizi e programma tolti, identità conservate" || fail "disinstallazione del server"
[ "$KO" -eq 0 ]
