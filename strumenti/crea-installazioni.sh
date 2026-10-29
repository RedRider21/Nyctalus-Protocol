#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Crea la cartella installazioni/: per ogni dispositivo, il pacchetto da
# installare e le istruzioni. È separata dallo sviluppo (motore/, pc/, mobile/…)
# e si rigenera da zero ogni volta.
#
#   installazioni/
#   ├── LEGGIMI.txt                  quale pacchetto per quale dispositivo
#   ├── PC-Linux/
#   │   ├── Nyctalus-PC-Linux.tar.gz  motore + browser + chat + launcher
#   │   └── LEGGIMI.txt
#   └── Android/
#       ├── nyctalus-termux.tar.gz    da installare dentro Termux
#       ├── Nyctalus.apk              l'app (se è già stata compilata)
#       └── LEGGIMI.txt               passo-passo
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/installazioni"
LAVORO="$(mktemp -d /tmp/nyctalus-installazioni.XXXXXX)"
trap 'rm -rf "$LAVORO"' EXIT

say() { printf '\033[1;36m» %s\033[0m\n' "$*"; }

say "Compilo il motore per il PC (release)…"
( cd "$ROOT" && cargo build --release -p nyctalus )

rm -rf "$OUT"
mkdir -p "$OUT/PC-Linux" "$OUT/Android"

# ============================== PC Linux ==============================
say "PC-Linux: assemblo il pacchetto"
P="$LAVORO/Nyctalus"
mkdir -p "$P/bin" "$P/browser" "$P/launcher" "$P/chat"
cp "$ROOT/target/release/nyctalus" "$P/bin/nyctalus"
cp -r "$ROOT/pc/browser/nyctalus_browser" "$ROOT/pc/browser/nxs_i18n" "$P/browser/"
cp -r "$ROOT/pc/launcher/nyctalus_launcher" "$P/launcher/"
cp -r "$ROOT/pc/chat/nyctalus_chat" "$P/chat/"
cp "$ROOT/strumenti/avvia-chat.sh" "$P/avvia-chat.sh"
cp "$ROOT/logo.svg" "$ROOT/LICENSE" "$P/"
cp "$ROOT/documenti/DEPLOY.md" "$ROOT/documenti/SICUREZZA.md" "$ROOT/documenti/MANUALE-CHAT.md" "$P/"
find "$P" -name __pycache__ -type d -prune -exec rm -rf {} +

cat > "$P/avvia-launcher.sh" <<'EOF'
#!/usr/bin/env bash
# Avvia il mini-launcher di Nyctalus (il pannello da cui parte tutto).
set -u
QUI="$(cd "$(dirname "$0")" && pwd)"
export PATH="$QUI/bin:$PATH"
export PYTHONPATH="$QUI/launcher:$QUI/browser${PYTHONPATH:+:$PYTHONPATH}"
exec python3 -m nyctalus_launcher "$@"
EOF
cat > "$P/avvia-browser.sh" <<'EOF'
#!/usr/bin/env bash
# Avvia solo il browser di Nyctalus (lo stealth avvia da sé i servizi).
set -u
QUI="$(cd "$(dirname "$0")" && pwd)"
export PATH="$QUI/bin:$PATH"
export PYTHONPATH="$QUI/browser${PYTHONPATH:+:$PYTHONPATH}"
exec python3 -m nyctalus_browser "$@"
EOF
chmod +x "$P/avvia-launcher.sh" "$P/avvia-browser.sh" "$P/avvia-chat.sh"
( cd "$LAVORO" && tar czf "$OUT/PC-Linux/Nyctalus-PC-Linux.tar.gz" Nyctalus )

cat > "$OUT/PC-Linux/LEGGIMI.txt" <<'EOF'
Nyctalus per PC Linux (Debian/Ubuntu/Mint…, x86_64)
==================================================

⚠️  Prototipo non ancora verificato da revisori indipendenti: usalo per provare
    funzionamento, usabilità e stabilità, NON per proteggere dati reali.

1) Pacchetti di sistema
     sudo apt install python3-gi gir1.2-gtk-3.0 gir1.2-webkit2-4.1
   facoltativi:
     sudo apt install gir1.2-keybinder-3.0                        # tasti rapidi globali
     sudo apt install gir1.2-gstreamer-1.0 gstreamer1.0-gtk3 \
          gstreamer1.0-plugins-good gstreamer1.0-plugins-ugly \
          gstreamer1.0-libav ffmpeg                               # fotocamera, audio, miniature

2) Estrai l'archivio dove vuoi (es. nella tua home)
     tar xzf Nyctalus-PC-Linux.tar.gz
     cd Nyctalus

3) Avvia
     ./avvia-launcher.sh     il pannello: logo (guida), Browser, Chat
     ./avvia-browser.sh      solo il browser
     ./avvia-chat.sh         solo la chat

Guide incluse: MANUALE-CHAT.md (la chat), DEPLOY.md (rete condivisa e
opzioni), SICUREZZA.md (come verificare).
EOF

# ============================== Android ==============================
say "Android: assemblo il pacchetto per Termux"
A="$LAVORO/nyctalus-termux"
mkdir -p "$A/sorgenti"
cp -r "$ROOT/mobile/orchestratore" "$ROOT/mobile/webapp" "$A/"
cp "$ROOT/mobile/termux/installa.sh" "$ROOT/mobile/termux/disinstalla.sh" "$ROOT/mobile/termux/nyctalus-mobile" "$A/"
# sorgenti del motore: in Termux si compila (se non c'è già un binario per telefono)
cp "$ROOT/Cargo.toml" "$ROOT/Cargo.lock" "$A/sorgenti/"
cp -r "$ROOT/motore" "$A/sorgenti/"
# il profilo di sviluppo del PC (disco quasi pieno) non serve sul telefono
find "$A" -name __pycache__ -type d -prune -exec rm -rf {} +
for t in aarch64-linux-android aarch64-unknown-linux-musl; do
  if [ -x "$ROOT/target/$t/release/nyctalus" ]; then
    mkdir -p "$A/bin"; cp "$ROOT/target/$t/release/nyctalus" "$A/bin/nyctalus"
    say "Android: incluso il motore precompilato ($t)"
    break
  fi
done
chmod +x "$A/installa.sh" "$A/disinstalla.sh" "$A/nyctalus-mobile"
cp "$ROOT/LICENSE" "$A/"
( cd "$LAVORO" && tar czf "$OUT/Android/nyctalus-termux.tar.gz" nyctalus-termux )

APK=""
for c in "$ROOT/mobile/android/app/build/outputs/apk/release/app-release.apk" \
         "$ROOT/mobile/android/app/build/outputs/apk/debug/app-debug.apk" \
         "$ROOT/mobile/android/Nyctalus.apk"; do
  [ -f "$c" ] && { cp "$c" "$OUT/Android/Nyctalus.apk"; APK=1; break; }
done

MOTORE_PRONTO="no"; [ -d "$A/bin" ] && MOTORE_PRONTO="sì"
cat > "$OUT/Android/LEGGIMI.txt" <<EOF
Nyctalus per Android (telefono)
===============================

⚠️  Prototipo: per provare funzionamento e usabilità, NON per dati reali.

Nyctalus sul telefono funziona come Termux-NexusSEC-OS: dentro TERMUX gira il
motore (lo stesso del PC) con un piccolo servizio, e l'interfaccia è una
web-app in stile Telegram (nel browser del telefono, o nell'app Nyctalus).
Convive con gli altri progetti in Termux: usa solo ~/nyctalus, porte 18xxx,
comandi "nyctalus-…", e non aggiorna i pacchetti di sistema.

COSA SCARICARE
  1. Termux — NON dal Play Store (versione vecchia):
       F-Droid:  https://f-droid.org/packages/com.termux/
       oppure GitHub: https://github.com/termux/termux-app/releases
       → file "termux-app_…+github-debug_arm64-v8a.apk" (quasi tutti i
         telefoni recenti sono arm64-v8a)
     Se usi già Termux per altri progetti, va bene quello che hai.
  2. Questo archivio: nyctalus-termux.tar.gz
$( [ -n "$APK" ] && echo "  3. L'app: Nyctalus.apk (facoltativa: senza, usi il browser del telefono)" \
               || echo "  3. L'app Nyctalus.apk: NON ancora compilata (arriverà con la compilazione
     automatica su GitHub). Nel frattempo si usa il browser del telefono." )

INSTALLAZIONE (una volta sola)
  a) Copia nyctalus-termux.tar.gz nella cartella Download del telefono.
  b) In Termux:
       termux-setup-storage            (consenti l'accesso ai file)
       cd ~
       tar xzf ~/storage/downloads/nyctalus-termux.tar.gz
       bash nyctalus-termux/installa.sh
     Motore già compilato nell'archivio: $MOTORE_PRONTO
     (se "no", l'installatore lo compila sul telefono: scarica Rust
      ~300 MB e impiega 10-30 minuti, solo la prima volta)

USO
  In Termux:   nyctalus-mobile
  Stampa un indirizzo tipo  http://127.0.0.1:18731/?t=…  → aprilo nel browser
  del telefono (Chrome): la prima volta il browser ricorda l'accesso.
  Dal menu del browser puoi "Aggiungere a schermata Home" → icona Nyctalus.
  Con l'app Nyctalus.apk non serve copiare indirizzi: fa tutto da sola.

  nyctalus-mobile --stato   dice se è attivo e l'indirizzo da aprire
  nyctalus-mobile --ferma   ferma tutto
  Disinstallare:  bash ~/nyctalus/disinstalla.sh   (toglie solo Nyctalus)

NOTA SULLA RETE
  Senza configurazione parte una rete di prova LOCALE (solo sul telefono): la
  chat si apre, ma per parlare con altre persone serve la rete condivisa
  (vedi DEPLOY.md, ~/.nyctalus-mobile/config.json: directory_url + authority).
EOF

cat > "$OUT/LEGGIMI.txt" <<'EOF'
Nyctalus — cosa installare e dove
=================================

  Dispositivo                 Cartella      Cosa contiene
  --------------------------  ------------  -----------------------------------------
  PC Linux (x86_64)           PC-Linux/     Nyctalus-PC-Linux.tar.gz + LEGGIMI.txt
                                            motore + browser + chat + launcher
  Telefono Android            Android/      nyctalus-termux.tar.gz (dentro Termux)
                                            Nyctalus.apk (l'app, quando disponibile)
                                            + LEGGIMI.txt passo-passo

In ogni cartella il LEGGIMI.txt spiega i passi. Windows e macOS: per ora solo il
motore a riga di comando (compilazione automatica su GitHub); le interfacce
grafiche arriveranno.

⚠️  Prototipo non ancora verificato da revisori indipendenti: usalo per provare,
    non per proteggere dati reali.
EOF

echo
say "Pronto: $OUT"
( cd "$OUT" && find . -type f | sort | sed 's#^\./#   #' )
