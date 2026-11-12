<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Manuale del Terminale (nyctalus-cli)

Con **nyctalus-cli** si chatta e si mandano file ai contatti **scrivendo comandi**, senza
finestre. Usa lo stesso motore della chat: stesso anonimato, stessi contatti, stessa
conversazione (quello che scrivi dal terminale lo vedi anche nella chat, e viceversa).

Serve quando vuoi fare in fretta, su un server o un Raspberry senza schermo, via SSH, o per
automatizzare qualcosa con uno script.

## 1. Aprirlo

| Dove | Come |
|---|---|
| PC Linux | pulsante **⌨ Terminale** del mini menu: si apre il terminale del sistema, già pronto. Oppure, in un terminale qualsiasi, il comando `nyctalus-cli` |
| Telefono | icona **⌨ Terminale** nella **seconda fila** della schermata iniziale dell'app: un pannello dentro l'app. Oppure, aprendo Termux a mano, il comando `nyctalus-cli` |
| Windows | pulsante **⌨️ Terminale** della finestra di Nyctalus: lo stesso pannello del telefono |
| Server, SSH, Raspberry | `nyctalus-cli avvia` in un terminale (accende il motore senza finestre; Ctrl-C lo spegne), poi i comandi da un altro terminale |

Il motore deve essere acceso: sul PC lo accende da solo il pulsante del mini menu (e lo spegne
quando chiudi il terminale, se l'aveva acceso lui); se chat o File sono aperti usa il loro.
Sul telefono lo accende l'app, come sempre.

## 2. I comandi

Nel terminale aperto dal **mini menu del PC** e nel pannello del **telefono** e di **Windows** si
scrivono così come sono: `contatti`, `scrivi mario ciao`. In un terminale qualsiasi (o in uno
script, o via SSH) si scrive `nyctalus-cli` davanti: `nyctalus-cli contatti`.
(Nel terminale del mini menu, `file` è quello di Nyctalus; il comando `file` del sistema resta
raggiungibile con `command file`.)

| Comando | Cosa fa |
|---|---|
| `io` | il tuo Contact ID, da dare agli amici |
| `contatti` | i contatti; ● se sono in linea |
| `aggiungi <indirizzo> <nome>` | aggiunge un contatto |
| `scrivi <contatto> <testo>` | manda un messaggio |
| `leggi <contatto> [-n 50]` | la conversazione (gli ultimi 20 messaggi, o quanti dici) |
| `ascolta` | messaggi, file e richieste che arrivano, **in diretta** (Ctrl-C o ■ per smettere) |
| `invia <contatto> <file…>` | manda uno o più file con il programma File (grandi, pezzi verificati, più percorsi) |
| `file` | i trasferimenti con l'avanzamento; `file -s` li segue in diretta |
| `accetta <id>` · `rifiuta <id>` | un file in arrivo che chiede conferma |
| `pausa <id>` · `riprendi <id>` · `annulla <id>` | un trasferimento |
| `togli <id>` · `pulisci` | dall'elenco dei trasferimenti (i file restano) |
| `impostazioni [soglia 100] [canali 3] [inchat si\|no] [riservata si\|no]` | soglia di accettazione automatica (MB), collegamenti in parallelo, file anche nella chat, **modalità riservata** (`MANUALE-CHAT.md` §3-ter·1) |
| `sblocca` | apre i dati protetti da passphrase (la chiede senza mostrarla); va dato a ogni avvio del motore, prima di ogni altro comando |
| `cassaforte` · `cassaforte attiva\|cambia\|togli` | **protezione dei dati con passphrase**: com'è adesso; la accende (cifra identità, contatti, cronologia e file ricevuti), cambia la passphrase, la toglie. ⚠️ Passphrase dimenticata = dati persi (`MANUALE-CHAT.md` §7-bis) |
| `aiuto` | l'elenco dei comandi |

**`<contatto>`**: il nome, anche solo l'inizio e senza maiuscole (`scrivi mar ciao` va a
«Mario» se è l'unico che comincia così), oppure l'indirizzo `.nyct`.
**`<id>`**: l'identificativo del trasferimento; bastano le prime lettere, quelle che mostra
`file`.

Esempi (in un terminale qualsiasi; in quello del mini menu senza `nyctalus-cli`):
```
nyctalus-cli scrivi mario "ci vediamo alle 8"
nyctalus-cli invia mario ~/Video/vacanze.mp4
nyctalus-cli file -s
```
Sul telefono, i file si indicano con il loro percorso: per esempio
`invia mario ~/storage/shared/Download/film.mkv`.

## 3. Il pannello del telefono e di Windows

- In alto scorre quello che rispondono i comandi; in basso il campo dove scrivi (niente
  maiuscola automatica né correttore) e **⏎** per eseguire.
- **aiuto** (o il pulsante «aiuto») mostra una **scheda** fatta per il telefono: i comandi a gruppi, ognuno con la sua spiegazione sotto. **Toccando un comando** lo usi: quelli che non chiedono altro partono subito, gli altri (es. `scrivi`) si preparano nel campo e tu aggiungi il resto.
- **↑** richiama i comandi già dati (anche le frecce della tastiera).
- I **pulsanti rapidi** (contatti, il mio ID, file, file in diretta, ascolta, aiuto) eseguono
  il comando con un tocco.
- **■** ferma un comando che resta acceso (`ascolta`, `file -s`).
- `pulisci schermo` svuota il pannello; **←** torna alla schermata iniziale.

**Sicurezza:** il pannello esegue **solo** nyctalus-cli. I comandi non passano mai da una shell
(scrivere `$(…)` o `;` non esegue niente) e il canale accetta solo la pagina dell'app. Anche
se qualcuno riuscisse a ingannare la pagina, potrebbe fare al massimo quello che fa già la
chat: non arriva alle chiavi né al resto di Termux.

## 4. Come si collega al motore

All'accensione il motore scrive la sua porta di controllo e un segreto in
`~/.nyctalus-chat/controllo`, leggibile **solo da te** (permessi 600), e lo cancella quando si
spegne. nyctalus-cli lo legge da lì, oppure dalle variabili `NYCTALUS_CHAT_PORT` e
`NYCTALUS_CHAT_TOKEN`. Senza il segreto il motore non risponde.

Se il motore è spento, nyctalus-cli lo dice e suggerisce come accenderlo; se nel frattempo si è
riavviato, basta ridare il comando.

Dettagli: protocollo di controllo in `SPECIFICA.md`; prove `24-cli` e `03-webapp` (passi del
Terminale) in `MANUALE-PROVE.md`.
