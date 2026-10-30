#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Un servizio .nyct (come la chat di un PC) viene spento e riacceso: il punto
# d'incontro conserva le sue vecchie registrazioni «morte». Chi lo cerca subito dopo
# deve raggiungerlo lo stesso, al primo colpo (prima veniva collegato a una
# registrazione morta: «SOCKS: connessione rifiutata»).
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · servizio .nyct spento e riacceso"
assicura_binario
rete_di_prova 19400
"$BIN" circuit --directory "$DIR" --authority "$PUB" --socks 127.0.0.1:19480 --hops 3 --control 127.0.0.1:19490 > "$TMP/c.log" 2>&1 & PIDS+=($!)
mkdir -p "$TMP/www" && echo "servizio vivo" > "$TMP/www/index.html"
python3 -m http.server 19481 --bind 127.0.0.1 --directory "$TMP/www" > /dev/null 2>&1 & PIDS+=($!)
sito() {
  "$BIN" site --backend 127.0.0.1:19481 --identity "$TMP/identita" --directory "$DIR" --authority "$PUB" --hops 3 > "$TMP/site$1.log" 2>&1 &
  SITO=$!; PIDS+=($SITO)
  ADDR=$(attendi_regex "$TMP/site$1.log" '[a-z2-7]{56}\.nyct' 20)
  sleep 4                                     # tempo di registrarsi ai punti d'incontro
}
visita() { curl -s -m 30 -o /dev/null -w '%{http_code}' --socks5-hostname 127.0.0.1:19480 "http://$ADDR/"; }

sito 1
[ "$(visita)" = 200 ] && pass "servizio raggiunto ($ADDR)" || { fail "servizio non raggiunto la prima volta"; exit 1; }
kill "$SITO"; sleep 1                         # spento: le sue registrazioni restano «morte» al punto d'incontro
sito 2                                        # riacceso con la stessa identità (stesso indirizzo)
C=$(visita)          # entro 30 s: le registrazioni morte (servizio spento di colpo) si scoprono in ~15 s
[ "$C" = 200 ] && pass "dopo il riavvio raggiunto al primo colpo (registrazioni morte saltate)" || fail "dopo il riavvio: risposta $C (registrazione morta)"
OK3=0; for _ in 1 2 3; do [ "$(visita)" = 200 ] && OK3=$((OK3 + 1)); done
[ "$OK3" = 3 ] && pass "e anche le visite successive (3 su 3)" || fail "visite successive: $OK3 su 3"
[ "$KO" -eq 0 ]
