#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Compila l'app Android (mobile/android) SENZA Gradle e senza scaricare nulla:
# bastano un Android SDK già presente (build-tools + una piattaforma) e un JDK.
#   aapt2 (risorse) → javac (codice) → d8 (dex) → zipalign → apksigner
# Risultato: installazioni/Android/Nyctalus.apk (l'unico posto delle APK; con
# NYCTALUS_APK_PROVA=1 installazioni/Android/Nyctalus-prova.apk).
#
# SDK: $ANDROID_HOME o $ANDROID_SDK_ROOT, altrimenti ~/Android/Sdk.
# Firma: chiave di sviluppo in mobile/android/privato/ (fuori da git), creata la
# prima volta; tenerla serve perché gli aggiornamenti si installino sopra.
set -eu
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
AND="$ROOT/mobile/android"
APP="$AND/app/src/main"

SDK=""
for c in "${ANDROID_HOME:-}" "${ANDROID_SDK_ROOT:-}" "$HOME/Android/Sdk"; do
  [ -n "$c" ] && [ -d "$c/build-tools" ] && { SDK="$c"; break; }
done
[ -n "$SDK" ] || { echo "Android SDK non trovato: imposta ANDROID_HOME" >&2; exit 1; }
BT="$SDK/build-tools/$(ls "$SDK/build-tools" | sort -V | tail -1)"
JAR="$SDK/platforms/$(ls "$SDK/platforms" | sort -V | tail -1)/android.jar"
JAVAC=(javac); command -v javac >/dev/null || JAVAC=(java -m jdk.compiler/com.sun.tools.javac.Main)
echo "» SDK: $BT · $(basename "$(dirname "$JAR")")"

L="$(mktemp -d /tmp/nyctalus-apk.XXXXXX)"; trap 'rm -rf "$L"' EXIT
VNAME=$(grep -oE "versionName '[^']+'" "$AND/app/build.gradle" | cut -d"'" -f2)
VCODE=$(grep -oE "versionCode [0-9]+" "$AND/app/build.gradle" | grep -oE '[0-9]+')

# aapt2 vuole il nome del pacchetto nel manifest (con Gradle sta in «namespace»)
sed 's#<manifest xmlns:android="http://schemas.android.com/apk/res/android">#<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="org.nyctalus.mobile">#' \
  "$APP/AndroidManifest.xml" > "$L/AndroidManifest.xml"
# NYCTALUS_APK_PROVA=1: «Nyctalus prova», un'altra app (pacchetto org.nyctalus.mobile.prova) da
# installare ACCANTO a quella vera per provarla; esce come Nyctalus-prova.apk
DOVE="${NYCTALUS_APK_DOVE:-$ROOT/installazioni/Android}"; mkdir -p "$DOVE"   # (le prove: una cartella temporanea)
RES="$APP/res"; RINOMINA=(); USCITA_APK="$DOVE/Nyctalus.apk"
if [ "${NYCTALUS_APK_PROVA:-0}" = 1 ]; then
  sed -i 's#android:authorities="org.nyctalus.mobile.foto"#android:authorities="org.nyctalus.mobile.prova.foto"#' "$L/AndroidManifest.xml"
  cp -r "$APP/res" "$L/res-prova"; RES="$L/res-prova"
  sed -i 's#<string name="app_name">Nyctalus</string>#<string name="app_name">Nyctalus prova</string>#' "$RES/values/strings.xml"
  RINOMINA=(--rename-manifest-package org.nyctalus.mobile.prova); USCITA_APK="$DOVE/Nyctalus-prova.apk"
fi
# l'app porta con sé il pacchetto per Termux (pulsante «Installa Nyctalus in Termux»)
mkdir -p "$L/assets"
# APK «con tutto» (di serie): la web-app negli asset e il motore come libreria (lib/<abi>/
# libnyctalus.so) per i telefoni veri, arm64 e armv7; Termux non serve. NYCTALUS_APK_TERMUX=1: la
# versione di prima, che porta il pacchetto per Termux e avvia lì Nyctalus.
cp -r "$ROOT/mobile/webapp" "$L/assets/webapp"
find "$L/assets/webapp" -name '__pycache__' -prune -exec rm -rf {} + 2>/dev/null || true
if [ "${NYCTALUS_APK_TERMUX:-0}" = 1 ]; then
  if [ -s "${NYCTALUS_PACCHETTO:-}" ]; then cp "$NYCTALUS_PACCHETTO" "$L/assets/nyctalus-termux.tar.gz"
  else bash "$ROOT/strumenti/pacchetto-termux.sh" "$L/pacchetto" >/dev/null; cp "$L/pacchetto/nyctalus-termux.tar.gz" "$L/assets/"; fi
else
  for coppia in arm64-v8a:aarch64-linux-android armeabi-v7a:armv7-linux-androideabi; do
    abi="${coppia%%:*}"; m="$ROOT/target/${coppia#*:}/release/nyctalus"
    if [ ! -s "$m" ]; then
      # NYCTALUS_APK_SOLO_VERIFICA=1 (controllo su GitHub): APK senza motore, solo per vedere che si compila
      [ "${NYCTALUS_APK_SOLO_VERIFICA:-0}" = 1 ] && { echo "» (solo verifica) senza il motore per $abi: questa APK non va installata"; continue; }
      echo "manca il motore per Android $abi ($m): compilalo con il flusso «Eseguibili»" >&2; exit 1
    fi
    mkdir -p "$L/libs/lib/$abi"; cp "$m" "$L/libs/lib/$abi/libnyctalus.so"
  done
fi
"$BT/aapt2" compile --dir "$RES" -o "$L/res.zip"
"$BT/aapt2" link -I "$JAR" --manifest "$L/AndroidManifest.xml" -o "$L/base.apk" --java "$L/gen" ${RINOMINA[@]+"${RINOMINA[@]}"} \
  --min-sdk-version 24 --target-sdk-version 34 --version-code "$VCODE" --version-name "$VNAME" -A "$L/assets" "$L/res.zip"

mkdir -p "$L/classi" "$L/dex"
mapfile -t SORGENTI < <(find "$L/gen" "$APP/java" -name '*.java')     # (il percorso può contenere spazi)
"${JAVAC[@]}" -nowarn -Xlint:-options -source 8 -target 8 -encoding UTF-8 -bootclasspath "$JAR" -classpath "$BT/core-lambda-stubs.jar" -d "$L/classi" "${SORGENTI[@]}"
mapfile -t CLASSI < <(find "$L/classi" -name '*.class')
"$BT/d8" --release --min-api 24 --lib "$JAR" --output "$L/dex" "${CLASSI[@]}"

cp "$L/base.apk" "$L/senza-firma.apk"
( cd "$L/dex" && zip -q -X "$L/senza-firma.apk" classes.dex )
[ -d "$L/libs" ] && ( cd "$L/libs" && zip -q -X -r "$L/senza-firma.apk" lib )
"$BT/zipalign" -p -f 4 "$L/senza-firma.apk" "$L/allineato.apk"

K="$AND/privato/firma-sviluppo.jks"
if [ ! -f "$K" ]; then
  mkdir -p "$AND/privato"; chmod 700 "$AND/privato"
  keytool -genkeypair -keystore "$K" -storepass nyctalus -keypass nyctalus -alias nyctalus \
    -keyalg RSA -keysize 3072 -validity 10000 -dname "CN=Nyctalus sviluppo, O=RedRider21" >/dev/null 2>&1
  echo "» creata la chiave di firma di sviluppo: $K"
fi
"$BT/apksigner" sign --ks "$K" --ks-pass pass:nyctalus --ks-key-alias nyctalus \
  --out "$USCITA_APK" "$L/allineato.apk"
rm -f "$USCITA_APK.idsig"
"$BT/apksigner" verify "$USCITA_APK"
echo "» pronto: $USCITA_APK ($(du -h "$USCITA_APK" | cut -f1), versione $VNAME)"
