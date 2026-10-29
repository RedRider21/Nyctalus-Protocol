#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Attacco dal vivo: qualcuno si collega alla chat di Bob dichiarando di essere
# Alice, ma senza la chiave privata di Alice. Deve essere respinto e Bob non
# deve mai vedere «Alice in linea» né ricevere messaggi a suo nome.
. "$(dirname "$0")/../lib.sh"
titolo "Sicurezza · impostore che si spaccia per un contatto"
assicura_binario
# Alice esiste davvero (ci serve solo il suo indirizzo pubblico)
"$BIN" chat --data "$TMP/alice" --backend 127.0.0.1:19281 --control 127.0.0.1:19291 --socks 127.0.0.1:1 > "$TMP/a.log" 2>&1 & PIDS+=($!)
"$BIN" chat --data "$TMP/bob" --backend 127.0.0.1:19282 --control 127.0.0.1:19292 --socks 127.0.0.1:1 > "$TMP/b.log" 2>&1 & PIDS+=($!)
ALICE=$(attendi_regex "$TMP/a.log" '[a-z2-7]{56}\.nyct' 10)
[ -n "$ALICE" ] && [ -n "$(attendi_regex "$TMP/b.log" '[a-z2-7]{56}\.nyct' 10)" ] || { fail "motori non partiti"; exit 1; }
# Bob ascolta gli eventi mentre l'impostore attacca
controllo 19292 - 6 SUB > "$TMP/eventi-bob.txt" & PIDS+=($!)
sleep 0.5
python3 - "$ALICE" > "$TMP/attacco.txt" <<'PY'
import os, socket, sys, time
alice = sys.argv[1]
def frame(d): return len(d).to_bytes(2, "big") + d
def leggi(s):
    n = s.recv(2)
    if len(n) < 2: return None
    n = int.from_bytes(n, "big"); b = b""
    while len(b) < n:
        x = s.recv(n - len(b))
        if not x: return None
        b += x
    return b
esiti = []
# 1) dichiara di essere Alice e risponde alla sfida con byte a caso
s = socket.create_connection(("127.0.0.1", 19282), timeout=5)
s.sendall(frame(b"H" + alice.encode()))
sfida = leggi(s)
esiti.append("sfida ricevuta" if sfida else "nessuna sfida")
s.sendall(frame(os.urandom(48)))
s.sendall(frame(b"M 1 messaggio falso a nome di Alice"))
time.sleep(1)
try:
    resto = s.recv(10)
    esiti.append("chiusa dal servizio" if resto == b"" else "connessione ANCORA aperta")
except Exception:
    esiti.append("chiusa dal servizio")
# 2) saluto malformato e indirizzo inventato
for saluto in (b"Xspazzatura", b"H" + b"a" * 56 + b".nyct"):
    s = socket.create_connection(("127.0.0.1", 19282), timeout=5)
    s.sendall(frame(saluto)); time.sleep(0.4)
    try: esiti.append("rifiutato" if s.recv(10) == b"" else "accettato?")
    except Exception: esiti.append("rifiutato")
print("\n".join(esiti))
PY
sleep 1
grep -q "sfida ricevuta" "$TMP/attacco.txt" && pass "Bob sfida chi dichiara di essere Alice" || fail "nessuna sfida d'identità"
grep -q "chiusa dal servizio" "$TMP/attacco.txt" && pass "l'impostore senza chiave viene buttato fuori" || fail "l'impostore resta collegato (grave)"
[ "$(grep -c rifiutato "$TMP/attacco.txt")" -eq 2 ] && pass "saluti malformati / indirizzi inventati → rifiutati" || fail "saluto malformato accettato"
grep -q "ST $ALICE 1" "$TMP/eventi-bob.txt" && fail "Bob ha visto «Alice in linea» (grave)" || pass "Bob non ha mai visto «Alice in linea»"
grep -q "^MSG " "$TMP/eventi-bob.txt" && fail "Bob ha ricevuto il messaggio falso (grave)" || pass "nessun messaggio falso consegnato"
[ ! -s "$TMP/bob/cronologia/$ALICE.log" ] && pass "niente nella cronologia di Bob" || fail "messaggio falso salvato in cronologia (grave)"
[ "$KO" -eq 0 ]
