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

## Da fare (in ordine)

1. ✅ **Approccio per la cipolla deciso** (sessione 2): Sphinx solo per aprire i percorsi, poi cifratura a strati simmetrica per pacchetto. Da riportare nella SPECIFICA §4.
2. **Tappa 1 — in corso:**
   - ✅ **1a — data-plane a strati simmetrici** (`crates/nyctalus-core/src/cipolla.rs`): `avvolgi`/`sbuccia` con ChaCha20, nonce = numero di sequenza, lunghezza costante, 5 test. Fatto sessione 2.
   - ⏳ **1b — apertura del circuito con Sphinx:** consegnare a ogni nodo la sua chiave di salto e l'indirizzo del nodo successivo.
   - ⏳ **1c — ID di circuito che cambia a ogni tratta** + difesa sul numero di sequenza in chiaro.
3. **Tappa 2:** programma nodo (`nyctalus nodo`) che toglie il suo strato e inoltra.
4. **Tappa 3:** prova completa sul PC, client → guard → medio → uscita → destinatario, verificando che nessun nodo conosca insieme mittente e destinazione.
5. Ragnatela vera: più percorsi medi paralleli dopo un guard fisso (SPECIFICA §4.2).
6. **Interfaccia a riga di comando (VISIONE §8.2), prioritaria perché sblocca sia terminale sia browser:**
   - ✅ **6a-i — protocollo SOCKS5** (`crates/nyctalus-core/src/socks5.rs`): analisi saluto + richiesta CONNECT (IPv4/IPv6/nome), riconoscimento `.nyct`, costruzione risposte; 8 test. Fatto sessione 2.
   - ⏳ **6a-ii — collegamento SOCKS5 al trasporto** nel demone (server TCP locale che usa questo modulo).
   - **6b — sottocomandi demone:** `nyctalus avvia` (demone + SOCKS5), `nyctalus stato`, `nyctalus sito ./cartella`, file di config `~/.config/nyctalus/config`.
   - **6c — strumenti:** `nyctalus monitor` (curses, stile `nyx`), `nyctalus esec <prog>` (stile `torsocks`).
7. Elenco dei nodi (SPECIFICA §9) e simulazione con Shadow.
8. Più avanti: camuffamento L0, rumore di fondo, ponte verso i `.onion` di Tor, browser pubblico basato su Firefox ESR.

## Per ripartire
```sh
cd "nyctalus protocol"
cargo test          # deve dare 29 test superati
```
