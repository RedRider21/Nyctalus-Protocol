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
(`http://127.0.0.1:18731/?t=…`). Con l'app Nyctalus basta toccare l'icona: avvia Nyctalus in Termux e
apre la chat. Le foto e i video scattati dalla chat con l'app restano nella sua cartella
privata (non finiscono nella galleria) e si cancellano dopo un giorno.

In cima all'elenco una striscia ti dice a che punto è la rete:
- **⏳ Avvio della rete…**: pochi secondi, intanto vedi già le chat;
- **Rete di prova LOCALE**: funziona solo sul telefono, per parlare con altri serve la
  rete condivisa (`DEPLOY.md`);
- **Nyctalus è fermo** con il bottone **Avvia**.

## 2. Le chat (schermata principale)

- Ogni riga mostra **avatar**, **nome**, **ultimo messaggio**, **ora o giorno**, il
  numero di **messaggi non letti** e il 📌 delle chat fissate (che restano in cima).
- 🔍 cerca fra i contatti; **✏️** (in basso a destra) aggiunge un contatto: incolla il
  suo **Contact ID** e dagli un nome.
- In basso c'è il **tuo Contact ID**: toccalo per copiarlo e darlo agli amici (non è
  segreto).
- All'apertura elenco e conversazioni compaiono **subito**, anche prima che la rete sia
  pronta: il telefono ne tiene una copia (cache **IndexedDB**) che poi si allinea con il
  motore.

**Pressione lunga su una chat** → menu: Apri · Segna come letto/non letto · Fissa in
alto · Rinomina · Copia il Contact ID · **Cancella cronologia** · **Elimina la chat**.

## 3. La conversazione

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
- **Rete Nyctalus**: **Avvia** / **Ferma**.

## 6. Sicurezza sul telefono

- Su Android **ogni app** può collegarsi alle porte `127.0.0.1`: per questo la web-app
  entra solo con un **segreto** (il `?t=…` dell'indirizzo, poi un cookie), che sta nella
  cartella privata di Termux. Senza segreto, niente accesso a chat e file.
- L'orchestratore legge **solo** le cartelle consentite (allegati, galleria in sola
  lettura) e rifiuta richieste da altri siti.
- I messaggi viaggiano cifrati da un capo all'altro e **chi ti scrive è autenticato**
  (vedi `MANUALE-CHAT.md` e `SICUREZZA.md`).
