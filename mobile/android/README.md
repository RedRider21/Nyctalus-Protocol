<!-- SPDX-License-Identifier: AGPL-3.0-or-later
     Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol. -->
# App Android di Nyctalus (APK)

Guscio leggero (circa 750 KB col pacchetto per Termux incluso, nessuna dipendenza AndroidX) attorno alla web-app
della chat. Deriva dal launcher di Termux-NexusSEC-OS e funziona allo stesso modo:

1. il motore e il servizio `nyctalus-mobile` girano **dentro Termux**;
2. l'app chiede a Termux di avviarli (intent `RUN_COMMAND`, permesso
   «Esegui comandi in Termux» che l'app chiede al primo avvio);
3. mostra la web-app da `http://127.0.0.1:18731`.

| Cosa | Come |
|---|---|
| Accesso | segreto casuale di 128 bit generato dall'app, scritto da Termux in `~/.nyctalus-mobile/token` (permessi 600), presentato una volta con `?t=` e poi come cookie |
| Termux non risponde | apre Termux un istante e torna da sola; altrimenti «Copia il comando di avvio» da incollare in Termux |
| Allegati | i campi file della web-app aprono galleria, file o fotocamera del telefono |
| Foto/video dalla fotocamera | restano nella cartella **privata** dell'app (`FotoProvider`), non in galleria; cancellati dopo un giorno |
| Ponte `window.NyctalusHost` | `copia(testo)`, `condividi(testo)`, `tema(id)`, `esci()` |
| Installa Nyctalus in Termux | il pacchetto è negli `assets`; `ServitorePacchetto` lo serve su 127.0.0.1 (porta libera, indirizzo casuale da 128 bit, 15 minuti); Termux lo scarica con curl, verifica lo SHA-256, lancia `installa.sh`, scrive il segreto, avvia il servizio e riapre l'app (`am start`) |
| Pulsanti | Installa Termux (F-Droid/GitHub), Aggiorna Termux (app o `pkg upgrade` con conferma), Installa/aggiorna Nyctalus, Apri Termux, Comando di avvio, Aggiornamenti da GitHub (in arrivo) |
| Tema | la schermata d'avvio usa i colori del tema scelto nella web-app |
| Link esterni | si aprono fuori dall'app, solo se toccati dall'utente |

L'app non chiede il permesso della fotocamera: usa l'app fotocamera del telefono.

## Compilare

```bash
ANDROID_HOME=/percorso/android-sdk bash strumenti/compila-apk.sh
```

Senza Gradle e senza scaricare nulla: servono un Android SDK (build-tools e una
piattaforma) e un JDK. Il risultato è `mobile/android/Nyctalus.apk`;
`strumenti/crea-installazioni.sh` lo copia in `installazioni/Android/`.
La chiave di firma di sviluppo sta in `mobile/android/privato/` (fuori da git):
conservala, altrimenti il telefono rifiuta gli aggiornamenti sopra la versione installata.

Su GitHub lo stesso script gira nel workflow `.github/workflows/apk-mobile.yml`.
I file Gradle (`settings.gradle`, `build.gradle`) servono solo ad aprire il progetto
in Android Studio.
