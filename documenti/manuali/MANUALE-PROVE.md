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
| `07-traffico` | **«Wireshark in mezzo»**: rete di prova in uno spazio di rete isolato, cattura di ogni pacchetto con dumpcap mentre Alice e Bob si scambiano messaggio, file e videochiamata con parole spia; in rete mai parole spia né indirizzi, nessun collegamento diretto, niente DNS/STUN/mDNS, TCP solo locale, pacchetti QUIC v1 indistinguibili da rumore; riporta dimensioni e ritmo (`NYCT_CATTURA=file.pcap` tiene la cattura per Wireshark) |
| `08-browser-pc` | **browser del PC in modalità 🦇**, in uno spazio di rete isolato con cattura: a rete non ancora pronta la scheda 🦇 non contatta il sito (pagina d'attesa), poi riparte da sola passando dal proxy, i nomi li risolve la rete (zero DNS nella cattura), il sito non riceve visite dirette; una scheda normale sì (controllo). Col browser precedente la prova falliva: la scheda 🦇 partiva in chiaro nei secondi d'avvio della rete |
| `05-orchestratore` | il servizio del telefono: niente accesso senza segreto (anche WebSocket), niente file di sistema (`/etc/passwd`), niente `../` fuori dalla web-app, niente comandi da altri siti, nomi caricati ripuliti, caricamenti enormi rifiutati |

## Prove di funzionamento (`test/funzionamento/`)

| Prova | Cosa verifica |
|---|---|
| `00-test-automatici` | tutti i test del motore (`cargo test`): crittografia, celle, conflux, rendezvous, chat |
| `01-chat-e-allegati` | su una **rete di prova** (4 nodi, circuiti a 3 salti): testo nei due sensi, identità autenticata, allegati da 3 MB e immagini **identici al byte** (SHA-256), avanzamento, cronologia; **tre allegati inviati insieme** a un contatto nuovo arrivano tutti (difetto dei collegamenti doppi); due contatti che si sono solo aggiunti si vedono **in linea senza scriversi** e l'**ultimo accesso** arriva nell'elenco |
| `02-orchestratore` | il servizio del telefono avvia rete e chat, ponte WebSocket verso il motore, caricamento e lettura a pezzi (Range), arresto senza processi orfani; **miniature** della galleria con Pillow, senza Pillow né ffmpeg (miniatura EXIF con orientamento) e mai la foto intera se grande; **galleria da 2000 foto** elencata in tempi brevi e tenuta in memoria |
| `03-webapp` | la web-app in Chrome a misura di telefono: elenco, cache IndexedDB, conversazione in fondo, pressione lunga, foto senza EXIF, **selettore allegati** (griglia, selezione numerata, «Invia (2)»), messaggi e foto a se stessi **attraverso la rete**; pulsanti della rete accesi/spenti secondo la fase; **chiamata e videochiamata** con microfono e telecamera FINTI di Chrome (Chrome muto: nessun suono dalle casse); temi Menta e Personalizzato (VP8 inviato e ridecodificato, riquadro trascinato/scambiato/ingrandito, VIDKEY, «Solo voce», videochiamata in arrivo); **⌨ Terminale** (#terminale): io e aiuto da nyctalus-cli, `$(…)` e `;` NON eseguiti, `avvia` rifiutato, `ascolta` fermato da ⏹, origine estranea rifiutata (403), collegamento in `~/.nyctalus-chat/controllo` (600) |
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
| `18-chiamate` | **chiamate, motore**: invito, risposta, 2 s di voce a pacchetti da 20 ms nei due sensi (identici, ritardo misurato), fine, invito rifiutato, invito ricordato per chi apre la chat dopo, id non valido rifiutato; **video**: invito video, VIDSTATO, immagini VP8, VIDKEY |
| `19-chiamata-pc` | **chiamata dalla chat GTK** con microfono, altoparlante e webcam DI PROVA: 📞, risposta, voce Opus nei due sensi, barra con durata e qualità, muto, volume, passaggio voce↔video nella finestra stile Telegram Web (VP8 nei due sensi, immagine completa su richiesta), riaggancia |
| `20-tema-globale` | **tema globale del PC**: mini menu e chat aperti insieme; tema scelto nel mini menu (⚙ → Tema) → la chat cambia colori al volo; scelto nella chat → il mini menu si ricolora; Menta e **Personalizzato** (colore + chiaro/scuro) arrivano a tutti (configurazione temporanea, finto motore) |
| `21-info-messaggio` | **«ℹ️ Info» del messaggio** nella chat del PC: dalle spunte del motore (con l'ora) quando un messaggio è stato messo in casella, consegnato e visualizzato; righe senza ora dei motori vecchi → «ora non disponibile» (finto motore) |
| `22-trasferimenti` | **programma File, motore**: Alice → Bob sulla rete di prova (3 salti, canali paralleli): sotto soglia parte da solo, sopra soglia «Accetta», pausa/ripresa, motore di Bob spento a metà e riacceso (riprende dai pezzi già verificati), file sempre identico (SHA-256), non-contatto rifiutato senza proporglielo; nella chat il file compare all'ora d'arrivo con ✓✓ e diventa blu alla lettura; con «inchat 0» non compare in chat. Motore di Bob rallentato solo in prova (`NYCTALUS_PROVA_RALLENTA_MS`) |
| `50-collaudo` | **nyctalus-collaudo** su una rete di prova camuffata (`--solo-rete`): configurazione, elenco firmato, 4 nodi vivi con i tempi, nodi muti senza chiave, tunnel temporaneo acceso e poi spento, rubrica `.nyct` raggiungibile, rapporto su file e JSON; un nodo spento segnalato per nome (e spiegato se era l'entrata fissa), due nodi spenti = non superato, elenco manomesso rifiutato, rete non configurata |
| `49-cassaforte-file-app` | **cassaforte, file ricevuti nelle app**: a cassaforte chiusa l'app non apre nulla (404); aperta, l'app del telefono serve foto e documento identici (anche a pezzi con Range) e la miniatura; miniature e copie in chiaro solo nella cartella temporanea; chat e File del PC aprono il file cifrato; alla chiusura le copie spariscono |
| `48-cassaforte-file` | **cassaforte, file ricevuti nel motore** (Alice → Bob su rete di prova): all'accensione i file già ricevuti vengono cifrati; allegati nuovi e file grande da 6 MB arrivano già cifrati (`NYCS1`); `APRI` dà una copia identica all'originale e rifiuta i percorsi fuori dai ricevuti; l'inoltro di un file cifrato arriva giusto |
| `47-cassaforte-pc` | **cassaforte nella chat del PC** (finestra GTK): richiesta della passphrase all'avvio, sbagliata rifiutata, giusta apre contatti e cronologia; «Protezione dei dati» accende, cambia e toglie |
| `46-cassaforte-webapp` | **cassaforte nell'app del telefono** (Chrome): il servizio parte con i dati cifrati, la chat chiede la passphrase, sbagliata rifiutata, giusta apre; Impostazioni › 💬 Chat mostra «Accesa» |
| `45-cassaforte` | **cassaforte, motore e nyctalus-cli**: `CIFRA` cifra identità, contatti, cronologia (niente più testo in chiaro sul disco); riavvio chiuso, `SBLOCCA` sbagliata/giusta, stesso Contact ID; `CAMBIA`, `DECIFRA`; un file manomesso non si apre |
| `44-circuiti-caduti` | **ripresa dai circuiti caduti**: un nodo intermedio spento a metà scaricamento, il file arriva comunque identico |
| `43-registrazione-provata` | **registrazione provata dei servizi `.nyct`**: senza prova, con prova falsa, scaduta o ripetuta il nodo rifiuta |
| `42-copertura-costante` | **copertura costante**: ritmo fisso nei due versi, da fermi e durante l'uso |
| `41-chat-mixnet` | **chat da mixnet**: buste trattenute da ogni nodo per un ritardo casuale e rimescolate |
| `40-uscite-diverse` | **uscite diverse** (`--exits N`): ogni sito sulla sua uscita, ogni uscita vede solo i suoi |
| `39-guardie-promosse` | **PC stabili promossi a guardia**: scelti dalla stabilità misurata, mai uscite |
| `38-forma-videochiamata` | **forma «videochiamata»** del traffico (`--webrtc`) e sonde mute |
| `37-script-unico-termux` | **lo script unico** `installa-termux.sh` eseguito come lo incolla l'utente (`curl … \| bash`) in una Termux simulata: pacchetto alterato rifiutato, installazione in un colpo, permesso all'app (una riga), impostazioni ricaricate, app riaperta, niente aggiornamento se Termux è pronto, rilanciabile |
| `36-ripiego-tcp` | **UDP bloccato**: il nodo ascolta anche in TCP; `NYCTALUS_TCP=1` scarica 1 MB identico in TCP; un ponte raggiungibile solo in TCP → il client passa **da solo** a TCP; una sonda TCP con byte a caso non riceve niente e il nodo non chiude |
| `35-aiuta-la-rete` | **nodi solo intermedi**: forzato come uscita il nodo rifiuta; 16 circuiti senza mai guardia/uscita sui PC degli utenti; navigazione; `nyctalus-rete aiuta` (servizio con `--middle-only --max-rate`) e `aggiungi --intermedio --banda` (ruolo e banda conservati) |
| `34-banda` | **banda dei nodi**: `nyctalus-rete banda`, la banda nell'elenco firmato (alterarla rompe la firma), il motore la legge; famiglia e banda insieme |
| `33-forma-http3` | **forma HTTP/3**: navigazione a 3 salti, siti `.nyct` e ponti con i nodi che sembrano un sito HTTP/3; senza contrassegno non si entra; un **vero Chrome** visita il nodo e riceve una pagina 404; certificato con dati normali; ripiego TCP come vero HTTPS (openssl, curl e Chrome vedono un sito con pagina 404); gestore (`--forma h3 --nome`), codice, PC, telefono, installatore (porta 443, `--nuova-identita`) |
| `32-ponte-nascosto` | **ponti nascosti**: si entra solo dal ponte (anche per i siti `.nyct`), il ponte non è nell'elenco e non entra chi non ha la sua chiave; **sonde**: un nodo senza camuffamento risponde, nodi camuffati e ponte mai; gestore del PC (`ponte aggiungi/elenco/togli`, file 600) e API del telefono (mai le chiavi alla pagina) |
| `31-servizio-molti-client` | un servizio `.nyct` serve **8 client collegati insieme e a lungo** (come la rubrica con le sessioni della chat): prima il limite era 4 (controprova: con il motore vecchio 4 su 8) |
| `30-impronta-browser` | **impronta uniforme** delle schede 🦇 del browser del PC: una pagina misura identità, lingua (anche nell'intestazione), ora, schermo, processori, WebGL, Web Audio, WebRTC, canvas, funzioni «native» e cornici; due avvii con fuso (Roma / New York) e lingua (it / en-GB) diversi danno un'impronta IDENTICA |
| `29-firefox` | **Nyctalus Firefox** con il Firefox vero, senza finestra (headless), su una rete di prova: profilo 700 con proxy anche per i nomi, mai diretto, `resistFingerprinting`, WebRTC spento; la pagina arriva attraverso 3 salti; dentro la pagina niente WebRTC, ora UTC, lingua en-US, niente WebGL; a rete spenta non esce niente; senza rete lo dice e non apre Firefox |
| `28-protezione` | **protezione di serie** (Tappa 4): rete nuova con camuffamento e rumore nel codice (chiave in file 600), 4 nodi camuffati, client con la chiave naviga a 3 salti, client senza chiave respinto, rubrica e casella camuffate, `importa` (PC) e orchestratore (telefono/Windows) prendono chiave e rumore, installatore del nodo con `--codice`, `protezione no/si` (stessa chiave), rumore fuori misura rifiutato |
| `27-nodi-ospiti` | **nodi di persone diverse** (Tappa 3): rete di casa + nodi di «mario» (2) e «luca» (1) con `nyctalus-rete ospite`; elenco pubblicato subito e firmato con famiglie (i nodi del PC = «casa»), famiglia «casa» vietata agli ospiti; 30 percorsi sempre di 3 gestori diversi; traffico vero attraverso nodi ospiti; togli; ospiti ancora presenti dopo il riavvio; senza ospiti elenco compatibile coi client vecchi |
| `26-riservata` | **modalità riservata** sulla rete di casa con rubrica e casella (ritmo e ritiro accorciati solo in prova): RISERVATA 1 ricordata su disco, il messaggio va in coda e arriva dalla casella identico (riempitivo tolto) e autenticato, 📮 per chi scrive, coda svuotata; nessuna ricevuta mandata; buste finte spedite e tornate; la rubrica non consegna subito a chi è riservato; buste vere e finte tutte della stessa misura; testo troppo lungo rifiutato; RISERVATA 0; rete senza casella → non disponibile; `nyctalus-cli impostazioni riservata si/no` |
| `25-percorsi` | **scelta dei percorsi** (Tappa 2) con il motore vero e un elenco firmato con famiglie: in 25 avvii × 3 circuiti la guardia è sempre la stessa (file privato 600, cartella 700), l'uscita cambia, mai due nodi della stessa famiglia; famiglia tolta → firma rotta; regola impossibile → avviso «NON anonimo», con `NYCTALUS_PARENTELA=rigida` rifiuto; niente scritto in `~/.nyctalus` |
| `24-cli` | **nyctalus-cli** fra Alice e Bob sulla rete di prova, solo da comando: io, aggiungi, contatti, scrivi (nome abbreviato, accenti, emoji), `ascolta` in diretta, leggi, invia (file identico), file, impostazioni, file sopra soglia visto in `ascolta` e accettato, pulisci, errori chiari (contatto inesistente, segreto sbagliato, motore spento); `avvia` senza finestre in una rete isolata (user namespace, HOME finto): segreto in file 600, niente secondo motore, spegnimento pulito; ⌨ Terminale del PC (`avvia-terminale.sh --dentro`) che accende e rispegne il motore |
| `23-file-pc` | **programma File del PC** (finestra GTK) sopra il motore di Bob: un file sotto soglia arriva da solo, uno sopra mostra «Rifiuta/Accetta»; toccato «Accetta» arriva identico, righe «Arrivato» con «Apri» e «Mostra nella cartella»; file cancellato → «Eliminato dalla cartella»; ✕ Togli e Pulisci elenco (i file ricevuti restano) |
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
