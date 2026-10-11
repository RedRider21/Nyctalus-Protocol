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
- [~] **F. 6b — demone da configurazione.** ✅ `start` (demone SOCKS5 conflux da config), `status` (mostra config + valida l'elenco), file `~/.config/nyctalus/config` (chiave=valore, flag che sovrascrivono). Fatto. ⏳ Resta `sito ./cartella` (pubblicare un `.nyct`): richiede i servizi nascosti/rendezvous (SPECIFICA §6), non ancora implementati.
- [x] **CLI in inglese.** Comandi e opzioni rinominati in inglese (start, status, nodes, node, authority, directory, exit, send, receive, circuit, test-circuit, single-hop; --listen, --hops, --circuits, --directory, --authority, --to, --fingerprint, --recipient, --out, --identity, --lanes, --entries, --probe, --timeout). Commenti del codice in italiano, sito bilingue.
- [x] **Comando `nodes`.** ✅ Mostra i nodi dell'elenco firmato (indirizzo, X25519, impronta TLS) e, con `--probe`, verifica quali sono **raggiungibili ora** aprendo verso ciascuno la connessione QUIC autenticata (impronta pinnata), misurando il tempo di risposta. Serve a scegliere un **target** per `single-hop`/`send`/`circuit` (i valori stampati si incollano direttamente). Provato: 2 nodi accesi → `online` con RTT; spenti → `offline (timeout)`. Modulo `nodi_online.rs` + flag booleani nel parser (`--probe`).
- [x] **G. 6c — strumenti.** ✅ `exec` (stile torsocks: imposta le variabili proxy e lancia un programma; provato con curl) e `monitor` (cruscotto testuale che legge una porta di controllo del demone con metriche circuits/flows/up/down/uptime; aggiornato ogni secondo). Il demone `start` espone la porta di controllo (`--control`, default 127.0.0.1:1090).
- [~] **H. Camuffamento + rumore di fondo.**
  - [x] **H1 — offuscamento del trasporto** ✅: ogni datagramma UDP è avvolto con ChaCha20 e una chiave di rete condivisa (`offuscamento.rs` nel core; `trasporto.rs` avvolge il socket UDP di quinn). Sul filo i byte appaiono casuali: nessuna firma QUIC per il DPI. Opzione `--obfuscation HEX` (nodi, start, circuit) o `obfuscation` nel config. Provato: con la chiave giusta funziona, senza fallisce. Limiti: niente resistenza al probing attivo né shaping dei tempi.
  - [ ] **H2 — rumore di fondo (cover traffic):** pacchetti fittizi per nascondere i tempi; va progettato con cura (costo in banda vs beneficio).
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
