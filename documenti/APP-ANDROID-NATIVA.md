<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# App Android autosufficiente (senza Termux) — progetto

Stato: **in lavorazione** (dal 15/11/2026). Obiettivo: un solo APK che si installa e funziona,
senza Termux, con la stessa interfaccia di oggi. Termux resta un'aggiunta facoltativa per chi
vuole i comandi (`nyctalus-cli`, collaudo, script, siti dal telefono).

## Com'è oggi

```text
APK (Java: MainActivity, BrowserActivity, notifiche, proxy)
  └─ chiede a Termux (RUN_COMMAND) di avviare
       nyctalus-mobile = orchestratore Python (mobile/orchestratore/nyctalus_mobile.py, ~1600 righe)
         ├─ serve la web-app (mobile/webapp) su http://127.0.0.1:18731 (segreto → cookie)
         ├─ avvia il motore: chat, start (tunnel), site (servizio .nyct)
         ├─ ponte WebSocket ↔ porta di controllo della chat; terminale (pty)
         └─ file, galleria, miniature, archivio cifrato, notifiche, SOCKS del browser
```

## Come sarà

```text
APK (Java)
  ├─ lib/<abi>/libnyctalus.so  = il motore (lo stesso eseguibile compilato dal flusso «Eseguibili»,
  │                               rinominato: Android 10+ esegue solo ciò che sta in nativeLibraryDir)
  ├─ assets/webapp              = la stessa web-app
  ├─ ServizioNyctalus           = servizio in primo piano (notifica fissa): lancia
  │      libnyctalus.so app --webapp … --dati …   (HOME = cartella privata dell'app)
  ├─ MainActivity / WebView     = apre http://127.0.0.1:18731/?t=SEGRETO
  └─ ponte JavaScript «NyctalusAndroid»: apri/condividi file, permessi, notifiche (cose che solo
     Android sa fare)

motore Rust: nuovo comando `nyctalus app` = l'orchestratore, con la STESSA interfaccia HTTP/WebSocket
del Python (stesse rotte /api/…, /ws, stessi file di configurazione ~/.nyctalus-mobile)
```

Perché in Rust dentro il motore e non in Java nell'app:
1. **si prova sul PC**: le prove esistenti dell'orchestratore (`test/strumenti/prova_orchestratore.py`,
   `02-orchestratore`, `03-webapp`, `05-orchestratore` di sicurezza, notifiche, miniature,
   cassaforte…) girano identiche contro `nyctalus app`;
2. **un solo programma** per telefono senza Termux, Termux e (dopo) Windows;
3. nessuno strumento nuovo da scaricare (niente Kotlin, niente NDK: gli eseguibili Android li
   compila già GitHub).

## Fasi

| # | Fase | Prova |
|---|---|---|
| ✅ 1 | `nyctalus app`: server HTTP, segreto/cookie, controllo dell'origine, file statici della web-app, `/api/stato` | `05-orchestratore` (sicurezza) contro Rust |
| ✅ 2 | servizi: avvia/ferma chat, tunnel, servizio `.nyct`; `/api/rete` (codice, reti salvate, vicini), `/api/impostazioni` | `02-orchestratore`, `prova_orchestratore.py` |
| ✅ 3 | ponte WebSocket `/ws` ↔ porta di controllo della chat; notifiche (`/api/notifiche`) | `03-webapp`, `12-notifiche` |
| ✅ 4 | file: allegati con Range, carica, galleria, sfoglia, miniature (anche EXIF), archivio e cassaforte | `prova_miniature.py`, `46`/`49-cassaforte` |
| ✅ 5 | SOCKS del browser interno | `06-browser-mobile` (sicurezza) |
| ◐ 6 | APK nativo: `libnyctalus.so` per arm64-v8a e armeabi-v7a, servizio in primo piano, WebView, ponte JavaScript, avvio all'accensione | `08-app-android` (compilazione e contenuto dell'APK) |
| 7 | passaggio dall'app con Termux: identità, contatti, messaggi copiati nella cartella dell'app (con il consenso) | prova dedicata |
| 8 | impostazioni nuove: DuckDNS, IP dal router; «telefono anche nodo» (solo Wi-Fi e in carica, solo intermedio) dopo il ponte inverso (tappa 2) | — |

Il terminale (`/ws/terminale`) resta solo nella versione con Termux.

## Limiti dichiarati
- A telefono spento o app chiusa dal sistema i messaggi aspettano nella **casella cifrata** della
  rete (come oggi); niente servizi di Google per le notifiche.
- Android può fermare i servizi in sottofondo per risparmiare batteria: l'app chiede di essere
  esclusa dall'ottimizzazione della batteria (facoltativo).
- Grandezza stimata: ~6-7 MB con arm64 + armv7 (oggi 11 MB con il pacchetto di Termux dentro).

## Stato delle prove contro `nyctalus app` (15/11/2026)
Sicurezza dell'orchestratore 9/9 · orchestratore (02) completo, senza processi orfani · web-app 74 + 3 saltate
(il ⌨ Terminale esiste solo con Termux) · notifiche 8/8 · cassaforte 46 e 49 complete · browser dell'app (06) 24/24 ·
ponti nascosti (32) 21/21 · test unitari Rust del modulo `app` 9. Differenza trovata e corretta: all'arresto
si lascia ai programmi il tempo di chiudersi (la chat svuota la vista in chiaro della cassaforte).

## Fase 6, stato (15/11)
Fatto: `MotoreNativo` (avvia «libnyctalus.so app» con HOME privata, TMPDIR/XDG_RUNTIME_DIR nella cache, galleria e
sfoglia del telefono, registro `app.log`), avvio/riavvio/notifiche/accensione senza Termux, permessi della galleria,
`compila-apk.sh` di serie «con tutto» (5,2 MB; `NYCTALUS_APK_TERMUX=1` per la versione di prima), «Installa in Termux»
dal pacchetto firmato della release. Prova 08: 34/34.
Da fare: motore per Android con `app` compilato dal flusso «Eseguibili» (download ~50 MB, da approvare); prova sul
telefono vero dell'utente (qui non c'è un telefono né un emulatore).

## Python o Rust (parere dato all'utente, 15/11)
Rust per ciò che trasporta dati o deve funzionare da solo (motore, telefono, Windows); Python per le finestre
GTK e gli strumenti di gestione su Linux (già presente, più rapido da modificare; gtk-rs costerebbe centinaia di
MB di dipendenze). Il vecchio orchestratore Python si toglie quando l'app nativa è stabile.

## Manualistica (decisione dell'utente, 15/11/2026)
Termux diventa **un'aggiunta facoltativa, a scelta**: l'app da sola fa tutto; Termux serve solo a
chi vuole i comandi (`nyctalus-cli`, `nyctalus-collaudo`, `nyctalus-integrita`, `nyctalus-rete`,
script e automazioni, altri programmi attraverso il tunnel, siti `.nyct` dal telefono). Va detto
chiaramente **ovunque**, nella versione che pubblica l'app nuova (prima sarebbe falso):
README · sito (installazione, telefono, comandi) · MANUALE-TELEFONO · MANUALE-INSTALLAZIONE §2 ·
GUIDA-COMPLETA (parte 1 e 2, telefono) · MANUALE-TERMINALE (diventa «solo con Termux») ·
COMANDI §7 (telefono) · MANUALE-COLLAUDO (dal telefono: con Termux) · LEGGIMI · DEPLOY · le note
della release. Tabella «Con Termux puoi…» (comandi, automazioni, curl/git/ssh nel tunnel, siti,
collaudo, gestione della rete) in MANUALE-TELEFONO e GUIDA-COMPLETA.
