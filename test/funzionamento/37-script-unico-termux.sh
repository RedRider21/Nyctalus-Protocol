#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# LO SCRIPT UNICO per Termux (installa-termux.sh), eseguito come lo incolla l'utente:
#   curl -fsSL …/releases/latest/download/installa-termux.sh | bash
# in un Termux simulato (HOME, PREFIX, pkg/dpkg/am finti) con i file serviti come da una release.
. "$(dirname "$0")/../lib.sh"
titolo "Funzionamento · script unico per Termux (curl | bash)"
assicura_binario
W="$TMP/release"; PK="$TMP/pk"; mkdir -p "$W" "$PK/nyctalus-termux/bin"
cp -r "$ROOT/mobile/orchestratore" "$ROOT/mobile/webapp" "$PK/nyctalus-termux/"
cp "$ROOT/mobile/termux/installa.sh" "$ROOT/mobile/termux/disinstalla.sh" "$ROOT/mobile/termux/nyctalus-mobile" "$PK/nyctalus-termux/"
cp "$BIN" "$PK/nyctalus-termux/bin/nyctalus"
( cd "$PK" && tar czf "$W/nyctalus-termux.tar.gz" nyctalus-termux )
( cd "$W" && sha256sum nyctalus-termux.tar.gz > nyctalus-termux.tar.gz.sha256 )
cp "$ROOT/mobile/termux/installa-termux.sh" "$W/"
python3 -m http.server 19741 --bind 127.0.0.1 --directory "$W" > /dev/null 2>&1 & PIDS+=($!)
H="$TMP/home"; P="$TMP/usr"; F="$TMP/finti"; mkdir -p "$H/.termux" "$P/bin" "$F"
printf 'extra-keys = [[ESC]]\n' > "$H/.termux/termux.properties"
printf '#!/bin/sh\necho "$*" >> "%s/pkg.txt"\n' "$TMP" > "$F/pkg"
printf '#!/bin/sh\n[ "$1" = "-s" ] || exit 1\ncase "$2" in python|termux-tools|python-pillow) exit 0 ;; *) exit 1 ;; esac\n' > "$F/dpkg"
printf '#!/bin/sh\nexec python3 "$@"\n' > "$F/python"
printf '#!/bin/sh\necho "$*" >> "%s/am.txt"\n' "$TMP" > "$F/am"
printf '#!/bin/sh\necho ricaricate >> "%s/reload.txt"\n' "$TMP" > "$F/termux-reload-settings"
chmod +x "$F"/*
sleep 0.5
REPO=http://127.0.0.1:19741
corri() { env HOME="$H" PREFIX="$P" PATH="$F:$PATH" NYCTALUS_REPO="$REPO" bash -c "curl -fsSL $REPO/installa-termux.sh | bash" > "$TMP/$1.log" 2>&1; }

# 1) pacchetto alterato: rifiutato
cp "$W/nyctalus-termux.tar.gz.sha256" "$TMP/sha.giusta"
echo "0000000000000000000000000000000000000000000000000000000000000000  nyctalus-termux.tar.gz" > "$W/nyctalus-termux.tar.gz.sha256"
corri alterato
[ ! -e "$H/nyctalus" ] && grep -q "NON lo installo" "$TMP/alterato.log" && pass "impronta sbagliata: lo script NON installa niente" || fail "pacchetto alterato installato: $(tail -2 "$TMP/alterato.log")"
cp "$TMP/sha.giusta" "$W/nyctalus-termux.tar.gz.sha256"

# 2) installazione vera, in un colpo
corri vero
[ -x "$H/nyctalus/bin/nyctalus" ] && [ -f "$H/nyctalus/webapp/index.html" ] && pass "un solo comando: Nyctalus scaricato, verificato e installato" || fail "installazione: $(tail -4 "$TMP/vero.log")"
grep -q "allow-external-apps = true" "$H/.termux/termux.properties" && grep -q "^extra-keys" "$H/.termux/termux.properties" \
  && [ "$(grep -c allow-external-apps "$H/.termux/termux.properties")" = 1 ] && pass "permesso all'app acceso (una riga sola, il resto intatto)" || fail "termux.properties: $(cat "$H/.termux/termux.properties")"
grep -q ricaricate "$TMP/reload.txt" 2>/dev/null && pass "impostazioni di Termux ricaricate (vale subito)" || fail "termux-reload-settings non chiamato"
grep -q -- "-n org.nyctalus.mobile/.MainActivity" "$TMP/am.txt" 2>/dev/null && pass "alla fine riapre l'app Nyctalus" || fail "app non riaperta"
! grep -q upgrade "$TMP/pkg.txt" 2>/dev/null && pass "Termux già pronto (Python c'è): nessun aggiornamento lanciato" || fail "pkg upgrade inutile"

# 3) di nuovo (aggiornamento): idempotente
corri ancora
[ -x "$H/nyctalus/bin/nyctalus" ] && [ "$(grep -c allow-external-apps "$H/.termux/termux.properties")" = 1 ] && pass "rilanciato: aggiorna senza duplicare niente" || fail "secondo giro: $(tail -3 "$TMP/ancora.log")"
[ "$KO" -eq 0 ]
