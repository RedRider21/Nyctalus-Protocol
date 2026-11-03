<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Manuale del programma File

**File** serve a mandare file **grandi** ai tuoi contatti, anche di molti GB, attraverso la rete
Nyctalus: nessuno vede cosa mandi né a chi, e il file arriva **identico** (ogni pezzo è
controllato). Per le foto e i documenti di tutti i giorni bastano gli allegati della chat; File
è per quando serve di più: file grossi, pausa e ripresa, più velocità.

## 1. Aprirlo

| Dove | Come |
|---|---|
| PC Linux | pulsante **File** (cartella con freccia) del mini menu, oppure il comando `nyctalus-file`, oppure **Nyctalus File** nel menu delle applicazioni |
| Telefono | pulsante **File** della schermata iniziale dell'app, oppure nella chat: menu ⋮ di un contatto → **📦 Manda un file grande** |
| Windows | pulsante **📦 File** della finestra di Nyctalus |

Usa lo stesso motore della chat: se la chat è aperta ci si aggancia, altrimenti lo accende da
solo. Il motore resta acceso finché è aperta **almeno una** fra chat e File: chiudere la chat
non interrompe un file in viaggio.

> Dopo l'aggiornamento, se la chat era già aperta, **chiudila e riaprila** una volta: così File
> può agganciarsi al suo motore.

## 2. Mandare un file

**Sul telefono e su Windows:** **＋ Invia** → il contatto → **🖼 Foto e video** (la galleria) oppure
**📁 Sfoglia le cartelle del telefono** (Download, Documenti, Film…). Tocchi uno o più file e
**Invia**. **Niente copie:** il motore legge il file **dove sta**. «↩ Torna a File» torna indietro
senza uscire. (Sul telefono, per vedere le cartelle Termux deve avere il permesso di leggere la memoria:
**«Installa / aggiorna Nyctalus in Termux»** lo chiede una volta sola, tocca **Consenti**; a mano
è `termux-setup-storage`.)

**Sul PC:**

1. **Invia file…** (in alto a sinistra), oppure **trascina** i file sulla finestra.
2. Scegli il **contatto** (c'è la ricerca).
3. Fatto: il file compare in **In uscita** con la barra, la velocità e il tempo che manca.

Se il contatto non è in linea, il file aspetta e **parte da solo** appena si collega.

## 3. Ricevere un file

- Arriva solo dai tuoi **contatti**: da chi non è fra i contatti viene rifiutato senza disturbarti.
- Fino alla **soglia** (100 MB, si cambia in ⚙) parte da solo; sopra, la riga dice «Vuole
  mandarti questo file» con **Accetta** / **Rifiuta**, e il desktop ti avvisa.
- Finito: **Apri** o **Mostra nella cartella**. Il file sta in `~/.nyctalus-chat/allegati/<contatto>/`
  e compare anche nella **conversazione** della chat, come un allegato.

## 3-bis. L'archivio (telefono e Windows)

I file ricevuti restano nell'**archivio privato** di Nyctalus (sul telefono la cartella di Termux:
**nessun'altra app la vede**). Scheda **Archivio**:
- **spazio**: quanto occupano i file ricevuti e quanto resta libero sul telefono;
- dal **più grande** al più piccolo, così una ISO o un film si trovano subito;
- **Apri** (per i tipi sicuri), **Elimina** (anche più file insieme con le caselle), **Condividi**.

Sul telefono **Apri** e **Condividi** li fa l'app Nyctalus (in primo piano): prende il file dall'archivio, ne fa una copia temporanea privata (cancellata dopo un giorno) e la passa, in sola lettura, all'app che scegli.

**Condividi** usa il foglio di condivisione standard di Android: il file va all'app che scegli tu
(WhatsApp, Drive, la galleria…). L'anonimato della rete Nyctalus **non cambia**, ma da lì in poi
il file è in mano a quell'app, che può copiarlo, caricarlo online o leggerne i metadati (per
esempio la posizione delle foto): per questo lo chiede sempre, con un avviso.

## 4. Durante il viaggio

| Pulsante | Cosa fa |
|---|---|
| **Pausa** | ferma il trasferimento per tutti e due; **Riprendi** riparte da dove era |
| **Annulla** | lo ferma per sempre; quello che era arrivato viene buttato |

Se cade la rete, si spegne il PC o il contatto si scollega, **non si ricomincia da capo**: alla
ripresa arrivano solo i pezzi che mancano.

**⚙ Impostazioni:** la soglia di accettazione automatica e quanti **collegamenti in parallelo**
usare (3 di solito; di più = più veloce ma più carico sulla rete).

## 5. Come funziona (in breve)

- Il file è diviso in **pezzi** da 256 KB, ognuno con la sua **impronta** (BLAKE3): un pezzo
  arrivato sbagliato o manomesso viene scartato e chiesto di nuovo; alla fine si ricontrolla
  tutto il file.
- I pezzi viaggiano su **più collegamenti contemporanei**, ognuno su un percorso diverso della
  rete: più veloce, e nessun nodo vede tutto il file passare.
- La proposta, l'accettazione e i comandi viaggiano nella stessa **sessione cifrata e
  autenticata** della chat; anche i collegamenti dei dati provano chi sei, quindi un estraneo
  non può chiedere pezzi dei tuoi file.
- Dettagli tecnici: `SPECIFICA.md` (trasferimenti), prova `22-trasferimenti` in `MANUALE-PROVE.md`.
