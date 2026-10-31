<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Manuale della chat `.nyct`

Una chat **senza server e senza metadati**: non esiste un'azienda in mezzo, nessun
numero di telefono, nessuna rubrica globale (solo, se il gestore la accende, la piccola
**rubrica della rete** a cui ci si iscrive se si vuole: §3-bis). Ogni persona **è** un indirizzo
`.nyct` — il suo **Contact ID** — e i messaggi viaggiano cifrati da un capo
all'altro dentro la rete Nyctalus (circuito a più salti → punto d'incontro →
servizio del destinatario). Modello simile a Ricochet/Cwtch.

---

## 1. Avviare la chat

- Dal **launcher**: premi il bottone **Chat**.
- Oppure da terminale:
  ```sh
  ./avvia-chat.sh          # nel pacchetto
  ./strumenti/avvia-chat.sh  # in sviluppo
  ```

La **finestra si apre subito**. La rete, però, ha bisogno di qualche secondo per
salire (deve costruire un circuito a più salti e registrare il tuo servizio al
punto d'incontro): finché non è pronta vedi già la finestra e il tuo Contact ID,
ma l'invio/ricezione parte quando la rete è su. È normale, non è un blocco.

> **Perché «ci mette un po'»?** Non è un difetto: a differenza di una chat
> normale, qui non ci si collega a un server centrale. Il programma mette in
> piedi un pezzo di rete anonima (relay + circuito cifrato + punto d'incontro)
> prima di poter parlare. È il prezzo dell'assenza di metadati.

---

## 2. Il tuo Contact ID (in basso a sinistra)

Il codice monospazio in basso a sinistra, con il bottone **Copia**, è il tuo
**Contact ID**: *è il tuo indirizzo nella rete*, l'unica cosa da dare agli altri
perché ti scrivano. Funziona come un numero di telefono o un indirizzo `.onion`.

- **Non è segreto**: condividilo pure (chat, email, di persona, QR…). Meglio
  passarlo per un canale che l'altro **riconosce come tuo**, così siete sicuri
  che sia davvero il tuo (come si fa con gli indirizzi Ricochet).
- Il **segreto** è la tua chiave privata, che resta sul tuo computer in
  `~/.nyctalus-chat/identita`. **Non cambia**: il tuo Contact ID resta lo stesso
  nel tempo. Se cancelli quel file perdi l'identità (ne nasce una nuova diversa).

---

## 3. Aggiungere un contatto e scrivere

1. Premi **«+»** in alto a sinistra.
2. Incolla l'**indirizzo `.nyct`** dell'altra persona (il suo Contact ID) e
   dagli un **nome** (è solo un soprannome locale, lo vedi solo tu).
3. Selezionalo nell'elenco a sinistra e scrivi nel campo in basso → **Invio** (o
   il bottone ➤).

**Emoticon:** il pulsante **😊** accanto alla graffetta apre il pannello: categorie (faccine,
gesti, cuori, animali, cibo, attività, viaggi, oggetti, simboli) e **🕘 Recenti**. Toccata
un'emoticon, entra nel testo dove stai scrivendo; il campo di scrittura resta visibile.

Il pallino/anello **verde** sull'avatar indica che il contatto è **in linea**. Non serve
scriversi prima: all'avvio la chat «bussa» da sola ai contatti (e poi ogni tanto a chi non
risponde, con pause crescenti fino a 10 minuti). Chi è acceso compare in linea entro pochi
secondi, e anche lui vede te.

Sotto il nome, in cima alla conversazione, c'è **«in linea»** oppure **«ultimo accesso oggi
alle 18:05»** (ieri / data). Lo calcola il tuo dispositivo da quando ha visto il contatto
l'ultima volta; non viene mandato a nessuno (file `~/.nyctalus-chat/visti`).
La **cronologia** resta sul tuo computer (in `~/.nyctalus-chat/cronologia`) e si
ricarica quando riapri una conversazione.

Se qualcuno ti scrive da un indirizzo **che non è fra i tuoi contatti**, la chat te lo segnala e
ti propone di aggiungerlo per rispondere.

---

## 3-bis. La rubrica della rete (👥)

Se la rete ha una **rubrica** (la rete di casa la accende da sola, §6 del manuale del
gestore), nella chat compare il pulsante **👥**: sul PC in alto, accanto al nome; sul
telefono in cima all'elenco delle chat. Si apre anche **da sola**, senza la chat: dal **menu
del PC** (bottone **Rubrica**: compare solo la finestra della rubrica) e dalla **schermata
d'avvio del telefono** (riquadro **👥 Rubrica**: una schermata intera; **←** o il tasto indietro
riportano all'avvio). La chat si apre solo quando scegli qualcuno a cui scrivere.

- **Iscriviti** con un nome (1-40 caratteri): chi usa la stessa rete lo vede e può scriverti.
  **✏️ Cambia nome** e **Cancellati** sono sempre lì; cancellato, sparisci dall'elenco.
- L'elenco mostra gli iscritti: prima **chi è in linea** (anello verde), poi gli altri con
  **l'ultimo accesso**. Accanto a ciascuno: **＋ Aggiungi** (entra fra i tuoi contatti e si
  apre la conversazione) oppure **💬 Scrivi** se c'è già. **⟳** aggiorna.
- Iscriversi è **facoltativo**: senza iscrizione vedi l'elenco ma non ci compari.
- Come funziona: la rubrica è un «utente automatico» della rete, con un suo indirizzo
  `.nyct` (scritto nel codice di rete). Le richieste sono messaggi della chat cifrata, che
  ne autentica il mittente: nessuno può iscriversi a nome tuo. La rubrica conserva solo
  l'elenco degli iscritti (nome, indirizzo), non la cronologia delle richieste.
- Per farsi vedere in linea, la chat di chi è iscritto saluta la rubrica ogni 3 minuti.
- Sul telefono la rubrica dice sempre cosa sta facendo: «Leggo la rete in uso…», «Chiedo
  l'elenco…»; se dopo 15 secondi non ha risposta avvisa che la prima volta il collegamento può
  richiedere fino a un minuto. Dopo 90 secondi, o se la rete in uso non ha una rubrica, spiega
  il motivo e offre **⟳ Riprova**. Una rete importata con un codice vecchio non ha la rubrica:
  ricollega il telefono con «📲 Collega l'app». Lo stesso vale se hai aggiornato da una
  versione fino alla 0.20: il servizio vecchio, rimasto acceso, aveva tolto la rubrica dal
  codice. Ricollega una volta dopo l'aggiornamento.

## 3-bis·1. Le spunte (come Telegram)

Accanto all'ora dei tuoi messaggi:

| Segno | Significa |
|---|---|
| ✓ | **spedito**: è partito verso il contatto |
| 📮✓ | **nella casella cifrata**: il contatto non era in linea, lo riceverà appena si collega |
| ✓✓ | **consegnato**: è arrivato sul dispositivo del contatto (anche ritirato dalla casella) |
| ✓✓ blu | **letto**: il contatto ha aperto la conversazione |

- Le conferme viaggiano cifrate dentro la stessa sessione dei messaggi; se il contatto non è
  raggiungibile partono appena lo diventa.
- **Messaggi mai persi:** se il contatto sparisce di colpo (es. modalità aereo) e un messaggio non
  riceve il «consegnato» entro 45 secondi, la chat lo mette nella casella. Se poi arriva due volte,
  il destinatario scarta il doppione.
- Con chi ha una versione senza spunte i messaggi restano su ✓.

## 3-ter. La casella cifrata (📮 messaggi a chi non è in linea)

Se la rete ha una casella (la rete di casa sì), puoi scrivere **anche a chi non è in linea**:
telefono spento, senza rete, programma chiuso. Scrivi come sempre; se il contatto non risponde,
accanto all'ora del messaggio compare **📮✓** (nella casella). Appena lui si
collega, il messaggio gli arriva (con la notifica) come se glielo avessi mandato in quel momento,
con l'ora in cui l'hai scritto.

- **Solo il destinatario lo può leggere.** Il messaggio parte già chiuso in una busta cifrata con
  la sua chiave (schema Noise «X»). Dentro c'è anche la prova crittografica che l'hai scritto tu:
  nessuno può lasciare posta a nome di un altro.
- **La casella non sa chi scrive.** La busta le arriva attraverso i circuiti, senza che il
  mittente si presenti; la casella vede solo «c'è posta per questo indirizzo». A ritirarla può
  essere solo il destinatario (lo chiede dalla sua chat, autenticata).
- **Solo testo** (fino a 16 KB). Gli allegati partono quando il contatto è in linea.
- La posta resta in casella al massimo **14 giorni**; al ritiro viene cancellata.
- Quello che la casella vede comunque: che qualcuno ha lasciato posta per quell'indirizzo, quando
  e quanto è grande, e quando il destinatario la ritira.

## 4. Allegati: file, foto, video, audio, documenti

- **Inviare**: apri la chat con un contatto e premi la **graffetta 📎**. Si apre il
  **selettore come in Telegram**:
  - la **griglia** delle tue foto e video più recenti (sul PC dalle cartelle Immagini,
    Video e Download; sul telefono dalla galleria); la prima casella è la **Fotocamera**;
  - **tocca/clicca le foto per sceglierle**: compare il **numero d'ordine** nel cerchio
    blu (1, 2, 3…), e vengono inviate in quell'ordine; un altro tocco le toglie;
  - con almeno una scelta compaiono la **didascalia**, «Senza metadati» e **«Invia (N)»**;
  - le schede in basso: **File** (qualunque file) · **Altre foto** (scegli da altre
    cartelle) · **Foto** e **Video** dalla fotocamera (con l'audio del microfono; il timer
    rosso mostra la durata, «Ferma e usa» chiude la registrazione).

  In alternativa puoi **trascinare i file** dentro la conversazione.
- Per i file scelti con «File», «Altre foto», la fotocamera o il trascinamento, **prima di inviare** compare l'**anteprima** (come «Invia foto» di Telegram):
  miniature, una **didascalia** facoltativa (arriva come messaggio subito dopo) e
  l'opzione per togliere i metadati dalle foto.
- **Come si vedono in chat**:
  - **foto** a tutta bolla con l'ora sopra; un clic le apre a grandezza piena;
  - **video** con la miniatura, il tasto ▶ e la durata; un clic li apre nel
    lettore del sistema;
  - **audio** con il pulsante ▶/⏸: si ascoltano **dentro la chat**;
  - **altri file** con il cerchio colorato dell'estensione (PDF, ZIP, DOCX…);
  - durante il trasferimento, un **anello** mostra l'avanzamento.
- **Dove finiscono**: in `~/.nyctalus-chat/allegati/<contatto>/`.
- **Limite**: 200 MB per file (si cambia con l'opzione `--max-file` del motore).
- **Fotocamera**: si accende **solo** quando scegli «Scatta una foto» o «Registra
  un video», e si spegne appena chiudi quella finestra. Servono GStreamer e
  ffmpeg (vedi `DEPLOY.md`); senza, queste due voci restano disattivate.

**Sicurezza degli allegati**
- Viaggiano nella **stessa sessione cifrata e autenticata** dei messaggi; alla
  fine il destinatario controlla l'**impronta BLAKE3** di tutto il file: se anche
  un solo byte non torna, il file viene **scartato**.
- Il **nome** del file ricevuto viene ripulito (niente percorsi come
  `../../`, niente file nascosti), e un file con lo stesso nome non viene mai
  sovrascritto.
- **Apri** funziona direttamente solo per i tipi innocui (foto, video, audio,
  PDF, testo). Programmi, script e archivi **non vengono mai aperti da soli**: la
  chat ti mostra la cartella e decidi tu.
- **Foto senza metadati**: per impostazione predefinita le foto JPEG/PNG vengono
  inviate **senza dati EXIF** (niente posizione GPS né modello del telefono). Si
  può disattivare dal menu ⚙.

---

## 4-bis. Notifiche

- **PC:** se la finestra della chat non è in primo piano, un messaggio o file in arrivo fa
  comparire una notifica del desktop con nome e anteprima (serve `notify-send`, di solito già
  presente). Si spengono da ⚙ → **Notifiche sul desktop dei nuovi messaggi**.
- **Telefono:** notifiche anche con l'app chiusa, senza Google: vedi `MANUALE-TELEFONO.md` §5-bis.

## 5. Aspetto e finestra

- Menu **⚙** in alto: scegli il **tema** (Notte, Ardesia stile Telegram, Viola
  notturno, Chiaro) e lo **sfondo** della chat (trama a pipistrelli, velo di
  colore, tinta unita).
- La finestra si ricorda **dimensione, posizione** e se era **massimizzata**:
  la ritrovi come l'hai lasciata.
- Finché non apri una conversazione vedi solo il benvenuto col pipistrello; il
  campo per scrivere compare quando selezioni un contatto.

---

### Menu del tasto destro (e i tre puntini ⋮)

Come in Telegram, con il **tasto destro**:
- **su un contatto** dell'elenco (o con **⋮** in alto nella conversazione):
  Apri la chat · Segna come letto/non letto · Fissa in alto · Rinomina… · Copia
  il Contact ID · **Cancella cronologia** · **Elimina la chat**;
- **su un messaggio**: Copia testo · Inoltra (a un altro contatto) · **Elimina
  per me**; sugli allegati anche Apri · Mostra nella cartella · Salva con nome….

«Elimina» e «Cancella» chiedono conferma e agiscono **solo sul tuo computer**:
non c'è un server che possa cancellare i messaggi dall'altra parte.

Nell'elenco vedi per ogni chat l'**ultimo messaggio**, l'ora (o il giorno), il
**numero di messaggi non letti** e il 📌 delle chat fissate, che restano in cima.

---

## 6. Serve una rete condivisa

La chat è a **due parti**: per parlarvi, **tu e l'altra persona dovete essere
sulla stessa rete**.

- **Rete pubblica (uso reale).** Se hai configurato la rete condivisa nel file
  `~/.nyctalus-browser/config.json` (`directory_url` + `authority`, vedi
  `DEPLOY.md`), la chat la usa automaticamente: così raggiungi chiunque sia su
  quella rete. È il modo con cui tu e i tuoi contatti vi parlate davvero.
- **Rete di prova locale (solo per curiosare).** Senza quella configurazione la
  finestra si apre comunque, ma su una rete di prova **solo sul tuo PC**: utile
  per vedere com'è fatta, **non** per raggiungere altre persone.

**Provare la chat da solo** (due finestre sul tuo computer, come due persone):
```sh
./strumenti/demo-chat.sh
```
Aggiungi in ciascuna finestra l'indirizzo dell'altra e scrivetevi.

---

## 7. Sicurezza in due righe

- I messaggi sono **cifrati end-to-end**; i relay della rete non vedono il
  contenuto e non conoscono gli IP di mittente e destinatario.
- **Chi ti scrive è autenticato**: la chat verifica con la crittografia che il
  mittente possieda davvero la chiave privata dell'indirizzo che dichiara, quindi
  **nessuno può spacciarsi** per un tuo contatto.
- Resta a te la **verifica umana** del Contact ID (scambiatevelo per un canale
  fidato) e l'**igiene**: se nei messaggi ti identifichi, nessuna rete può
  nasconderti. Vedi `SICUREZZA.md`.

> ⚠️ Prototipo non ancora verificato da revisori indipendenti: usalo per provare
> funzionamento, usabilità e stabilità, **non** per proteggere dati reali.

---

## 8. Dove stanno i tuoi dati

Tutto in `~/.nyctalus-chat/`:

| File/cartella | Contenuto |
|---|---|
| `identita` | la tua chiave (identità) — **il tuo Contact ID**. Tienila al sicuro. |
| `contatti` | l'elenco dei contatti che hai aggiunto (indirizzo + soprannome). |
| `cronologia/<addr>.log` | lo storico dei messaggi (e degli allegati) con ciascun contatto. |
| `allegati/<contatto>/` | i file ricevuti. |
| `ui.json` | tema, sfondo, dimensione e posizione della finestra. |

Per **cambiare identità** da zero, chiudi la chat ed elimina `identita` (perderai
il vecchio Contact ID). Per fare pulizia di una conversazione, cancella il
relativo file in `cronologia/`.
