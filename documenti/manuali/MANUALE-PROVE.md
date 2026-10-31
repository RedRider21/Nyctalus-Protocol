<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Manuale delle prove (funzionamento e sicurezza)

Tutte le prove stanno nella cartella **`test/`** dei sorgenti e si rifanno quando
serve, ad esempio dopo ogni modifica o prima di distribuire una versione.

```sh
test/esegui-tutti.sh                  # tutte le prove, con riepilogo finale
test/esegui-tutti.sh sicurezza        # solo sicurezza  (= strumenti/verifica.sh)
test/esegui-tutti.sh funzionamento    # solo funzionamento
bash test/sicurezza/04-impostore-chat.sh   # una prova sola
NYCT_TIENI=1 test/esegui-tutti.sh     # conserva i log delle prove, per indagare
```

Esito: ogni verifica stampa **PASS** / **FAIL** / **SALTA**; alla fine il riepilogo
per prova e il totale. Il comando termina con errore se anche una sola verifica fallisce.

**Le prove sono sicure da lanciare:**
- usano solo **cartelle temporanee** (`/tmp/nyctalus-test.*`), sempre ripulite;
- usano **porte dedicate 19xxx**, diverse da sviluppo (1080…) e telefono (18xxx);
- **non toccano i tuoi dati** (identità, chat) né gli altri programmi;
- **non accendono mai la webcam**: la fotocamera si prova con sorgenti video/audio
  finte di GStreamer;
- chiudono tutti i processi che avviano.

**Requisiti:** Rust (`cargo`), Python 3. Facoltativi, altrimenti la prova viene
**SALTATA**: `google-chrome` (web-app), un display grafico + GStreamer (fotocamera),
`ffmpeg` (video).

---

## Prove di sicurezza (`test/sicurezza/`)

| Prova | Cosa verifica |
|---|---|
| `01-directory-firmata` | l'elenco dei nodi firmato è accettato; manomesso o firmato da un'altra autorità è **rifiutato** |
| `02-offuscamento` | con la chiave di rete il nodo risponde, senza resta muto (trasporto camuffato) |
| `03-segreto-controllo` | la porta di controllo della chat risponde solo a chi ha il **segreto** (sbagliato o assente → rifiutato) |
| `04-impostore-chat` | **attacco dal vivo**: chi si spaccia per un contatto senza averne la chiave viene sfidato e buttato fuori; nessun messaggio falso consegnato né salvato |
| `06-browser-mobile` | browser dell'app in modalità Nyctalus: processo separato, nessun permesso sensibile, verifica col gettone, `.nyct` solo in rete, nessuna uscita verso altre app, WebRTC spento, download e traduzione in rete; **inoltratore**: pagina di verifica, niente connessioni a rete ferma, UDP rifiutato, indirizzi locali (127.x, localhost, 192.168.x, 10.x, ::1) rifiutati, sito vero raggiunto attraverso la rete |
| `05-orchestratore` | il servizio del telefono: niente accesso senza segreto (anche WebSocket), niente file di sistema (`/etc/passwd`), niente `../` fuori dalla web-app, niente comandi da altri siti, nomi caricati ripuliti, caricamenti enormi rifiutati |

## Prove di funzionamento (`test/funzionamento/`)

| Prova | Cosa verifica |
|---|---|
| `00-test-automatici` | tutti i test del motore (`cargo test`): crittografia, celle, conflux, rendezvous, chat |
| `01-chat-e-allegati` | su una **rete di prova** (4 nodi, circuiti a 3 salti): testo nei due sensi, identità autenticata, allegati da 3 MB e immagini **identici al byte** (SHA-256), avanzamento, cronologia; **tre allegati inviati insieme** a un contatto nuovo arrivano tutti (difetto dei collegamenti doppi); due contatti che si sono solo aggiunti si vedono **in linea senza scriversi** e l'**ultimo accesso** arriva nell'elenco |
| `02-orchestratore` | il servizio del telefono avvia rete e chat, ponte WebSocket verso il motore, caricamento e lettura a pezzi (Range), arresto senza processi orfani; **miniature** della galleria con Pillow, senza Pillow né ffmpeg (miniatura EXIF con orientamento) e mai la foto intera se grande; **galleria da 2000 foto** elencata in tempi brevi e tenuta in memoria |
| `03-webapp` | la web-app in Chrome a misura di telefono: elenco, cache IndexedDB, conversazione in fondo, pressione lunga, foto senza EXIF, **selettore allegati** (griglia, selezione numerata, «Invia (2)»), messaggi e foto a se stessi **attraverso la rete**; pulsanti della rete accesi/spenti secondo la fase |
| `04-fotocamera` | foto (JPEG) e video (H.264 + audio AAC, file chiuso bene) dalla fotocamera della chat, finestra di anteprima d'invio; **tasto destro su una foto** in chat: il menu riguarda quella foto (prima «Elimina per me» poteva cancellarne un'altra) |
| `05-installatore-termux` | installatore in un **Termux simulato**: tutto in `~/nyctalus`, nessun pacchetto superfluo né `pkg upgrade`, `termux.properties` intatto e senza righe doppie, avvio automatico solo se chiesto, `nyctalus-mobile` avvia/ferma, disinstallazione che **non tocca gli altri progetti**, comandi omonimi altrui non sovrascritti |
| `06-installazioni` | la cartella `installazioni/`: pacchetti e LEGGIMI per ogni dispositivo, **nessun dato personale** dentro, il pacchetto PC si estrae e parte, quello Android contiene tutto il necessario |
| `07-installatore-pc` | installatore del PC in una HOME finta: menu e comandi, nessun sudo superfluo, cose altrui intatte, disinstallazione pulita |
| `09-server-e-rete` | il percorso completo di una **rete condivisa**: installatore dei **nodi** in una radice finta (servizio systemd corretto, chiavi 600, riga stabile ai riavvii, nodi avviati col comando del servizio), **gestore** (`nyctalus-rete` crea/aggiungi/firma/verifica/codice/servi; il codice non contiene mai la chiave segreta), **client** PC e telefono che importano il codice e passano alla rete condivisa; codici rovinati rifiutati |
| `10-servizio-riavviato` | un servizio `.nyct` (come la chat di un PC) spento di colpo e riacceso: chi lo cerca subito dopo lo raggiunge al primo tentativo (le registrazioni morte al punto d'incontro vengono scoperte e saltate), e le visite seguenti sono immediate |
| `11-rubrica` | la **rubrica della rete**: la rete di casa la accende e la mette nel codice; iscrizione, elenco con chi è in linea, nome non valido rifiutato, cancellazione, nessuna cronologia, file privato; pulsanti Rubrica su PC e telefono |
| `12-notifiche` | **notifiche** senza Google: flusso `/api/notifiche` del servizio del telefono (negato senza il segreto, un messaggio in arrivo → riga con mittente e testo); servizio dell'app, permessi, apertura della conversazione dalla notifica, interruttore; notifica del desktop sul PC |
| `13-casella` | **casella cifrata**: Bruno è spento, Anna gli scrive → la busta va nella casella (illeggibile, senza l'indirizzo di Anna, file privati, nessun registro); Bruno si accende e la riceve autenticata come di Anna, la casella la cancella; **spunte** (📮✓ → ✓✓, ✓ → ✓✓ → ✓✓ blu) e **modalità aereo** (lato di Bruno congelato: il messaggio senza «ricevuto» va nella casella e arriva una volta sola; con il **battito** Bruno non resta «in linea»); deposito anonimo via SOCKS, busta troppo grande rifiutata, busta falsa scartata |
| `14-siti` | **`nyctalus-sito`**: sito creato con la pagina d'esempio (niente risorse esterne, file privati), messo online sulla rete di casa e **visitato attraverso la rete**, pagina aggiunta visibile subito, arresto pulito; sito dinamico dietro un server web locale; nomi/backend/codici sbagliati rifiutati; la chiave esportata riprende lo stesso indirizzo; unità systemd dell'utente; rimozione con copia della chiave |
| `15-nodi-per-nome` | nodi annunciati con un **nome** (IP dinamico): righe ed elenco firmato col nome, sonda dei nodi, sito `.nyct` visitato con circuiti a 3 salti fra nodi «per nome», nome irrisolvibile saltato |
| `16-rubrica-di-rete` | **rubrica e casella in una rete vera**: il gestore crea la rete, il «server» le accende col solo codice (`nyctalus-rete rubrica --codice`, nessuna chiave dell'autorità), il gestore rifà il codice includendole (e le ricorda), un utente si iscrive; unità systemd |
| `17-windows` | **Windows simulato**: il programma d'avvio accende il servizio una volta sola e dà l'indirizzo della finestra; la finestra d'avvio (stato «pronto», riquadri, 🔗 Rete, guida di Firefox, ⏻ Esci che spegne tutto); contenuto dello zip senza dati personali |
| `18-chiamate` | **chiamate, motore**: invito, risposta, 2 s di voce a pacchetti da 20 ms nei due sensi (identici, ritardo misurato), fine, invito rifiutato, invito ricordato per chi apre la chat dopo, id non valido rifiutato |
| `19-chiamata-pc` | **chiamata dalla chat GTK** con microfono e altoparlante DI PROVA: 📞, risposta, voce Opus nei due sensi, barra con durata e qualità, muto, volume, riaggancia |
| `08-app-android` | app Android: niente permesso fotocamera, scatti privati, nomi di file filtrati; il comando che l'app fa eseguire a Termux scrive il **suo segreto** (600) e il servizio già acceso lo accetta, il vecchio no; «Installa Nyctalus in Termux» in un **Termux simulato** (pacchetto con impronta sbagliata rifiutato, installazione, `allow-external-apps`, ritorno all'app, niente `pkg upgrade`); il **servitore Java vero** dell'app provato su JVM (404 agli indirizzi sbagliati, consegna integra, spegnimento); compila l'APK se c'è un Android SDK (`ANDROID_HOME`) e controlla che contenga il pacchetto |

## Aggiungere una prova

Un file `NN-nome.sh` in `test/sicurezza/` o `test/funzionamento/` che inizia con
`. "$(dirname "$0")/../lib.sh"` e usa le funzioni comuni:

| Funzione | A cosa serve |
|---|---|
| `titolo`, `pass`, `fail`, `salta`, `verifica` | intestazione ed esiti |
| `assicura_binario` | compila il motore se manca o è vecchio |
| `rete_di_prova BASE` | autorità + 4 nodi sulle porte BASE…BASE+3 (variabili `SEC` `PUB` `DIR`) |
| `identita NOME BACKEND CONTROLLO SOCKS [SEGRETO]` | motore della chat + servizio `.nyct`; indirizzo in `$ADDR` (non chiamarla dentro `$(...)`) |
| `controllo PORTA SEGRETO ATTESA COMANDI…` | parla con il motore della chat (`NYCT_FINO=regex` per fermarsi alla risposta) |
| `attendi_regex FILE REGEX [sec]` | aspetta una riga in un log |

Ogni processo avviato va aggiunto a `PIDS` (`… & PIDS+=($!)`): la libreria lo chiude a
fine prova.
