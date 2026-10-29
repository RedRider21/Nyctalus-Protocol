#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Crea la cartella installazioni/: per ogni dispositivo il pacchetto completo da
# copiare, scompattare e installare, con le istruzioni. È separata dallo
# sviluppo e si rigenera da zero ogni volta.
#
#   installazioni/
#   ├── LEGGIMI.txt                    quale pacchetto per quale dispositivo
#   ├── Nyctalus-completo.zip          TUTTO (PC + Android + manuali), da copiare in un colpo
#   ├── PC-Linux/
#   │   ├── Nyctalus-PC-Linux.zip       scompatta → ./installa.sh
#   │   └── LEGGIMI.txt
#   ├── Android/
#   │   ├── nyctalus-termux.tar.gz      per Termux (si apre senza installare nulla)
#   │   ├── nyctalus-termux.zip         lo stesso, in zip
#   │   ├── Nyctalus.apk                l'app (se è già stata compilata)
#   │   └── LEGGIMI.txt
#   └── Manuali/                       installazione, PC, telefono, chat, prove, deploy, sicurezza
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/installazioni"
LAVORO="$(mktemp -d /tmp/nyctalus-installazioni.XXXXXX)"
trap 'rm -rf "$LAVORO"' EXIT

say() { printf '\033[1;36m» %s\033[0m\n' "$*"; }

# zip che conserva i permessi Unix (gli script restano eseguibili dopo lo scompattamento)
crea_zip() { # crea_zip CARTELLA FILE.zip   (dentro lo zip la cartella stessa è la radice)
  python3 - "$1" "$2" <<'PY'
import os, sys, zipfile
src, dst = os.path.abspath(sys.argv[1]), sys.argv[2]
base = os.path.dirname(src)
with zipfile.ZipFile(dst, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as z:
    for radice, cartelle, files in os.walk(src):
        cartelle.sort()
        for nome in sorted(files):
            p = os.path.join(radice, nome)
            info = zipfile.ZipInfo.from_file(p, os.path.relpath(p, base))   # porta con sé i permessi
            info.compress_type = zipfile.ZIP_DEFLATED
            with open(p, "rb") as f:
                z.writestr(info, f.read(), compresslevel=9)
PY
}
manuali_in() { # copia i manuali in una cartella
  mkdir -p "$1"
  cp "$ROOT"/documenti/manuali/*.md "$ROOT/documenti/DEPLOY.md" "$ROOT/documenti/SICUREZZA.md" "$1/"
}

say "Compilo il motore per il PC (release)…"
( cd "$ROOT" && cargo build --release -p nyctalus )

rm -rf "$OUT"
mkdir -p "$OUT/PC-Linux" "$OUT/Android"
manuali_in "$OUT/Manuali"

# ============================== PC Linux ==============================
say "PC-Linux: assemblo il pacchetto"
P="$LAVORO/Nyctalus"
mkdir -p "$P/bin" "$P/browser" "$P/launcher" "$P/chat"
cp "$ROOT/target/release/nyctalus" "$P/bin/nyctalus"
cp -r "$ROOT/pc/browser/nyctalus_browser" "$ROOT/pc/browser/nxs_i18n" "$P/browser/"
cp -r "$ROOT/pc/launcher/nyctalus_launcher" "$P/launcher/"
cp -r "$ROOT/pc/chat/nyctalus_chat" "$P/chat/"
cp "$ROOT/strumenti/avvia-chat.sh" "$ROOT/pc/installa.sh" "$ROOT/pc/disinstalla.sh" "$P/"
cp "$ROOT/logo.svg" "$ROOT/LICENSE" "$P/"
manuali_in "$P/Manuali"
find "$P" -name __pycache__ -type d -prune -exec rm -rf {} +

for prog in launcher browser; do
  modulo="nyctalus_$prog"; extra=""; [ "$prog" = launcher ] && extra='$QUI/launcher:'
  cat > "$P/avvia-$prog.sh" <<EOF
#!/usr/bin/env bash
# Avvia il $prog di Nyctalus (funziona anche lanciato dal menu o da un collegamento).
set -u
QUI="\$(cd "\$(dirname "\$(readlink -f "\$0")")" && pwd)"
export PATH="\$QUI/bin:\$PATH"
export PYTHONPATH="${extra}\$QUI/browser\${PYTHONPATH:+:\$PYTHONPATH}"
exec python3 -m $modulo "\$@"
EOF
done
chmod +x "$P"/*.sh "$P/bin/nyctalus"

cat > "$P/LEGGIMI.txt" <<'EOF'
Nyctalus per PC Linux
=====================

⚠️  Prototipo non ancora verificato da revisori indipendenti: usalo per provare,
    non per proteggere dati reali.

INSTALLARE
   ./installa.sh                    controlla i pacchetti di sistema (chiede prima
                                    di installare), copia il programma in
                                    ~/.local/share/nyctalus, crea le voci nel menu
                                    delle applicazioni e i comandi nyctalus-*
   ./installa.sh --avvio-automatico  in più, il launcher parte all'accesso

USARE
   Dal menu delle applicazioni: «Nyctalus», «Nyctalus Browser», «Nyctalus Chat»
   oppure:  nyctalus-launcher · nyctalus-browser · nyctalus-chat
   Senza installare: ./avvia-launcher.sh · ./avvia-browser.sh · ./avvia-chat.sh

DISINSTALLARE
   ~/.local/share/nyctalus/disinstalla.sh   (toglie solo Nyctalus; i dati solo se confermi)

Manuali nella cartella Manuali/ (inizia da LEGGIMI.md e MANUALE-INSTALLAZIONE.md).
EOF
crea_zip "$P" "$OUT/PC-Linux/Nyctalus-PC-Linux.zip"
cp "$P/LEGGIMI.txt" "$OUT/PC-Linux/LEGGIMI.txt"

# ============================== Android ==============================
say "Android: assemblo il pacchetto per Termux"
bash "$ROOT/strumenti/pacchetto-termux.sh" "$LAVORO"
A="$LAVORO/nyctalus-termux"
cp "$LAVORO/nyctalus-termux.tar.gz" "$OUT/Android/"
crea_zip "$A" "$OUT/Android/nyctalus-termux.zip"

# l'app: si compila se c'è un Android SDK (porta con sé lo stesso pacchetto Termux)
if [ -d "${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}/build-tools" ]; then
  say "Android: compilo l'app (APK)"
  NYCTALUS_PACCHETTO="$LAVORO/nyctalus-termux.tar.gz" bash "$ROOT/strumenti/compila-apk.sh"
fi
APK=""
for c in "$ROOT/mobile/android/Nyctalus.apk" \
         "$ROOT/mobile/android/app/build/outputs/apk/release/app-release.apk" \
         "$ROOT/mobile/android/app/build/outputs/apk/debug/app-debug.apk"; do
  [ -f "$c" ] && { cp "$c" "$OUT/Android/Nyctalus.apk"; APK=1; break; }
done
MOTORE_PRONTO="no"; [ -d "$A/bin" ] && MOTORE_PRONTO="sì"

cat > "$OUT/Android/LEGGIMI.txt" <<EOF
Nyctalus per Android (telefono)
===============================

⚠️  Prototipo: per provare funzionamento e usabilità, NON per dati reali.

Funziona come Termux-NexusSEC-OS: dentro TERMUX gira il motore con un piccolo
servizio, l'interfaccia è una web-app in stile Telegram (nel browser del
telefono o nell'app Nyctalus). Convive con gli altri progetti in Termux.

COSA SCARICARE
  1. Termux — NON dal Play Store:  https://f-droid.org/packages/com.termux/
     oppure https://github.com/termux/termux-app/releases  (file …arm64-v8a.apk)
  2. nyctalus-termux.tar.gz  (o .zip: è lo stesso; il .tar.gz si apre senza
     installare nulla, lo .zip richiede «pkg install unzip»)
$( [ -n "$APK" ] && echo "  3. Nyctalus.apk — l'app (consigliata: contiene già tutto il necessario)" || echo "  3. Nyctalus.apk — non ancora compilata: per ora si usa il browser del telefono" )

INSTALLARE CON L'APP (il modo più semplice)
  1) Installa Termux, aprilo una volta e aspetta che finisca di prepararsi.
  2) Installa Nyctalus.apk e aprila: concedi «Esegui comandi in Termux».
  3) Tocca «🦇 Installa / aggiorna Nyctalus in Termux». L'installazione si vede in
     Termux; alla fine si torna nell'app da sola. Solo la PRIMA volta Termux può
     non partire da solo: tieni premuto in Termux → Incolla → Invio (il comando è
     già negli appunti).
  Nell'app ci sono anche: Installa Termux, Aggiorna Termux, Apri Termux,
  Comando di avvio. Gli aggiornamenti da GitHub arriveranno più avanti.

INSTALLARE A MANO (senza app, si usa il browser)
  a) Copia nyctalus-termux.tar.gz nella cartella Download del telefono
     (cavo USB, oppure mandalo a te stesso e salvalo in Download).
  b) In Termux:
       termux-setup-storage
       cd ~
       tar xzf ~/storage/downloads/nyctalus-termux.tar.gz
       bash nyctalus-termux/installa.sh
     Motore già compilato nell'archivio: $MOTORE_PRONTO
     (se «no», la prima volta lo compila sul telefono: ~300 MB e 10-30 minuti)

USARE
  nyctalus-mobile   → apri nel browser l'indirizzo http://127.0.0.1:18731/?t=… che stampa
  nyctalus-mobile --stato · --ferma
  Disinstallare: bash ~/nyctalus/disinstalla.sh   (toglie solo Nyctalus)

Manuali: cartella Manuali/ (MANUALE-INSTALLAZIONE.md, MANUALE-TELEFONO.md).
EOF

cat > "$OUT/LEGGIMI.txt" <<'EOF'
Nyctalus — cosa installare e dove
=================================

  Dispositivo           Cartella     Cosa prendere                  Poi
  --------------------  -----------  -----------------------------  ----------------------------
  PC Linux (x86_64)     PC-Linux/    Nyctalus-PC-Linux.zip          scompatta → ./installa.sh
  Telefono Android      Android/     nyctalus-termux.tar.gz (+APK)  in Termux → installa.sh
  Tutto in un colpo     —            Nyctalus-completo.zip          contiene tutte le cartelle

Istruzioni passo-passo: il LEGGIMI.txt di ogni cartella e Manuali/MANUALE-INSTALLAZIONE.md.
Windows e macOS: per ora solo il motore a riga di comando; le interfacce arriveranno.

⚠️  Prototipo non ancora verificato da revisori indipendenti: usalo per provare,
    non per proteggere dati reali.
EOF

say "Archivio completo"
mkdir -p "$LAVORO/Nyctalus-completo"
cp -r "$OUT/LEGGIMI.txt" "$OUT/PC-Linux" "$OUT/Android" "$OUT/Manuali" "$LAVORO/Nyctalus-completo/"
crea_zip "$LAVORO/Nyctalus-completo" "$OUT/Nyctalus-completo.zip"

echo
say "Pronto: $OUT"
( cd "$OUT" && find . -type f | sort | sed 's#^\./#   #' )
