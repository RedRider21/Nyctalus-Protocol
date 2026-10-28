#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Assembla la PARTE DA DISTRIBUIRE in una struttura di cartelle stabile, separata
# dallo sviluppo (crates/, target/, …). Produce `pacchetto/Nyctalus/`:
#
#   Nyctalus/
#   ├── bin/nyctalus              il motore (demone, nodi, strumenti)
#   ├── browser/                  il browser (nyctalus_browser + nxs_i18n)
#   ├── launcher/                 il mini-launcher da desktop
#   ├── logo.svg
#   ├── avvia-launcher.sh         ← avvia il launcher (da cui parte tutto)
#   ├── avvia-browser.sh          ← avvia solo il browser
#   ├── LEGGIMI.txt               guida rapida
#   └── DEPLOY.md · SICUREZZA.md · LICENSE
#
# Poi, opzionale, un archivio .tar.gz pronto da inviare ai tester.
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/nyctalus"
OUT="$ROOT/pacchetto/Nyctalus"

echo "» Compilo il motore (release)…"
( cd "$ROOT" && cargo build --release -p nyctalus )

echo "» Assemblo $OUT"
rm -rf "$ROOT/pacchetto"
mkdir -p "$OUT/bin" "$OUT/browser" "$OUT/launcher"

cp "$BIN" "$OUT/bin/nyctalus"
cp -r "$ROOT/browser/nyctalus_browser" "$OUT/browser/"
cp -r "$ROOT/browser/nxs_i18n" "$OUT/browser/"
cp -r "$ROOT/launcher/nyctalus_launcher" "$OUT/launcher/"
cp "$ROOT/logo.svg" "$OUT/logo.svg"
cp "$ROOT/DEPLOY.md" "$ROOT/SICUREZZA.md" "$ROOT/LICENSE" "$OUT/"
# niente cache Python nel pacchetto
find "$OUT" -name __pycache__ -type d -prune -exec rm -rf {} + 2>/dev/null || true

# --- script di avvio, self-contained (usano i percorsi DENTRO il pacchetto) ---
cat > "$OUT/avvia-launcher.sh" <<'EOF'
#!/usr/bin/env bash
# Avvia il mini-launcher di Nyctalus (il pannello da cui parte tutto).
set -u
QUI="$(cd "$(dirname "$0")" && pwd)"
export PATH="$QUI/bin:$PATH"
export PYTHONPATH="$QUI/launcher:$QUI/browser${PYTHONPATH:+:$PYTHONPATH}"
exec python3 -m nyctalus_launcher "$@"
EOF

cat > "$OUT/avvia-browser.sh" <<'EOF'
#!/usr/bin/env bash
# Avvia solo il browser di Nyctalus (lo stealth avvia da sé i servizi).
set -u
QUI="$(cd "$(dirname "$0")" && pwd)"
export PATH="$QUI/bin:$PATH"
export PYTHONPATH="$QUI/browser${PYTHONPATH:+:$PYTHONPATH}"
exec python3 -m nyctalus_browser "$@"
EOF
chmod +x "$OUT/avvia-launcher.sh" "$OUT/avvia-browser.sh"

cat > "$OUT/LEGGIMI.txt" <<'EOF'
Nyctalus — pacchetto da usare
=============================

⚠️  Prototipo non ancora verificato da revisori indipendenti: usalo per provare
    funzionamento, usabilità e stabilità, NON per proteggere dati reali.
    (Vedi SICUREZZA.md.)

Servono:  python3 + PyGObject (GTK3) + WebKit2GTK
  Debian/Ubuntu:  sudo apt install python3-gi gir1.2-gtk-3.0 gir1.2-webkit2-4.1

Avvio:
  ./avvia-launcher.sh     → apre il pannello: logo (sito/guida), Browser, Chat,
                            Gestore file (chat e file arriveranno).
  ./avvia-browser.sh      → apre solo il browser.

Nel browser: parte NORMALE; il bottone "Stealth" accende la rete Nyctalus e
avvia da sé i servizi (una rete di prova locale, oppure quella pubblica se
configurata — vedi DEPLOY.md §4). I siti .nyct accendono lo stealth da soli.

Struttura:
  bin/        il motore (demone, nodi, strumenti): comando `nyctalus`
  browser/    il browser
  launcher/   il mini-launcher
  DEPLOY.md   installare, configurare, mettere in piedi una rete
  SICUREZZA.md  come verificare sicurezza e anonimato
EOF

echo "» Creo l'archivio"
( cd "$ROOT/pacchetto" && tar czf "Nyctalus.tar.gz" "Nyctalus" )

echo
echo "✓ Pronto:  $OUT"
echo "✓ Archivio: $ROOT/pacchetto/Nyctalus.tar.gz"
echo "  Da inviare ai tester: scompattano e lanciano ./avvia-launcher.sh"
