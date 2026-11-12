#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# CASSAFORTE, FILE RICEVUTI NELLE APP: con la protezione accesa i file ricevuti sono cifrati sul
# disco, eppure l'app del telefono li mostra (file intero, a pezzi con Range, miniatura) e la
# chat del PC li apre: passano dal comando APRI del motore. Le miniature NON finiscono in
# chiaro sul disco, e alla chiusura la cartella temporanea delle copie in chiaro viene svuotata.
# HOME finta con dati di prova (mai i dati veri).
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · cassaforte: file ricevuti nelle app (telefono e PC)"
assicura_binario
python3 -c "import PIL" 2>/dev/null || { salta "Pillow non installato"; exit 0; }
export XDG_RUNTIME_DIR="$TMP/run"; mkdir -p "$XDG_RUNTIME_DIR"; chmod 700 "$XDG_RUNTIME_DIR"
H="$TMP/home"; BOB=mdeq6zcq5g2xwl4swp6n2wmve6clwolpffok32pojjostzjdec7lsaa.nyct
mkdir -p "$H/.nyctalus-mobile" "$H/.nyctalus-chat/allegati/$BOB"
TOK=segreto0123456789abcdef0123456789; printf %s "$TOK" > "$H/.nyctalus-mobile/token"
cat > "$H/.nyctalus-mobile/config.json" <<'EOF'
{"porte": {"http": 19933, "controllo": 19791, "backend": 19782, "socks": 19780, "socks_locale": 19786,
           "monitor": 19796, "nodi": [19740, 19741, 19742, 19743]}}
EOF
hx() { printf %s "$1" | xxd -p | tr -d '\n'; }
printf '%s %s\n' "$BOB" "$(hx Bob)" > "$H/.nyctalus-chat/contatti"
FOTO="$H/.nyctalus-chat/allegati/$BOB/foto.jpg"; DOC="$H/.nyctalus-chat/allegati/$BOB/documento.bin"
python3 - "$FOTO" <<'EOF'
import os, sys
from PIL import Image
Image.frombytes("RGB", (1600, 1200), os.urandom(1600 * 1200 * 3)).save(sys.argv[1], "JPEG", quality=92)
EOF
head -c 700000 /dev/urandom > "$DOC"
cp "$FOTO" "$TMP/foto.jpg"; cp "$DOC" "$TMP/documento.bin"
PP="passphrase del telefono"

# la cassaforte si accende con il motore: i file già ricevuti vengono cifrati
NYCTALUS_CHAT_TOKEN=preparazione0123456789 "$BIN" chat --data "$H/.nyctalus-chat" --backend 127.0.0.1:19850 --control 127.0.0.1:19851 --socks 127.0.0.1:19852 > "$TMP/prep.log" 2>&1 & PREP=$!; PIDS+=($PREP)
attendi_regex "$TMP/prep.log" '[a-z2-7]{56}\.nyct' 10 > /dev/null
NYCT_FINO="^(CIFRATA|ERR)" controllo 19851 preparazione0123456789 30 "CIFRA $(hx "$PP")" | grep -q "^CIFRATA" && kill $PREP && wait $PREP 2>/dev/null
[ "$(head -c 5 "$FOTO")" = NYCS1 ] && [ "$(head -c 5 "$DOC")" = NYCS1 ] && pass "foto e documento ricevuti: cifrati sul disco (prova)" || fail "file di prova non cifrati"

HOME="$H" NYCTALUS_BIN="$BIN" python3 "$ROOT/mobile/orchestratore/nyctalus_mobile.py" > "$TMP/o.log" 2>&1 & ORCH=$!; PIDS+=($ORCH)
[ -n "$(attendi_regex "$TMP/o.log" 'servizi pronti' 40)" ] || fail "servizi: $(tail -3 "$TMP/o.log")"
U="http://127.0.0.1:19933"; C=(-s -b "nyct=$TOK")
# a cassaforte CHIUSA il file non si può aprire
[ "$(curl "${C[@]}" -o /dev/null -w '%{http_code}' "$U/api/file?p=$(hx "$FOTO")")" = 404 ] \
  && pass "a cassaforte chiusa l'app non riesce ad aprire il file (404)" || fail "file servito a cassaforte chiusa"
read -r _P SM < "$H/.nyctalus-chat/controllo"
NYCT_FINO="^(APERTA|SBLOCCATA|OK|ERR|NO)" controllo 19791 "$SM" 30 "SBLOCCA $(hx "$PP")" > "$TMP/sblocco.txt"
for _ in $(seq 1 100); do curl "${C[@]}" "$U/api/stato" | grep -q '\.nyct' && break; sleep 0.3; done

curl "${C[@]}" -o "$TMP/s-foto.jpg" "$U/api/file?p=$(hx "$FOTO")"; curl "${C[@]}" -o "$TMP/s-doc.bin" "$U/api/file?p=$(hx "$DOC")"
cmp -s "$TMP/s-foto.jpg" "$TMP/foto.jpg" && cmp -s "$TMP/s-doc.bin" "$TMP/documento.bin" \
  && pass "app del telefono: foto e documento serviti in chiaro, identici agli originali" || fail "file serviti diversi (sblocco: $(head -2 "$TMP/sblocco.txt" | tr '\n' ' '))"
curl "${C[@]}" -H "Range: bytes=300000-300999" -o "$TMP/pezzo.bin" "$U/api/file?p=$(hx "$DOC")"
cmp -s "$TMP/pezzo.bin" <(tail -c +300001 "$TMP/documento.bin" | head -c 1000) && pass "anche a pezzi (Range, come per scorrere un video)" || fail "Range sbagliato"
curl "${C[@]}" -o "$TMP/mini.jpg" "$U/api/miniatura?p=$(hx "$FOTO")"
python3 -c "
import sys
from PIL import Image
im = Image.open(sys.argv[1]); sys.exit(0 if im.format == 'JPEG' and max(im.size) <= 400 else 1)" "$TMP/mini.jpg" 2>/dev/null \
  && pass "miniatura della foto cifrata: generata" || fail "miniatura non valida"
[ -z "$(find "$H/.nyctalus-mobile" "$H/.nyctalus-chat" -name '*.jpg' ! -path "*/allegati/*" 2>/dev/null)" ] \
  && [ -n "$(find "$XDG_RUNTIME_DIR" -name '*.jpg' | head -1)" ] && [ "$(head -c 5 "$FOTO")" = NYCS1 ] \
  && pass "miniatura e copia in chiaro solo nella cartella temporanea: sul disco resta tutto cifrato" || fail "tracce in chiaro: $(find "$H" -name '*.jpg' ! -path '*/allegati/*' | head -3)"
curl "${C[@]}" "$U/api/archivio" | grep -q '"nome": "foto.jpg"' && pass "archivio dei file ricevuti: elencati come sempre" || fail "archivio"

# chat e File del PC: la stessa strada (APRI) dal loro modulo
R=$(HOME="$H" NYCTALUS_CHAT_PORT=19791 NYCTALUS_CHAT_TOKEN="$SM" PYTHONPATH="$ROOT/pc/chat" python3 -c "
import sys
from nyctalus_chat import media
c = media.in_chiaro(sys.argv[1]); print(c)
print('DIM', media.dimensione_logica(c) is not None)" "$FOTO" 2>/dev/null)
CP=$(echo "$R" | head -1)
[ "$CP" != "$FOTO" ] && cmp -s "$CP" "$TMP/foto.jpg" && echo "$R" | grep -q "DIM True" \
  && pass "chat del PC: la foto cifrata si apre e si vede in anteprima (copia temporanea)" || fail "chat PC: $R"
R=$(HOME="$H" NYCTALUS_CHAT_PORT=19791 NYCTALUS_CHAT_TOKEN="$SM" PYTHONPATH="$ROOT/pc/file" python3 -c "
import sys
from nyctalus_file import app
print(app.in_chiaro(sys.argv[1]))" "$DOC" 2>/dev/null | tail -1)
[ "$R" != "$DOC" ] && cmp -s "$R" "$TMP/documento.bin" && pass "programma File del PC: il file cifrato si apre" || fail "File PC: $R"

# alla chiusura la cartella temporanea si svuota
kill -TERM $ORCH 2>/dev/null; for _ in $(seq 1 40); do kill -0 $ORCH 2>/dev/null || break; sleep 0.25; done; sleep 1
[ -z "$(find "$XDG_RUNTIME_DIR" -type f 2>/dev/null | head -1)" ] && pass "alla chiusura le copie in chiaro spariscono" || fail "copie in chiaro rimaste: $(find "$XDG_RUNTIME_DIR" -type f | head -3)"
[ "$KO" -eq 0 ]
