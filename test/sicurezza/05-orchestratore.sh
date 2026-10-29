#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Orchestratore del telefono: senza segreto niente accesso; niente richieste da
# altre origini; nessun file fuori dalle cartelle consentite; nomi ripuliti;
# caricamenti troppo grandi rifiutati.
. "$(dirname "$0")/../lib.sh"
titolo "Sicurezza · orchestratore del telefono"
assicura_binario
H="$TMP/home"; mkdir -p "$H/.nyctalus-mobile"
TOK=segreto0123456789abcdef0123456789
printf %s "$TOK" > "$H/.nyctalus-mobile/token"
PORTA=19731
HOME="$H" NYCTALUS_MOBILE_PORT=$PORTA NYCTALUS_AVVIO_AUTOMATICO=0 NYCTALUS_BIN="$BIN" \
  python3 "$ROOT/mobile/orchestratore/nyctalus_mobile.py" > "$TMP/o.log" 2>&1 & PIDS+=($!)
[ -n "$(attendi_regex "$TMP/o.log" 'Nyctalus mobile' 10)" ] || { fail "orchestratore non partito"; exit 1; }
python3 - "$PORTA" "$TOK" > "$TMP/r.txt" <<'PY'
import http.client, sys, json
porta, tok = int(sys.argv[1]), sys.argv[2]
def r(m, u, corpo=None, h=None, cookie=True):
    c = http.client.HTTPConnection("127.0.0.1", porta, timeout=10); hh = dict(h or {})
    if cookie: hh["Cookie"] = "nyct=" + tok
    c.request(m, u, body=corpo, headers=hh); x = c.getresponse(); return x.status, x.read()
print("senza", r("GET", "/api/stato", cookie=False)[0])
print("sbagliato", r("GET", "/api/stato", h={"Cookie": "nyct=xxxxxxxxxxxxxxxxxxxx"}, cookie=False)[0])
print("giusto", r("GET", "/api/stato")[0])
print("ws-senza", r("GET", "/ws", h={"Upgrade": "websocket", "Connection": "Upgrade", "Sec-WebSocket-Key": "x"}, cookie=False)[0])
print("passwd", r("GET", "/api/file?p=" + "/etc/passwd".encode().hex())[0])
print("statico", r("GET", "/../../../../etc/passwd")[0])
print("origine", r("POST", "/api/servizi/ferma", b"", {"Origin": "http://cattivo.example"})[0])
st, b = r("POST", "/api/carica?nome=..%2F..%2F.bashrc", b"x", {"Origin": "http://127.0.0.1:%d" % porta})
print("nome", json.loads(b).get("percorso", "").rsplit("/", 1)[-1])
print("enorme", r("POST", "/api/carica?nome=a", None, {"Origin": "http://127.0.0.1:%d" % porta, "Content-Length": str(10**12)})[0])
PY
v() { grep -q "^$1 $2$" "$TMP/r.txt"; }
v senza 403 && pass "senza segreto → 403" || fail "accesso senza segreto"
v sbagliato 403 && pass "segreto sbagliato → 403" || fail "accesso con segreto sbagliato"
v giusto 200 && pass "segreto giusto → 200" || fail "segreto giusto rifiutato"
v ws-senza 403 && pass "WebSocket senza segreto → 403" || fail "WebSocket aperto senza segreto (grave)"
v passwd 404 && pass "/etc/passwd via /api/file → negato" || fail "lettura di file di sistema (grave)"
grep -q "^statico 200" "$TMP/r.txt" && fail "uscita dalla cartella della web-app (grave)" || pass "../ fuori dalla web-app → negato"
v origine 403 && pass "comandi da un'altra origine → 403" || fail "richiesta da altra origine accettata"
v nome bashrc && pass "nome caricato ripulito (../../.bashrc → bashrc)" || fail "nome caricato non ripulito: $(grep ^nome "$TMP/r.txt")"
v enorme 413 && pass "caricamento enorme → 413" || fail "caricamento enorme accettato"
[ "$KO" -eq 0 ]
