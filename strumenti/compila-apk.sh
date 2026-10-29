#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
#
# Compila l'app Android (mobile/android) SENZA Gradle e senza scaricare nulla:
# bastano un Android SDK già presente (build-tools + una piattaforma) e un JDK.
#   aapt2 (risorse) → javac (codice) → d8 (dex) → zipalign → apksigner
# Risultato: mobile/android/Nyctalus.apk (che crea-installazioni.sh mette in
# installazioni/Android/). La compilazione su GitHub usa invece Gradle (stesso codice).
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
"$BT/aapt2" compile --dir "$APP/res" -o "$L/res.zip"
"$BT/aapt2" link -I "$JAR" --manifest "$L/AndroidManifest.xml" -o "$L/base.apk" --java "$L/gen" \
  --min-sdk-version 24 --target-sdk-version 34 --version-code "$VCODE" --version-name "$VNAME" "$L/res.zip"

mkdir -p "$L/classi" "$L/dex"
mapfile -t SORGENTI < <(find "$L/gen" "$APP/java" -name '*.java')     # (il percorso può contenere spazi)
"${JAVAC[@]}" -nowarn -Xlint:-options -source 8 -target 8 -encoding UTF-8 -bootclasspath "$JAR" -classpath "$BT/core-lambda-stubs.jar" -d "$L/classi" "${SORGENTI[@]}"
mapfile -t CLASSI < <(find "$L/classi" -name '*.class')
"$BT/d8" --release --min-api 24 --lib "$JAR" --output "$L/dex" "${CLASSI[@]}"

cp "$L/base.apk" "$L/senza-firma.apk"
( cd "$L/dex" && zip -q -X "$L/senza-firma.apk" classes.dex )
"$BT/zipalign" -p -f 4 "$L/senza-firma.apk" "$L/allineato.apk"

K="$AND/privato/firma-sviluppo.jks"
if [ ! -f "$K" ]; then
  mkdir -p "$AND/privato"; chmod 700 "$AND/privato"
  keytool -genkeypair -keystore "$K" -storepass nyctalus -keypass nyctalus -alias nyctalus \
    -keyalg RSA -keysize 3072 -validity 10000 -dname "CN=Nyctalus sviluppo, O=RedRider21" >/dev/null 2>&1
  echo "» creata la chiave di firma di sviluppo: $K"
fi
"$BT/apksigner" sign --ks "$K" --ks-pass pass:nyctalus --ks-key-alias nyctalus \
  --out "$AND/Nyctalus.apk" "$L/allineato.apk"
rm -f "$AND/Nyctalus.apk.idsig"
"$BT/apksigner" verify "$AND/Nyctalus.apk"
echo "» pronto: $AND/Nyctalus.apk ($(du -h "$AND/Nyctalus.apk" | cut -f1), versione $VNAME)"
