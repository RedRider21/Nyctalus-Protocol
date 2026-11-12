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

Tocca l'icona **Nyctalus**: compare la schermata di benvenuto e l'app avvia Nyctalus in Termux.
La prima volta (o se qualcosa non va) sotto la riga di stato compare **⚡ Prepara Nyctalus**: fa
tutto da solo, Termux compreso (`MANUALE-INSTALLAZIONE.md` §2.1); sparisce appena Nyctalus è pronto.
Sotto il logo, due file di pulsanti quadrati con la sola icona, come il mini menu del PC
(tenendo premuto un pulsante compare il suo nome):

| Icona | Cosa apre |
|---|---|
| fumetto · **Chat** | la chat; si accende con «● Nyctalus è pronto» |
| persone · **Rubrica** | la sola **rubrica della rete** a tutto schermo: chi c'è, chi è in linea, iscriversi (`MANUALE-CHAT.md` §3-bis) |
| globo · **Browser** | il browser, con la **🦇 modalità Nyctalus** (§1-bis) |
| cartella · **File** | il programma **File**: file grandi ai contatti, archivio dei ricevuti con spazio, elimina e condividi (`MANUALE-FILE.md`) |
| i · **Info e manuali** | il sito con i manuali online |
| cursori · **Impostazioni** | tutte le impostazioni (sotto) |
| *(seconda fila)* riquadro con `>_` · **Terminale** | **nyctalus-cli** in un pannello: chat e file scrivendo comandi (`contatti`, `scrivi mario ciao`, `file -s`…), output in diretta, ⏹ per fermare (`MANUALE-TERMINALE.md`) |

**🔗 Rete** (sotto la scritta di stato): a quale rete sei collegato, con l'**impronta**. La rete è
una sola per tutta l'app (chat, browser 🦇). Toccala per:
- **importare un codice di rete** (`NYCT1:…`; se l'hai copiato, è già proposto);
- **passare** a un'altra rete salvata (● quella in uso), o tornare alla rete di prova;
- togliere una rete dall'elenco.

Ancora più comodo: dalla pagina della rete (per la rete di casa `http://IP-DEL-PC:18800/`, in
Chrome) tocca **📲 Collega l'app Nyctalus** e conferma nome e impronta.

**Impostazioni** (l'ultimo pulsante): **tutte** le impostazioni, a schermo intero — le stesse del
☰ della chat (§5: Contact ID, tema, sfondo, foto senza metadati, rete Nyctalus, rete condivisa)
più la sezione **App**: 🔔 notifiche dei messaggi con l'app chiusa (§5-bis) e ⏻ ripartenza
all'accensione del telefono. **←** (o il tasto indietro) torna alla schermata iniziale.
Il **tema** vale per tutta l'app: schermata iniziale, chat, chiamate, anche al volo.
Se Nyctalus non è ancora pronto si apre una finestra ridotta (tema, notifiche, accensione).

**Strumenti ▾**: **⚡ Prepara tutto**, Termux (installa/aggiorna), **🦇 Installa/aggiorna Nyctalus
in Termux** (dopo ogni APK nuova l'app lo propone da sola), comando di avvio, **☁ Aggiornamenti da
GitHub** (controlla l'ultima release, scarica e installa la nuova app).

Il **tasto indietro** riporta sempre indietro di un passo fino a questa schermata; la chat resta
aperta dietro. Se Android ferma Nyctalus mentre sei in un'altra app, l'app lo riaccende da sola
(se resta fermo: in Termux `nyctalus-mobile --sfondo`). La notifica fissa di Termux è normale.

Senza l'app: in Termux `nyctalus-mobile` e apri in Chrome l'indirizzo che stampa.

## 1-bis. Il browser (solo nell'app)

È il browser di NovaOS in stile Chrome: schede, incognito, preferiti, cronologia, trova nella
pagina, traduzione, modalità lettura, sito desktop, download. In più ha la **modalità
Nyctalus**: il pulsante **🦇** a sinistra della barra, oppure la voce in cima al menu ⋮.

**Per tornare all'app:** menu ⋮ → **↩ Torna a Nyctalus** (in fondo). Riporta alla schermata
iniziale dell'app e chiude il browser. Il pulsante **🏠** invece resta quello del browser: apre
la pagina iniziale che hai scelto.

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

In cima, una striscia dice a che punto è la rete: **⏳ Avvio…** (pochi secondi), **Rete di prova LOCALE** (importa un codice di rete per parlare con altri, §1), **Nyctalus è fermo** (bottone **Avvia**).

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

Dal menu **⋮** in alto a destra: **Chiamata vocale** o **Videochiamata**. La schermata è come
in Telegram: avatar e tasti tondi in basso (Vivavoce, 📹 Video, Muto, Termina, ⌄ per ridurla e
continuare a usare la chat); col video chi parla è a tutto schermo e tu sei in un **riquadro**
che puoi **trascinare** in un angolo, **toccare** per scambiarlo con lo schermo intero e
**allargare con due dita** (doppio tocco: misura normale). 📞 **Solo voce** spegne la telecamera,
🔄 **Gira** passa alla telecamera posteriore. Le chiamate in arrivo squillano anche con l'app
chiusa, come notifica, con i pulsanti **Rispondi** (verde) e **Rifiuta** (rosso), come nelle
altre app di chiamate: Rispondi apre la chiamata già accettata (con il video, se è una
videochiamata), Rifiuta la chiude senza aprire l'app. Da Android 12 è la notifica «chiamata in
arrivo» del sistema; a schermo bloccato compare a tutto schermo. Alla prima chiamata Android chiede il permesso del microfono, alla prima
videochiamata quello della telecamera (dati solo alla chat, mai ai siti senza il tuo «Consenti»).
Dettagli: `MANUALE-CHAT.md` §3-quater.

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

Foto e video scattati dalla chat con l'app restano nella sua cartella privata (non nella galleria) e si cancellano dopo un giorno.

Tocca **📎**: sale il **selettore**.
- La **griglia** mostra le tue **foto e video più recenti**; la prima casella è la
  **Fotocamera**.
- **Tocca le foto per sceglierle**: compare il **numero d'ordine** nel cerchio blu (1,
  2, 3…). Tocca di nuovo per togliere.
- Con almeno un elemento scelto compaiono la **didascalia** e **Invia (N)**.
- Le schede in basso: **Galleria** · **File** (qualunque file) · **Altre foto** (il
  selettore di sistema) · **Video** (registra un video).

Note:
- Per vedere le foto nella griglia (e le cartelle nel programma File) Termux deve poter leggere la memoria: «Installa / aggiorna Nyctalus in Termux» lo chiede una volta (tocca **Consenti**); a mano: `termux-setup-storage`.
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
- **Tema**: Notte, Ardesia (stile Telegram), Viola, Chiaro, Menta, Cielo, oppure **🎨 Personalizzato** (selettore di colore o codice `#rrggbb`, più Scuro/Chiaro). Vale per tutta l'app;
- **Sfondo della chat**: pipistrelli, velo di colore, tinta unita;
- **Foto senza metadati**;
- **Forma della rete**: se chi gestisce la rete ha scelto la **forma HTTP/3**, sotto «Rete
  condivisa» compare «per chi osserva sembri un visitatore del sito …» (`GUIDA-COMPLETA.md` §12).
- **UDP bloccato**: se la rete dove sei (ufficio, Wi-Fi di un albergo) blocca UDP, l'app passa
  **da sola** a TCP sulla stessa porta dopo qualche secondo (`GUIDA-COMPLETA.md` §13-bis).
- **🌉 Ponti nascosti** (in Rete condivisa, quando sei su una rete condivisa): incolla la
  riga segreta di un ponte, cioè un'entrata privata fuori dall'elenco. Da quel momento si entra
  sempre da lì. Spiegazione: `GUIDA-COMPLETA.md` §13.
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
- Per spegnerle: **Impostazioni → App → 🔔 Notifiche** (toccalo per passare a spente).
- Dopo aver riacceso il telefono apri l'app una volta, oppure attiva l'avvio all'accensione (sotto).

Se il telefono è spento o senza rete, i messaggi restano nella **casella cifrata** della rete e
ti arrivano, con la notifica, appena torni in rete (`MANUALE-CHAT.md` §3-ter).

**Avvio all'accensione (a scelta):** in **Impostazioni → App → ⏻ Riparti all'accensione** decidi
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
- **🔐 Protezione dei dati con passphrase** (dalla 0.3.20): **Impostazioni** → scheda
  **💬 Chat** → «Proteggi con una passphrase». Identità, contatti, cronologia e **file
  ricevuti** vengono cifrati sul telefono; a ogni avvio l'app mostra «🔐 Dati protetti» e
  chiede la passphrase (finché non la scrivi la chat resta chiusa e fuori linea). Foto, video
  e documenti ricevuti si aprono come sempre: l'app ne prepara una copia in chiaro temporanea,
  cancellata alla chiusura; nemmeno le miniature restano in chiaro. ⚠️ Passphrase dimenticata
  = dati persi. Tutti i dettagli e i limiti: `MANUALE-CHAT.md` §7-bis.
