<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Manuale d'uso — telefono (Android)

Sul telefono Nyctalus è una **web-app in stile Telegram** che apri nel browser (o
nell'app **Nyctalus**). Dietro, dentro **Termux**, girano il motore (lo stesso del PC) e
un piccolo servizio, l'**orchestratore**, che avvia la rete, fa da ponte con il motore e
gestisce gli allegati. Installazione: **`MANUALE-INSTALLAZIONE.md`**, capitolo 2.

---

## 1. Avvio

In Termux scrivi `nyctalus-mobile` e apri nel browser l'indirizzo che stampa
(`http://127.0.0.1:18731/?t=…`). Con l'app Nyctalus basta toccare l'icona: compare la **schermata di benvenuto** col logo,
e l'app avvia Nyctalus in Termux. Sotto il nome «Nyctalus · Anonymous Protocol» c'è una
fila di pulsanti, come il mini menu del PC:

| Pulsante | Cosa apre |
|---|---|
| 💬 **Chat** | la chat. Si accende quando la scritta dice «● Nyctalus è pronto» |
| 👥 **Rubrica** | la sola **rubrica della rete**, a tutto schermo (← o indietro tornano qui; la chat si apre solo scegliendo qualcuno a cui scrivere): chi usa la rete, chi è in linea, iscriversi con un nome (vedi `MANUALE-CHAT.md` §3-bis). Funziona se la rete ha una rubrica (la rete di casa sì) |
| 🌐 **Browser** | il browser: **🦇 modalità Nyctalus** (anonimo, siti `.nyct`) oppure navigazione normale; parte da DuckDuckGo |
| 📁 **File** | in arrivo: invio e ricezione di file attraverso Nyctalus |
| ℹ️ **Info** | il sito del progetto con i manuali online (anche toccando il logo) |

Le reti che importi restano in un **elenco**: toccando **🔗 Rete** le vedi tutte (● quella in uso)
e passi dall'una all'altra con un tocco, senza reimportare il codice; c'è anche la rete di
prova. Toccando quella in uso puoi toglierla dall'elenco.

Sotto la scritta di stato c'è la riga **🔗 Rete**: dice a quale rete sei collegato
(«prova locale», oppure il nome della rete e la sua impronta). **La rete è una sola per
tutta l'app**: chat, browser in modalità 🦇 e, quando ci sarà, il gestore dei file. Toccala
per **importare un codice di rete** (se ne hai già copiato uno, è proposto da solo) o per
tornare alla rete di prova. Più comodo ancora: dalla pagina della rete (per la rete di casa
`http://IP-DEL-PC:18800/`) tocca **📲 Collega l'app Nyctalus**. L'app si apre e chiede conferma
con nome e impronta della rete; l'esito resta a schermo.

Il **tasto indietro** riporta sempre al punto di partenza: dalla conversazione all'elenco
delle chat, dall'elenco a questa schermata; dal browser, chiudendo l'ultima scheda. La chat
resta aperta dietro, quindi ritoccando 💬 ci si rientra subito.
I pulsanti per Termux (installa/aggiorna Termux, installa/aggiorna Nyctalus, apri Termux,
comando di avvio) sono sotto **Strumenti ▾**. **Dopo aver installato un'APK nuova tocca sempre
«Installa/aggiorna Nyctalus»** (dalla 0.25 il motore è già pronto nell'app: bastano pochi secondi): la chat, il motore e il servizio stanno in Termux, e l'APK da
sola non li aggiorna. Se il servizio è acceso, l'aggiornamento lo ferma, sostituisce i file e lo
riaccende con la versione nuova (fino alla 0.20 restava acceso il servizio vecchio e
l'installazione poteva fermarsi a metà). Le foto e i video scattati dalla chat con l'app restano nella sua cartella
privata (non finiscono nella galleria) e si cancellano dopo un giorno.

In cima all'elenco una striscia ti dice a che punto è la rete:
- **⏳ Avvio della rete…**: pochi secondi, intanto vedi già le chat;
- **Rete di prova LOCALE**: funziona solo sul telefono. Per parlare con altri importa un
  **codice di rete** (vedi sotto);
- **Nyctalus è fermo** con il bottone **Avvia**.

**Se Android ferma Nyctalus** mentre sei in un'altra app (per esempio Chrome), tornando
nell'app questa se ne accorge e lo riaccende da sola in Termux. Per ridurre il problema,
quando Nyctalus parte Termux resta «sveglio» (`termux-wake-lock`): compare una notifica fissa
di Termux, ed è normale. Se resta fermo, apri Termux e scrivi `nyctalus-mobile --sfondo`.

## 1-bis. Il browser (solo nell'app)

È il browser di NovaOS in stile Chrome: schede, incognito, preferiti, cronologia, trova nella
pagina, traduzione, modalità lettura, sito desktop, download. In più ha la **modalità
Nyctalus**: il pulsante **🦇** a sinistra della barra, oppure la voce in cima al menu ⋮.

- **🦇 ON (verde):** tutto il traffico del browser passa dalla rete Nyctalus e il sito non vede
  il tuo indirizzo. Si aprono anche i siti `.nyct`.
- **Prima di aprire qualunque pagina il browser verifica** che il traffico passi davvero
  dalla rete. Se Nyctalus non è acceso o il telefono non lo permette, **non apre nulla** e lo
  spiega.
- In questa modalità le schede sono tutte in incognito: niente cronologia, niente cache né
  archivi dei siti, cookie cancellati quando esci. Sono spenti anche WebRTC, fotocamera,
  microfono e posizione. La ricerca usa DuckDuckGo invece di Google. Non si passano indirizzi
  ad altre app. Download e traduzione passano anche loro dalla rete (i download finiscono in
  `Android/data/org.nyctalus.mobile/files/Download`).
- Il browser gira in un processo separato: chiudendolo, la volta dopo riparte normale.
- Serve Android 9 o successivo, con «Android System WebView» o Chrome aggiornati.
- I siti `.nyct` fuori dalla modalità Nyctalus sono bloccati, come sul PC.

## 2. Le chat (schermata principale)

- Ogni riga mostra **avatar**, **nome**, **ultimo messaggio**, **ora o giorno**, il
  numero di **messaggi non letti** e il 📌 delle chat fissate (che restano in cima).
- 🔍 cerca fra i contatti; **✏️** (in basso a destra) aggiunge un contatto: incolla il
  suo **Contact ID** e dagli un nome.
- **👥** (in alto, se la rete ha una rubrica) apre la **rubrica della rete**: iscriviti con
  un nome, guarda chi è in linea e aggiungi le persone con un tocco (`MANUALE-CHAT.md` §3-bis).
- In basso c'è il **tuo Contact ID**: toccalo per copiarlo e darlo agli amici (non è
  segreto).
- All'apertura elenco e conversazioni compaiono **subito**, anche prima che la rete sia
  pronta: il telefono ne tiene una copia (cache **IndexedDB**) che poi si allinea con il
  motore.

**Pressione lunga su una chat** → menu: Apri · Segna come letto/non letto · Fissa in
alto · Rinomina · Copia il Contact ID · **Cancella cronologia** · **Elimina la chat**.

**Avvisi senza ripetizioni.** Se lo stesso errore si ripete (per esempio il PC con la rete di
casa è spento), in chat resta **un solo avviso**, il più recente. Se è la chat stessa a non
raggiungere Nyctalus, lo dice la barra in alto: «⚠ La chat non riesce a parlare con
Nyctalus: ritento da sola…». I tentativi si fanno sempre più radi, fino a uno ogni 30 secondi.

## 3. La conversazione

Accanto all'ora dei tuoi messaggi ci sono le **spunte** come in Telegram: ✓ spedito, 📮✓ nella
casella cifrata (il contatto non era in linea), ✓✓ consegnato, ✓✓ blu letto (vedi
`MANUALE-CHAT.md`).

- Tocca una chat per aprirla; **←** (o il tasto Indietro) torna all'elenco.
- Scrivi in basso e tocca **➤**.
- I messaggi sono raggruppati per giorno (**Oggi**, **Ieri**…); la chat resta in fondo
  quando arrivano messaggi e foto.
- **⋮** in alto: lo stesso menu della pressione lunga sulla chat.

**Pressione lunga su un messaggio** → Copia testo · Inoltra (a un altro contatto) ·
**Elimina per me** (solo dal tuo telefono). Sugli allegati: Apri · Inoltra · Elimina.

## 4. Allegati (come Telegram)

Tocca **📎**: sale il **selettore**.
- La **griglia** mostra le tue **foto e video più recenti**; la prima casella è la
  **Fotocamera**.
- **Tocca le foto per sceglierle**: compare il **numero d'ordine** nel cerchio blu (1,
  2, 3…). Tocca di nuovo per togliere.
- Con almeno un elemento scelto compaiono la **didascalia** e **Invia (N)**.
- Le schede in basso: **Galleria** · **File** (qualunque file) · **Altre foto** (il
  selettore di sistema) · **Video** (registra un video).

Note:
- Per vedere le foto nella griglia, in Termux una volta sola: `termux-setup-storage`.
- La griglia mostra le foto **a pagine**: le prime 30, poi altre mentre scorri, fino alle
  300 più recenti. Per le altre usa **Altre foto**.
- Le **miniature** si preparano poche alla volta, in sottofondo e solo per le caselle
  visibili. Si usa prima quella già contenuta nella foto, che è istantanea; restano in
  memoria, quindi la seconda apertura è immediata. Con `python-pillow` (l'installatore
  lo aggiunge) sono nitide; senza, si usano quelle già contenute nelle foto. Una foto grande
  senza miniatura mostra un'icona: non si carica mai la foto intera solo per l'anteprima.
- Il cerchio di selezione compare quando la foto è pronta.
- Puoi scegliere **più foto insieme**: arrivano tutte (prima della versione 0.5 dell'app
  una poteva perdersi).
- Le foto JPEG/PNG partono **senza metadati** (niente posizione GPS né modello del
  telefono): si può cambiare nelle impostazioni.
- In chat le **foto** si vedono a tutta bolla, i **video** con miniatura e ▶ (le
  miniature richiedono `pkg install ffmpeg`), gli **audio** si ascoltano dentro la chat,
  gli altri file hanno il cerchio colorato con l'estensione.
- **Apri** usa le app del telefono, ma solo per i tipi innocui (foto, video, audio, PDF,
  testo): programmi e APK ricevuti **non si aprono mai da soli**.

## 5. Impostazioni (☰)

- il tuo **Contact ID** con **Copia** e **Condividi**;
- **Tema**: Notte, Ardesia (stile Telegram), Viola, Chiaro;
- **Sfondo della chat**: pipistrelli, velo di colore, tinta unita;
- **Foto senza metadati**;
- **Rete condivisa** (la stessa della riga 🔗 Rete della schermata iniziale: vale per tutta
  l'app): **📥 Importa codice** → incolla il codice `NYCT1:…` che ti ha dato
  chi gestisce la rete (per la rete di casa lo mostra il PC: apri nel browser del
  telefono l'indirizzo `http://IP-DEL-PC:18800/` e copialo). La rete si riavvia da sola
  su quella condivisa; **↩ Rete di prova** torna indietro. Dopo l'importazione compare
  l'**impronta della rete** (es. `92E6-9EB0`): deve essere uguale a quella mostrata da chi
  gestisce la rete (per la rete di casa, il PC).
- **Rete Nyctalus**: una spia dice in che fase sei. **● verde** = attiva (e se è la rete
  di prova locale o quella condivisa); **● gialla lampeggiante** = in avvio o in arresto;
  **○ grigia** = ferma; **● rossa** = errore. **▶ Avvia rete** si può toccare solo a
  rete ferma (o dopo un errore, come «↻ Riprova»), **■ Ferma rete** solo a rete attiva.

## 5-bis. Notifiche dei messaggi (anche con l'app chiusa)

Come Telegram, ma **senza Google**: quando Nyctalus è pronto, l'app accende un piccolo servizio
che resta attivo in sottofondo (nella barra c'è l'icona fissa **«Nyctalus attivo»**). Per ogni
messaggio o file in arrivo compare una notifica con il nome del contatto e l'anteprima;
**toccala** e si apre direttamente quella conversazione. Con l'app aperta non compaiono
notifiche (i messaggi si vedono già) e aprendola si azzerano.

- La prima volta Android 13+ chiede il permesso di mostrare notifiche: rispondi **Consenti**
  (se l'hai negato: Impostazioni → App → Nyctalus → Notifiche).
- Nessuno all'esterno sa quando ricevi messaggi: le notifiche nascono sul telefono, dal servizio
  in Termux (solo `127.0.0.1`), non passano da server di Google o altri.
- Se Android ferma Termux, l'app lo riaccende da sola (al massimo una volta ogni 5 minuti).
- **Batteria:** il servizio consuma poco ma qualcosa. Per non farlo fermare, in Impostazioni →
  App → Nyctalus (e Termux) → Batteria scegli **Senza restrizioni**.
- Per spegnerle: **Strumenti ▾ → 🔔 Notifiche dei messaggi: attive** (toccalo per passare a spente).
- Dopo aver riacceso il telefono apri l'app una volta, oppure attiva l'avvio all'accensione (sotto).

Se il telefono è spento o senza rete, i messaggi restano nella **casella cifrata** della rete e
ti arrivano, con la notifica, appena torni in rete (`MANUALE-CHAT.md` §3-ter).

**Avvio all'accensione (a scelta):** in **Strumenti ▾ → ⏻ Avvio all'accensione del telefono** decidi
tu se, dopo aver riacceso il telefono, Nyctalus deve ripartire da solo (servizio in Termux e
notifiche) oppure aspettare che tu apra l'app. All'inizio è su **no**.

## 6. Sicurezza sul telefono

- Su Android **ogni app** può collegarsi alle porte `127.0.0.1`: per questo la web-app
  entra solo con un **segreto** (il `?t=…` dell'indirizzo, poi un cookie), che sta nella
  cartella privata di Termux. Senza segreto, niente accesso a chat e file.
- L'orchestratore legge **solo** le cartelle consentite (allegati, galleria in sola
  lettura) e rifiuta richieste da altri siti.
- I messaggi viaggiano cifrati da un capo all'altro e **chi ti scrive è autenticato**
  (vedi `MANUALE-CHAT.md` e `SICUREZZA.md`).
