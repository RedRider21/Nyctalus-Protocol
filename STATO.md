# Nyctalus Protocol — Stato dei lavori

> Aggiornato al 27 settembre 2026, fine della prima sessione di sviluppo.

## Fatto ✅

| Cosa | Dove |
|---|---|
| Documento di visione v0.2 (nome, principi, ruoli dei nodi, browser, rapporto con NexusSec) | `VISIONE.md` |
| Specifica tecnica v0.2 (modello di minaccia, livelli L0–L3, formati) | `SPECIFICA.md` |
| Etichette segrete rotanti (BLAKE3), consumate solo dopo l'autenticazione | `crates/nyctalus-core/src/etichette.rs` |
| Cifratura end-to-end dei frammenti (ChaCha20-Poly1305), pacchetti fissi da 1200 byte | `crates/nyctalus-core/src/cifratura.rs` |
| Ricomposizione con limiti anti-DoS | `crates/nyctalus-core/src/ricomposizione.rs` |
| Flusso mittente/ricevitore | `crates/nyctalus-core/src/flusso.rs` |
| Stretta di mano Noise NK (mittente anonimo, forward secrecy) | `crates/nyctalus-core/src/stretta.rs` |
| Indirizzi `.nyct` (chiave pubblica in base32 con controllo anti-errori) | `crates/nyctalus-core/src/indirizzo.rs` |
| Programma di prova `nyctalus ricevi / invia`: QUIC a corsie parallele, pinning del certificato, controllo del flusso | `crates/nyctalus/` |

**Verifiche:** 29 test su 29 superati, clippy pulito. Trasferimento di 100–200 MB sullo stesso PC a circa 110–127 MB/s, con file identici e 0 pacchetti scartati.

**Ambiente:** Rust 1.98 (rustup, profilo minimal, ~620 MB). La cartella `target/` pesa ~500 MB: `cargo clean` la svuota. Il disco è al 95%.

## Dove ci siamo fermati: il livello a cipolla (Sphinx)

Ho misurato la libreria Sphinx di Nym (`sphinx-packet` 0.8), che è collaudata e usata in produzione:

| Misura | Valore |
|---|---|
| Intestazione Sphinx (fino a 5 salti) | 348 byte |
| Spazio utile in un pacchetto da 1200 byte | 852 byte (meno 27 byte del nostro L2 = ~825) |
| Costruzione dei pacchetti (client, 1 core) | ~4.150 pacchetti/s |
| Apertura di uno strato (nodo, 1 core) | ~13.200 pacchetti/s ≈ 16 MB/s |

**Conclusione:** usare Sphinx su **ogni** pacchetto è troppo lento per l'obiettivo "più veloce di Tor", perché ogni pacchetto richiede un'operazione a chiave pubblica per ogni nodo.

**Proposta da decidere all'inizio della prossima sessione:**
- **Sphinx solo per aprire i percorsi:** un pacchetto Sphinx per percorso porta a ogni nodo la sua chiave di salto.
- **Poi cifratura a strati solo simmetrica per pacchetto** (ChaCha20, veloce come il nostro L2), con un identificativo di circuito che cambia a ogni salto.

È lo stesso principio di Tor e del progetto di ricerca HORNET: anonimato equivalente, velocità di un ordine di grandezza superiore.

## Scaletta (checklist autorevole)

### Fatto ✅
- **1a** data-plane a strati simmetrici (`cipolla.rs`).
- **1b** apertura circuito con Sphinx (`circuito.rs`).
- **Tappa 2** programma nodo relè (`nodo.rs`) + `prova-circuito`.
- **Tappa 3** navigazione anonima a 3 salti: uscita verso Internet + `avvia-circuito` (SOCKS5 onion); provato con curl.
- **6a** protocollo SOCKS5 (`socks5.rs`) + proxy a un salto (`avvia.rs`/`uscita.rs`).
- Documenti (VISIONE/SPECIFICA/STATO/README), pubblicazione GitHub + Pages + logo.

### Da fare (in ordine)
- [x] **A. Elenco firmato dei nodi (directory).** ✅ `elenco.rs` (core, autorità Ed25519, firma/verifica, 4 test) + comandi `nyctalus autorita` / `nyctalus elenco`. Client e nodi leggono l'elenco con `--elenco FILE --autorita HEX`; elenco di autorità non fidata/manomesso rifiutato.
- [x] **B. Autenticazione dei collegamenti.** ✅ Con l'elenco: il client pinna il guard (impronta TLS), ogni relè pinna il nodo a valle (cerca l'indirizzo nell'elenco firmato → impronta attesa, `tls::config_client`). Provato a 3 salti: traffico OK e nodo con «autenticazione a valle ATTIVA». Senza elenco resta il fallback `insicuro` (prototipo).
- [x] **C. Anti-correlazione (1c).** ✅ Rimosso il numero di sequenza dal filo: il nonce degli strati a cipolla è un **contatore locale** per direzione (lo stream QUIC è ordinato), così lo stesso pacchetto non mostra valori comuni fra le tratte. Formato quadro ora `[len][payload]`, 1º quadro = apertura Sphinx. Provato: 300 KB integri su circuito autenticato a 3 salti. L'**ID di circuito esplicito per tratta** arriverà con la multiplazione (punto D).
- [x] **D. Circuito condiviso.** ✅ Un solo circuito (una sola apertura Sphinx) trasporta più connessioni, multiplexate con celle `[flusso][comando][dati]` dentro gli strati a cipolla (`cella.rs`); i relè restano ciechi ai flussi. `avvia-circuito` è ora un demone con un circuito condiviso + task scrittore/lettore. Provato: 2 curl in parallelo (250KB+180KB) integri sullo stesso circuito. (L'ID di circuito esplicito per tratta, per condividere i link fra nodi, resta per l'ottimizzazione del trasporto.)
- [x] **E. Ragnatela multipercorso.**
  - [x] **E1 — più circuiti paralleli** (guard condiviso, percorsi diversi) con assegnazione delle connessioni a rotazione: connessioni concorrenti su strade diverse. Opzioni `--circuiti N` (1–8, default 2) e `--salti N` (1–5, default 3). Provato: 4 download paralleli su 2 circuiti con percorsi diversi, tutti integri.
  - [x] **E2 — conflux** (fatto): circuiti con **guard e uscita condivisi** (intermedi diversi); le celle di ogni flusso sono numerate con una sequenza end-to-end dentro l'onion e spezzate sui circuiti; l'uscita le **ricompone** (conflux.rs + registro dei gruppi all'uscita, LINK=set_id). Provato: un download da 600 KB spezzato su 2 circuiti torna integro; 2 download paralleli integri.
- [~] **F. 6b — demone da configurazione.** ✅ `start` (demone SOCKS5 conflux da config), `status` (mostra config + valida l'elenco), file `~/.config/nyctalus/config` (chiave=valore, flag che sovrascrivono). Fatto.
- [~] **I2. Servizi interni `.nyct` + punto d'incontro (SPECIFICA §6).** Prima versione funzionante (slice 1, `incontro.rs`):
  - [x] **`site --dir ./cartella --rendezvous IP:PORT --fingerprint FP [--identity FILE]`**: ospita una cartella come sito `.nyct`. Il servizio si registra presso un **nodo punto d'incontro** con la sua chiave pubblica (il `.nyct`), poi serve i file via HTTP su una sessione **Noise NK end-to-end**.
  - [x] **`browse ADDRESS.nyct --rendezvous IP:PORT --fingerprint FP [--path /]`**: visita un sito `.nyct`. Il client chiede al nodo di essere collegato al servizio; il nodo apre un nuovo stream e fa da **ponte opaco** (byte grezzi); client e servizio fanno Noise NK (l'indirizzo `.nyct` *è* la chiave) e sopra viaggia HTTP. Il nodo non vede il contenuto; il client non conosce l'IP del servizio e viceversa.
  - Record E2E ChaCha20-Poly1305 (chiave per direzione dai segreti Noise, nonce a contatore); content-type per estensione; blocco della traversata di cartella. Provato end-to-end: `index.html` servito (200) e cifrato, 404 per i file mancanti. Marca `NYCTRDV1` sul 1º quadro per distinguere il controllo rendezvous dall'apertura Sphinx del circuito.
  - [x] **(a) Selezione deterministica** del punto d'incontro ✅ (`rendezvous.rs` nel core): servizio e client calcolano lo **stesso ordine** di nodi candidati da `BLAKE3(chiave_servizio ‖ epoca ‖ indice)` sull'elenco firmato. Il sito si registra al primo nodo raggiungibile; il client prova i candidati in ordine (e anche l'epoca precedente, per il cambio di giorno). Niente più `--rendezvous` a mano (resta come override). L'epoca (il giorno) fa **ruotare** il punto d'incontro nel tempo. Provato: sito e `browse` senza `--rendezvous` si incontrano sullo stesso nodo scelto in automatico.
  - [x] **(c) Integrazione nel proxy SOCKS** ✅: il demone (`start`/`circuit`) riconosce gli host `.nyct` (`Destinazione::e_interna`) e invece dell'uscita del circuito apre un **tunnel al punto d'incontro** con sessione Noise NK end-to-end (`incontro::tunnel_socks`), facendo da ponte byte-per-byte fra il browser e il servizio. I punti d'incontro candidati sono gli stessi nodi dell'elenco firmato che alimentano i circuiti. **Il Nyctalus Browser (stealth) apre `http://<addr>.nyct` e renderizza HTML+CSS+JS** (validato a schermo, rendezvous onion a 3 salti). Tre correzioni dal test col browser vero: i `.nyct` esentati dal solo-HTTPS (come gli `.onion`), schema proxy `socks5://` (DNS remoto), e **keepalive QUIC** sui circuiti client (ping 8s) perché i circuiti registrati in attesa non cadano per inattività.
  - [x] **(d, parziale) Keep-alive** ✅: il servizio è ora un server HTTP in loop sulla sessione cifrata (legge richieste a record, bufferizza fino a `\r\n\r\n`, risponde, ripete finché il client chiude; `Connection: keep-alive`/`close` rispettati, tetto anti-DoS sull'intestazione). Una pagina con più risorse si scarica su una stessa connessione.
  - [x] **(b) Rendezvous a più salti (uscita onion)** ✅: client e servizio raggiungono il punto d'incontro con un **circuito a più salti** (`--hops`, default 3), come l'uscita verso Internet. L'uscita del circuito, tolto il proprio strato, riconosce il controllo (marca `NYCTRDV1`) e **cuce i due circuiti** relè-a-relè (`uscita_rendezvous` + splice a canali; nonce di andata da 1, di ritorno con "OK"/"NO" come frame 0). Astrazione `Canale` (diretto **o** onion) così Noise/HTTP restano un solo codice; registro a coda + pool di registrazioni per servire più client in parallelo. **Il punto d'incontro vede solo l'ultimo relè, non gli IP di client e servizio.** Il `--rendezvous` manuale resta come modo diretto a salto singolo (per prove con un nodo). Provato a 3 salti: `browse` (HTML + asset), `curl` via SOCKS, tutto cifrato E2E.
  - [x] **(d) Rifiniture** ✅: il servizio consuma il **corpo della richiesta** (`Content-Length`) così il keep-alive resta allineato anche con POST; il client (`browse`) legge la risposta **per Content-Length** (chiusura pulita, niente dipendenza dal momento di chiusura); **web server esterno** dietro al demone con `site --backend IP:PORT` (proxy byte-per-byte verso Nginx/Apache o qualunque server HTTP: gestisce lui GET/POST/keep-alive/pipeline); **pool** di registrazioni regolabile con `--pool N` (default 4). Provato: sito `.nyct` servito da `python -m http.server` esterno, visitato via `browse` a 3 salti.
- [x] **CLI in inglese.** Comandi e opzioni rinominati in inglese (start, status, nodes, node, authority, directory, exit, send, receive, circuit, test-circuit, single-hop; --listen, --hops, --circuits, --directory, --authority, --to, --fingerprint, --recipient, --out, --identity, --lanes, --entries, --probe, --timeout). Commenti del codice in italiano, sito bilingue.
- [x] **Comando `nodes`.** ✅ Mostra i nodi dell'elenco firmato (indirizzo, X25519, impronta TLS) e, con `--probe`, verifica quali sono **raggiungibili ora** aprendo verso ciascuno la connessione QUIC autenticata (impronta pinnata), misurando il tempo di risposta. Serve a scegliere un **target** per `single-hop`/`send`/`circuit` (i valori stampati si incollano direttamente). Provato: 2 nodi accesi → `online` con RTT; spenti → `offline (timeout)`. Modulo `nodi_online.rs` + flag booleani nel parser (`--probe`).
- [x] **G. 6c — strumenti.** ✅ `exec` (stile torsocks: imposta le variabili proxy e lancia un programma; provato con curl) e `monitor` (cruscotto testuale che legge una porta di controllo del demone con metriche circuits/flows/up/down/uptime; aggiornato ogni secondo). Il demone `start` espone la porta di controllo (`--control`, default 127.0.0.1:1090).
- [~] **H. Camuffamento + rumore di fondo.**
  - [x] **H1 — offuscamento del trasporto** ✅: ogni datagramma UDP è avvolto con ChaCha20 e una chiave di rete condivisa (`offuscamento.rs` nel core; `trasporto.rs` avvolge il socket UDP di quinn). Sul filo i byte appaiono casuali: nessuna firma QUIC per il DPI. Opzione `--obfuscation HEX` (nodi, start, circuit) o `obfuscation` nel config. Provato: con la chiave giusta funziona, senza fallisce. Limiti: niente resistenza al probing attivo né shaping dei tempi.
  - [~] **H2 — rumore di fondo (cover traffic):** primo slice fatto.
    - [x] **Celle a lunghezza fissa** (`cella.rs`: `LUNGHEZZA` = 5 + `CAPACITA` 1024, campo lunghezza interno + riempimento a zero): tutte le celle hanno la stessa dimensione sul filo, così un osservatore non deduce la lunghezza dei contenuti **e** le celle fittizie sono indistinguibili dalle vere. I lettori (`avvia_circuito`, `nodo`) spezzano i dati a `CAPACITA`. Provato: 270 KB trasferiti integri (SHA256) attraverso ~270 celle fisse.
    - [x] **Celle fittizie (cover traffic) in salita** (client→uscita): comando `cmd::RIEMPIMENTO`, iniettate a ritmo di **Poisson** (demone `--cover HZ`, default 0) nel flusso in uscita; l'uscita le scarta, il collettore di ritorno le salta.
    - [x] **Cover traffic in discesa** (uscita→client): il nodo (`node --cover HZ`) inietta celle fittizie nel ritorno del gruppo conflux, scartate dal client. Insieme è stato sistemato il **ciclo di vita** dell'uscita conflux: gli scrittori di ritorno si tolgono dalla lista quando il circuito cade e lo stato del gruppo si rimuove dal registro a gruppo esaurito (prima restava e lo schedulatore girava a vuoto). Provato: trasferimento di 270 KB integro con cover **bidirezionale** (nodi + demone a 30 celle/s).
    - [x] **Riempimento del LINK**: il 1º quadro conflux (set_id) è ora riempito alla stessa dimensione di un'unità dati (`conflux::quadro_link`, `LUNGHEZZA_UNITA`), così non si distingue per lunghezza; il destinatario legge i primi 16 byte e ignora il resto.
    - [ ] Da fare: riempimento del **pacchetto di apertura Sphinx** (formato proprio, più delicato) e uniformità dei quadri del **tunnel rendezvous** (Noise/HTTP, sottosistema separato); **taratura** del ritmo con misure reali (costo banda vs beneficio).
- [x] **I. Ponte verso i `.onion` di Tor.** ✅ L'uscita riconosce gli indirizzi `.onion` e li raggiunge tramite il **Tor già presente sul sistema** (proxy SOCKS, default 127.0.0.1:9050, opzione `--tor-socks`), con un tunnel SOCKS5 — niente dipendenza pesante (no Arti incorporato), build leggera, riusa il Tor di NexusSec. `ponte.rs`: e_onion + tunnel_tor. Provato con un finto Tor: `.onion` instradato, siti normali in TCP diretto.
- [ ] **J. Browser pubblico** basato su Firefox ESR (multipiattaforma).
- [ ] **K. Simulazione con Shadow** e confronto misurato con Tor.
- [~] **Distribuzione (eseguibili multi-SO).** CI GitHub Actions pronta e **verificata** (compila Linux x86_64/arm64, Windows x86_64, macOS Intel/Apple Silicon); su un tag `v*` produce i binari come Release. **Pubblicazione rimandata**: non si distribuisce un binario di anonimato non ancora verificato.

## Per ripartire
```sh
cd "nyctalus protocol"
cargo test          # deve dare 29 test superati
```

## Pubblicazione (sessione 2)
- **Repo GitHub:** https://github.com/RedRider21/Nyctalus-Protocol (pubblico, RedRider21, AGPL-3.0, topics, homepage=Pages). Sorgenti su `main`; i prossimi avanzamenti si caricano via via.
- **Sito/Pages:** https://redrider21.github.io/Nyctalus-Protocol/ — branch `gh-pages` (repo git separato in scratchpad, NON nella cartella progetto). Sito **bilingue IT/EN** + **tema chiaro/scuro** (toggle, persistiti in localStorage), responsivo, con manuale. File: index.html, stile.css, logo.svg, .nojekyll.
- **Logo:** `logo.svg` (nottola in distintivo circolare, onde ecolocalizzazione, nodi alle ali); nel repo e in cima al README.
