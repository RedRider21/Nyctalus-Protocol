#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# «🏠 LA MIA RETE» nel mini menu del PC: le scelte a spunta diventano lo stesso comando del
# terminale (nyctalus-rete casa …), che la finestra mostra ed esegue. Qui la parte senza grafica
# (HOME finta): comando costruito per ogni scelta, nomi e numeri sbagliati rifiutati, rete accesa
# dalla finestra (staccata dal mini menu) con i nodi senza uscita, spegnimento, riaccensione
# che spegne prima la precedente.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · finestra «La mia rete» (mini menu del PC)"
assicura_binario
python3 -c "import gi; gi.require_version('Gtk', '3.0'); from gi.repository import Gtk" 2>/dev/null || { salta "GTK 3 per Python non installato"; exit 0; }
H="$TMP/casa"; mkdir -p "$H"
export HOME="$H" NYCTALUS_BIN="$BIN" NYCTALUS_RUBRICA_PORTE=24830,24831,24832,24833,24834 NYCTALUS_UNITA_CASA=nyctalus-casa-prova-53.service
PY() { PYTHONPATH="$ROOT/pc/launcher:$ROOT/pc/browser" python3 - "$@"; }

PY <<'EOF' > "$TMP/cmd.txt" 2>&1
import nyctalus_launcher.app as l
c = l.comando_casa
try:
    c({"nome": ""}); print("ERR nome vuoto accettato")
except ValueError as e:
    assert str(e) == "nome"
print(l.comando_leggibile(c({"nome": "miacasa.duckdns.org"})))
print(l.comando_leggibile(c({"nome": "87.7.1.2", "senza_uscita": False, "sempre": False})))
print(l.comando_leggibile(c({"internet": False, "nodi": 5, "base": 5001, "porta": 5100, "sempre": False})))
for cattivo in ({"nome": "a;rm -rf ~"}, {"internet": False, "nodi": 2}, {"internet": False, "base": 18799}, {"internet": False, "porta": 80}):
    try:
        c(cattivo); print("ERR accettato", cattivo)
    except ValueError:
        pass
EOF
grep -q ERR "$TMP/cmd.txt" && fail "valori sbagliati accettati: $(grep ERR "$TMP/cmd.txt")"
L1="nyctalus-rete casa --internet miacasa.duckdns.org --senza-uscita --servizio"
L2="nyctalus-rete casa --internet 87.7.1.2 --con-uscita"
L3="nyctalus-rete casa --nodi 5 --porta-base 5001 --porta 5100 --senza-uscita"
[ "$(sed -n 1p "$TMP/cmd.txt")" = "$L1" ] && [ "$(sed -n 2p "$TMP/cmd.txt")" = "$L2" ] && [ "$(sed -n 3p "$TMP/cmd.txt")" = "$L3" ] \
  && pass "le spunte diventano il comando del terminale (Internet, senza uscita, sempre accesa, nodi e porte)" || fail "comandi: $(cat "$TMP/cmd.txt")"
! grep -q ERR "$TMP/cmd.txt" && grep -c . "$TMP/cmd.txt" | grep -qx 3 \
  && pass "nome con caratteri pericolosi, troppo pochi nodi, porte sovrapposte o basse: rifiutati" || fail "controlli: $(cat "$TMP/cmd.txt")"

# accensione dalla finestra (non come servizio): processo staccato, nodi senza uscita
PY <<'EOF' > "$TMP/acc.txt" 2>&1
import nyctalus_launcher.app as l
rete = l.RADICE / "gestore-rete" / "nyctalus-rete"
args = l.comando_casa({"internet": False, "base": 24821, "porta": 24820, "sempre": False}) + ["--ip", "127.0.0.1", "--non-importare"]
ok, testo = l.accendi_casa(rete, args, attesa=240)
print("ESITO", ok, l.pid_finestra() is not None)
print(testo)
EOF
curl -sf http://127.0.0.1:24820/directory.txt -o "$TMP/dir1.txt"
grep -q "ESITO True True" "$TMP/acc.txt" && grep -q "Rete di casa attiva" "$TMP/acc.txt" && [ "$(grep -c '^v .*uscita=no' "$TMP/dir1.txt")" = 4 ] \
  && pass "«Accendi»: rete di casa accesa, staccata dal mini menu, 4 nodi senza uscita; il testo mostra codice e passi" || fail "accensione: $(head -5 "$TMP/acc.txt")"
P1=$(cat "$H/.nyctalus-rete/finestra.pid" 2>/dev/null)

# riaccendere con altre scelte spegne prima la rete precedente (niente porte occupate)
PY <<'EOF' > "$TMP/acc2.txt" 2>&1
import nyctalus_launcher.app as l
rete = l.RADICE / "gestore-rete" / "nyctalus-rete"
args = l.comando_casa({"internet": False, "base": 24821, "porta": 24820, "sempre": False, "senza_uscita": False}) + ["--ip", "127.0.0.1", "--non-importare"]
ok, testo = l.accendi_casa(rete, args, attesa=240)
print("ESITO", ok)
EOF
curl -sf http://127.0.0.1:24820/directory.txt -o "$TMP/dir2.txt"
P2=$(cat "$H/.nyctalus-rete/finestra.pid" 2>/dev/null)
grep -q "ESITO True" "$TMP/acc2.txt" && [ -n "$P1" ] && ! kill -0 "$P1" 2>/dev/null && [ "$P1" != "$P2" ] && ! grep -q "uscita=no" "$TMP/dir2.txt" \
  && pass "riaccesa con altre scelte (con uscita): la precedente si spegne, la nuova parte sulle stesse porte" || fail "riaccensione: $(tail -3 "$TMP/acc2.txt") · $P1/$P2"

PY <<'EOF' > "$TMP/sp.txt" 2>&1
import nyctalus_launcher.app as l
rete = l.RADICE / "gestore-rete" / "nyctalus-rete"
print(l.spegni_casa(rete)); print("VIVA", l.pid_finestra())
EOF
sleep 1
grep -q "VIVA None" "$TMP/sp.txt" && ! kill -0 "$P2" 2>/dev/null && ! curl -sf -m 3 http://127.0.0.1:24820/directory.txt > /dev/null \
  && ! pgrep -f "listen 127.0.0.1:2482[1-4]" > /dev/null \
  && pass "«Spegni»: rete, pagina e nodi fermati" || fail "spegnimento: $(cat "$TMP/sp.txt")"
grep -q '_mia_rete' "$ROOT/pc/launcher/nyctalus_launcher/app.py" && for l in it en es fr de; do grep -q '"lau.miarete.senza_uscita"' "$ROOT/pc/browser/nxs_i18n/strings/$l.json" || echo "$l"; done | grep -q . \
  && fail "testi della finestra mancanti in qualche lingua" || pass "voce «🏠 La mia rete» nel menu ⚙, testi in 5 lingue"
[ "$KO" -eq 0 ]
