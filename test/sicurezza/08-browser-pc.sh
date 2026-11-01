#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Browser del PC in modalità 🦇, in uno spazio di rete isolato con cattura dei pacchetti:
# a rete non pronta non parte nulla (a prova di errore), poi tutto passa dal proxy, i nomi li
# risolve la rete (zero DNS), il sito non riceve visite dirette; una scheda normale sì (controllo).
if [ -z "${NYCT_NETNS:-}" ]; then
  salta_prima() { . "$(dirname "$0")/../lib.sh"; titolo "Sicurezza · browser del PC in modalità 🦇 (cattura dei pacchetti)"; salta "$1"; exit 0; }
  [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ] || salta_prima "nessun display grafico"
  python3 -c 'import gi; gi.require_version("WebKit2","4.0"); from gi.repository import WebKit2' 2>/dev/null || salta_prima "WebKit2GTK non installato"
  command -v unshare >/dev/null && command -v dumpcap >/dev/null && unshare -rn true 2>/dev/null \
    || salta_prima "servono unshare, dumpcap e gli spazi di rete utente"
  exec unshare -rn env NYCT_NETNS=1 bash "$0" "$@"
fi
. "$(dirname "$0")/../lib.sh"
titolo "Sicurezza · browser del PC in modalità 🦇 (cattura dei pacchetti)"
ip link set lo up; ip addr add 10.99.0.1/32 dev lo
CAP="$TMP/browser.pcap"
dumpcap -q -P -i lo -w "$CAP" > "$TMP/dumpcap.log" 2>&1 & CATT=$!
sleep 1.5
H="$TMP/home"; mkdir -p "$H/.nyctalus-browser" "$H/.config"
echo '{"stealth": true, "socks_port": 21490, "homepage": "about:blank"}' > "$H/.nyctalus-browser/config.json"
HOME="$H" XDG_CONFIG_HOME="$H/.config" PYTHONPATH="$ROOT/pc/browser" timeout 90 python3 "$STRUMENTI/prova_browser_pc.py" 21490 > "$TMP/r.txt" 2>"$TMP/e.txt" \
  || echo "KO prova interrotta: $(grep -v -i warning "$TMP/e.txt" | tail -2)" >> "$TMP/r.txt"
kill "$CATT" 2>/dev/null; wait "$CATT" 2>/dev/null
[ -n "${NYCT_CATTURA:-}" ] && cp "$CAP" "$NYCT_CATTURA"
python3 - "$CAP" >> "$TMP/r.txt" <<'PY'
import struct, sys
f = open(sys.argv[1], "rb"); f.read(24); dns = 0; tot = 0
while True:
    h = f.read(16)
    if len(h) < 16: break
    n = struct.unpack("<IIII", h)[2]; d = f.read(n); tot += 1
    if len(d) > 38 and d[12:14] == b"\x08\x00":
        o = 14 + (d[14] & 15) * 4
        if d[23] in (6, 17) and 53 in struct.unpack("!HH", d[o:o + 4]): dns += 1
print(("OK " if dns == 0 and tot > 0 else "KO ") + "cattura: %d pacchetti, %d di DNS (deve essere 0)" % (tot, dns))
PY
while read -r esito resto; do
  case "$esito" in OK) pass "$resto" ;; KO) fail "$resto" ;; esac
done < "$TMP/r.txt"
[ "$KO" -eq 0 ]
