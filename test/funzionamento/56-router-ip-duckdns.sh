#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# IL ROUTER, SENZA SITI ESTERNI: IP pubblico chiesto al router (NAT-PMP, UPnP IGD) e porte aperte
# da sole; DuckDNS facoltativo. Qui un finto router (NAT-PMP su UDP, UPnP su HTTP) e un finto
# DuckDNS sul PC stesso; HOME finta. Si prova anche il caso del router che non risponde e del
# CGNAT (IP condiviso dell'operatore), e «casa --internet auto».
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · IP e porte dal router (NAT-PMP, UPnP), DuckDNS"
assicura_binario
G="$ROOT/pc/gestore-rete/nyctalus-rete"
export HOME="$TMP/casa"; mkdir -p "$HOME"
cat > "$TMP/router.py" <<'PY'
import http.server, json, socket, struct, sys, threading
ip, porta_pmp, porta_http, registro = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
def scrivi(t):
    open(registro, "a").write(t + "\n")
def pmp():
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.bind(("127.0.0.1", porta_pmp))
    while True:
        d, a = s.recvfrom(64)
        if d[:2] == b"\x00\x00":
            s.sendto(b"\x00\x80\x00\x00" + struct.pack(">I", 1) + socket.inet_aton(ip), a)
        elif len(d) == 12 and d[1] in (1, 2):
            p = struct.unpack(">H", d[4:6])[0]; scrivi("pmp %d/%s" % (p, "udp" if d[1] == 1 else "tcp"))
            s.sendto(bytes([0, 128 + d[1], 0, 0]) + struct.pack(">I", 1) + d[4:6] * 2 + d[8:12], a)
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_GET(self):
        if self.path == "/desc.xml":
            x = ('<?xml version="1.0"?><root xmlns="urn:schemas-upnp-org:device-1-0"><device><serviceList><service>'
                 '<serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>/ctl</controlURL>'
                 '</service></serviceList></device></root>').encode()
        elif self.path.startswith("/update?"):
            scrivi("duckdns " + self.path); x = b"OK" if "token=0123" in self.path else b"KO"
        else:
            self.send_response(404); self.end_headers(); return
        self.send_response(200); self.end_headers(); self.wfile.write(x)
    def do_POST(self):
        corpo = self.rfile.read(int(self.headers["Content-Length"])).decode()
        az = self.headers.get("SOAPAction", "")
        if az.endswith('#GetExternalIPAddress"'):
            r = "<NewExternalIPAddress>%s</NewExternalIPAddress>" % ip
        else:
            import re
            scrivi("upnp %s/%s" % (re.search(r"<NewExternalPort>(\d+)", corpo).group(1), re.search(r"<NewProtocol>(\w+)", corpo).group(1).lower()))
            r = ""
        self.send_response(200); self.end_headers()
        self.wfile.write(('<?xml version="1.0"?><s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body>%s</s:Body></s:Envelope>' % r).encode())
threading.Thread(target=pmp, daemon=True).start()
http.server.ThreadingHTTPServer(("127.0.0.1", porta_http), H).serve_forever()
PY
python3 "$TMP/router.py" 203.0.113.9 24351 24352 "$TMP/router.log" & PIDS+=($!)
python3 "$TMP/router.py" 100.70.1.2 24353 24354 "$TMP/cgnat.log" & PIDS+=($!)
for _ in $(seq 1 50); do porta_occupata 24352 && porta_occupata 24354 && break; sleep 0.2; done

# 1) NAT-PMP: IP e porte
O=$(NYCTALUS_GATEWAY=127.0.0.1:24351 NYCTALUS_UPNP_URL=http://127.0.0.1:1/x python3 "$G" ip)
echo "$O" | grep -q "203.0.113.9.*NAT-PMP" && pass "IP pubblico chiesto al router con NAT-PMP (nessun sito esterno)" || fail "ip NAT-PMP: $O"
NYCTALUS_GATEWAY=127.0.0.1:24351 python3 "$G" porte --nodi 2 --porta-base 24701 --porta 24800 > "$TMP/p1.txt" 2>&1
[ "$(grep -c "^pmp " "$TMP/router.log")" = 5 ] && grep -q "pmp 24701/udp" "$TMP/router.log" && grep -q "pmp 24800/tcp" "$TMP/router.log" && grep -q "NAT-PMP" "$TMP/p1.txt" \
  && pass "porte aperte dal router con NAT-PMP: UDP e TCP dei nodi, TCP della pagina (5)" || fail "porte NAT-PMP: $(cat "$TMP/p1.txt") · $(cat "$TMP/router.log")"
# 2) UPnP (NAT-PMP assente)
: > "$TMP/router.log"
O=$(NYCTALUS_GATEWAY=127.0.0.1:1 NYCTALUS_UPNP_URL=http://127.0.0.1:24352/desc.xml python3 "$G" ip)
echo "$O" | grep -q "203.0.113.9.*UPnP" && pass "senza NAT-PMP: IP chiesto al router con UPnP" || fail "ip UPnP: $O"
NYCTALUS_GATEWAY=127.0.0.1:1 NYCTALUS_UPNP_URL=http://127.0.0.1:24352/desc.xml python3 "$G" porte --nodi 2 --porta-base 24701 --porta 24800 > "$TMP/p2.txt" 2>&1
[ "$(grep -c "^upnp " "$TMP/router.log")" = 5 ] && grep -q "UPnP" "$TMP/p2.txt" && pass "porte aperte dal router con UPnP (AddPortMapping)" || fail "porte UPnP: $(cat "$TMP/p2.txt")"
# 3) router muto e CGNAT
O=$(NYCTALUS_GATEWAY=127.0.0.1:1 NYCTALUS_UPNP_URL=http://127.0.0.1:1/x python3 "$G" ip)
echo "$O" | grep -q "il router non lo dice" && pass "router che non risponde: lo dice e indica le alternative (pagina del router, DuckDNS)" || fail "router muto: $O"
NYCTALUS_GATEWAY=127.0.0.1:1 NYCTALUS_UPNP_URL=http://127.0.0.1:1/x python3 "$G" porte > "$TMP/p3.txt" 2>&1; E=$?
[ "$E" != 0 ] && grep -q "inoltrale a mano" "$TMP/p3.txt" && pass "router che non apre le porte: errore chiaro (inoltro a mano)" || fail "porte, router muto: $E"
O=$(NYCTALUS_GATEWAY=127.0.0.1:24353 python3 "$G" ip)
echo "$O" | grep -q "CGNAT" && pass "IP condiviso dell'operatore (100.64…): avviso CGNAT" || fail "cgnat: $O"
# 4) DuckDNS
export NYCTALUS_DUCKDNS_URL="http://127.0.0.1:24352/update"
python3 "$G" duckdns --dominio prova-nyct --token 01234567-89ab-cdef-0123-456789abcdef > "$TMP/d1.txt" 2>&1
grep -q "prova-nyct.duckdns.org aggiornato" "$TMP/d1.txt" && grep -q "domains=prova-nyct&token=0123" "$TMP/router.log" && [ "$(stat -c %a "$HOME/.nyctalus-rete/duckdns.json")" = 600 ] \
  && pass "DuckDNS: dominio e token salvati (leggibili solo da te) e nome aggiornato" || fail "duckdns: $(cat "$TMP/d1.txt")"
python3 "$G" duckdns --dominio prova-nyct --token ffffffff-89ab-cdef-0123-456789abcdef > "$TMP/d2.txt" 2>&1; E=$?
[ "$E" != 0 ] && grep -q "dominio o token sbagliati" "$TMP/d2.txt" && pass "DuckDNS con token sbagliato: errore chiaro" || fail "duckdns token sbagliato: $E $(cat "$TMP/d2.txt")"
python3 "$G" duckdns --dominio "a;b" --token x > /dev/null 2>&1 && fail "dominio non valido accettato" || pass "dominio o token non validi: rifiutati"
A1=$(NYCTALUS_GATEWAY=127.0.0.1:24351 python3 "$G" ip --per-rete)
python3 "$G" duckdns --togli > /dev/null 2>&1
A2=$(NYCTALUS_GATEWAY=127.0.0.1:24351 python3 "$G" ip --per-rete)
[ "$A1" = "prova-nyct.duckdns.org" ] && [ "$A2" = "203.0.113.9" ] \
  && pass "indirizzo per la rete: il nome DuckDNS se c'è (fisso), altrimenti l'IP detto dal router" || fail "per-rete: «$A1» «$A2»"
# 5) casa --internet auto (niente DuckDNS: IP dal router, nessuna richiesta DNS fuori dal PC);
#    il router apre le porte da solo; con DuckDNS configurato il nome si aggiorna da solo
python3 "$G" duckdns --dominio prova-nyct --token 01234567-89ab-cdef-0123-456789abcdef > /dev/null 2>&1
mv "$HOME/.nyctalus-rete/duckdns.json" "$TMP/duckdns.json"
: > "$TMP/router.log"
NYCTALUS_BIN="$BIN" NYCTALUS_RUBRICA_PORTE=24330,24331,24332,24333,24334 NYCTALUS_GATEWAY=127.0.0.1:24351 \
  python3 "$G" casa --internet auto --porta 24320 --porta-base 24321 --nodi 3 --non-importare < /dev/null > "$TMP/casa.log" 2>&1 & CASA=$!; PIDS+=($CASA)
for _ in $(seq 1 240); do grep -q "Lascia questa finestra aperta\|✗" "$TMP/casa.log" && break; sleep 0.5; done
grep -q "indirizzo pubblico: 203.0.113.9" "$TMP/casa.log" && grep -q "il router ha aperto da solo 7 porte" "$TMP/casa.log" \
  && pass "casa --internet auto: IP dal router, e il router apre da solo le 7 porte (niente inoltri a mano)" || fail "casa auto: $(grep -E "pubblico|router|✗" "$TMP/casa.log" | head -4)"
kill -INT $CASA 2>/dev/null; for _ in $(seq 1 40); do kill -0 $CASA 2>/dev/null || break; sleep 0.25; done
cp "$TMP/duckdns.json" "$HOME/.nyctalus-rete/duckdns.json"
NYCTALUS_BIN="$BIN" NYCTALUS_RUBRICA_PORTE=24330,24331,24332,24333,24334 NYCTALUS_GATEWAY=127.0.0.1:1 NYCTALUS_UPNP_URL=http://127.0.0.1:1/x \
  python3 "$G" casa --internet 203.0.113.9 --porta 24320 --porta-base 24321 --nodi 3 --non-importare < /dev/null > "$TMP/casa2.log" 2>&1 & CASA=$!; PIDS+=($CASA)
for _ in $(seq 1 240); do grep -q "Lascia questa finestra aperta\|✗" "$TMP/casa2.log" && break; sleep 0.5; done
for _ in $(seq 1 20); do grep -q "duckdns " "$TMP/router.log" && break; sleep 0.5; done
grep -q "duckdns " "$TMP/router.log" && grep -q "inoltrate a mano" "$TMP/casa2.log" \
  && pass "con la rete accesa il nome DuckDNS si aggiorna da solo; router senza UPnP/NAT-PMP: lo dice" || fail "duckdns in sottofondo: $(grep -E "router|duckdns" "$TMP/casa2.log" | head -3)"
kill -INT $CASA 2>/dev/null; for _ in $(seq 1 40); do kill -0 $CASA 2>/dev/null || break; sleep 0.25; done
[ "$KO" -eq 0 ]
