<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Manuale della chat `.nyct`

Una chat **senza server e senza metadati**: non esiste un'azienda in mezzo, nessun
numero di telefono, nessuna rubrica globale. Ogni persona **è** un indirizzo
`.nyct` — il suo **Contact ID** — e i messaggi viaggiano cifrati da un capo
all'altro dentro la rete Nyctalus (circuito a più salti → punto d'incontro →
servizio del destinatario). Modello simile a Ricochet/Cwtch.

---

## 1. Avviare la chat

- Dal **launcher**: premi il bottone **Chat**.
- Oppure da terminale:
  ```sh
  ./avvia-chat.sh          # nel pacchetto
  ./scripts/avvia-chat.sh  # in sviluppo
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

Il pallino/anello **verde** sull'avatar indica che il contatto è **in linea**.
La **cronologia** resta sul tuo computer (in `~/.nyctalus-chat/cronologia`) e si
ricarica quando riapri una conversazione.

Se qualcuno ti scrive da un indirizzo **non in rubrica**, la chat te lo segnala e
ti propone di aggiungerlo per rispondere.

---

## 4. Allegati: file, foto, video, audio, documenti

- **Inviare**: apri la chat con un contatto e premi la **graffetta 📎** accanto al
  campo di scrittura (puoi sceglierne più d'uno, con i filtri Immagini / Video /
  Audio / Documenti / Archivi), oppure **trascina i file** dentro la conversazione.
- **Foto**: compaiono con l'**anteprima**; un clic le apre a grandezza piena.
- **Altri file**: una scheda con icona, nome e dimensione, e i pulsanti **Apri** e
  **Mostra nella cartella**. Durante il trasferimento vedi la **barra di
  avanzamento**.
- **Dove finiscono**: in `~/.nyctalus-chat/allegati/<contatto>/`.
- **Limite**: 200 MB per file (si cambia con l'opzione `--max-file` del motore).

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

## 5. Aspetto e finestra

- Menu **⚙** in alto: scegli il **tema** (Notte, Ardesia stile Telegram, Viola
  notturno, Chiaro) e lo **sfondo** della chat (trama a pipistrelli, velo di
  colore, tinta unita).
- La finestra si ricorda **dimensione, posizione** e se era **massimizzata**:
  la ritrovi come l'hai lasciata.
- Finché non apri una conversazione vedi solo il benvenuto col pipistrello; il
  campo per scrivere compare quando selezioni un contatto.

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
./scripts/demo-chat.sh
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
