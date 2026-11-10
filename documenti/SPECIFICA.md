# Nyctalus Protocol — Specifica tecnica v0

> Versione 0.3 · 27 settembre 2026 · Stato: bozza di lavoro
> Complemento tecnico di [VISIONE.md](VISIONE.md). Legenda:
> ✅ implementato e testato · 📐 progettato, da implementare · ❓ da decidere

---

## 1. Modello di minaccia

### 1.1 Avversari considerati

| Sigla | Avversario | Cosa può fare | Copertura |
|---|---|---|---|
| **A1** | Osservatore locale (provider, Wi-Fi pubblico, datore di lavoro) | Vede tutto il traffico in entrata e uscita dal dispositivo dell'utente | ✅ Pieno obiettivo |
| **A2** | Sito o servizio di destinazione | Vede le richieste che gli arrivano | ✅ Pieno obiettivo |
| **A3** | Nodi malevoli (una frazione minoritaria della rete) | Leggono, ritardano, scartano, duplicano o falsificano i pacchetti che transitano | ✅ Pieno obiettivo |
| **A4** | Censore attivo | Blocca protocolli riconoscibili, sonda i server sospetti | 📐 Obiettivo (camuffamento, §8) |
| **A5** | Avversario globale passivo | Osserva contemporaneamente ingresso e uscita della rete | ⚠️ Protezione pari a Tor, non superiore (vedi VISIONE §10) |

### 1.2 Proprietà garantite

1. **Riservatezza del contenuto:** solo gli estremi di un flusso leggono i dati.
2. **Integrità:** ogni alterazione di un pacchetto viene rilevata e il pacchetto scartato.
3. **Non collegabilità:** nessun singolo nodo conosce insieme l'origine e la destinazione.
4. **Uniformità:** tutti i pacchetti hanno la stessa lunghezza; etichette e contenuto sono indistinguibili da byte casuali.
5. **Resistenza di base al DoS:** la memoria usata dal destinatario per ogni flusso ha un limite fisso.

### 1.3 Fuori dal modello
- Un dispositivo dell'utente già compromesso (malware).
- Un utente che si identifica da solo (login, dati personali).
- Il fingerprinting del browser: è compito del browser dedicato, non del protocollo.

---

## 2. Architettura a livelli

```
 L3  Applicazione     proxy SOCKS5 locale · siti interni · browser         📐
 L2  Flusso           etichette + cifratura end-to-end + ricomposizione    ✅
 L1  Cipolla          pacchetti Sphinx, un strato per ogni nodo            📐
 L0  Collegamento     QUIC tra nodi adiacenti, camuffato da HTTPS          ✅ QUIC · 📐 camuffamento
```

- **L0** protegge il singolo "salto" tra due nodi vicini e lo fa sembrare traffico web normale.
- **L1** fa sì che ogni nodo sappia solo da dove arriva il pacchetto e a chi passarlo.
- **L2** protegge i dati da un estremo all'altro del flusso e li ricompone.
- **L3** è ciò che vedono le applicazioni.

---

## 3. Livello flusso (L2) ✅

Codice: `motore/nyctalus-core/src/` — moduli `chiavi`, `etichette`, `cifratura`, `ricomposizione`, `flusso`.

### 3.1 Flussi
Un **flusso** è una sequenza ordinata di byte che va **in una sola direzione**. Una conversazione usa due flussi con `id_flusso` diversi, uno per direzione. Ogni flusso ha:
- un `segreto_condiviso` di 32 byte, noto solo ai due estremi (§3.2);
- un `id_flusso` di 64 bit;
- una `dimensione_frammento` D (byte utili per pacchetto);
- una `finestra` W (quanti frammenti possono essere in volo oltre quello atteso).

Valori predefiniti: **D = 1173** (pacchetto da 1200 byte) e **W = 512**.

### 3.2 Derivazione delle chiavi ✅
```
chiave_etichette = BLAKE3-derive_key("Nyctalus v0 2026-09-27 etichette dei frammenti", segreto ‖ id_flusso_LE64)
chiave_cifratura = BLAKE3-derive_key("Nyctalus v0 2026-09-27 cifratura dei frammenti", segreto ‖ id_flusso_LE64)
```
Contesti diversi producono chiavi indipendenti. Il contesto contiene la versione: ogni cambio di formato cambia le chiavi.

✅ Il `segreto_condiviso` arriva dalla stretta di mano **Noise NK** (§5.1).

### 3.3 Etichette ✅
```
etichetta(i) = primi 8 byte di BLAKE3-keyed(chiave_etichette, i_LE64)
```
- Il destinatario pre-calcola le etichette degli indici `[base, base + W)` in una tabella etichetta → indice.
- **Riconoscimento in due tempi:** `cerca` trova l'indice senza modificare nulla; `consuma` toglie l'etichetta dalla tabella **solo dopo** che il pacchetto ha superato l'autenticazione (§3.4). In questo modo:
  - un replay non viene riconosciuto;
  - un nodo che ha visto passare un'etichetta non può "bruciarla" inviando per primo un pacchetto falso.
- La finestra avanza quando il ricompositore consegna i dati.
- Con 8 byte e W = 512, la probabilità che un pacchetto estraneo venga scambiato per uno del flusso è circa 512 / 2⁶⁴ ≈ 3 · 10⁻¹⁷. In quel caso comunque l'autenticazione lo scarta.

### 3.3-bis Anti-correlazione del nonce ✅
Nel circuito (stream QUIC ordinato) il numero di sequenza usato come nonce **non viaggia in chiaro**: ogni nodo lo ricostruisce con un contatore locale per direzione (bit alto = ritorno). Così lo stesso pacchetto non espone alcun valore comune fra una tratta e l'altra. L'ID di circuito esplicito per tratta arriverà con la multiplazione (§ multiplexing, punto D).

### 3.3-ter Multiplazione dei flussi (punto D) ✅
Un **solo circuito** trasporta più connessioni dell'applicazione. Dentro gli strati a cipolla viaggiano **celle** `[flusso u16][comando u8][dati]` (`motore/nyctalus/src/cella.rs`): `APRI host:porta`, `DATI`, `CHIUDI`, e in ritorno `APERTO`/`RIFIUTATO`. Solo client e uscita interpretano le celle; i relè vedono un unico canale di byte cifrati e non distinguono i flussi. Lato client un task scrittore serializza l'andata (nonce unico) e un lettore smista il ritorno al flusso giusto; l'uscita apre una connessione TCP per flusso.

### 3.3-quater Conflux: splitting su più strade (punto E2) ✅
Più circuiti che condividono **guard e uscita** (intermedi diversi) formano un *gruppo* identificato da un `set_id` da 16 byte, comunicato all'uscita nel primo quadro (LINK, dentro l'onion). Ogni cella riceve una **sequenza end-to-end** a 8 byte (dentro l'onion, invisibile ai relè) e viene spezzata sui circuiti; il ricevitore la **ricompone** nell'ordine originale (`motore/nyctalus/src/conflux.rs`, `Riordina`). Limiti del prototipo: guard/uscita fissi per la sessione, nessuna gestione della morte di un circuito.

**Celle piene (0.3.12):** una cella (di lunghezza fissa) parte quando è piena o dopo 2 ms di attesa (`cella::leggi_per_cella`): prima ogni lettura dal sito o dall'applicazione produceva una cella, anche quasi vuota, sprecando banda (misurato: 107 MB spediti dai nodi contro 79 per gli stessi 21 MB).

**Collegamenti fra nodi (0.3.11):** fra due nodi vicini un solo collegamento QUIC riusato da tutti i circuiti (una corsia ciascuno; le corsie QUIC sono indipendenti, a differenza del TCP di Tor). **Nodi solo intermedi** (`--middle-only`, `ruolo=intermedio`): mai uscita, guardia o punto d'incontro. **Guardie promosse (0.3.19):** `ruolo=intermedio guardia=si` (campo firmato; senza `ruolo=intermedio` è ignorato): candidato guardia, mai uscita né punto d'incontro; candidati guardia = nodi pieni ∪ promossi. **Uscite diverse (0.3.19):** il client può aprire fino a 4 fasci conflux (set_id distinti) sulla stessa connessione verso la guardia, ognuno con un'uscita diversa (mai la guardia, mai un intermedio); ogni destinazione (nome o IP) resta legata al suo fascio. **Forma videochiamata (0.3.19):** ogni datagramma = RTP v2 (PT 96, SSRC per flusso, orologio 90 kHz) + carico ChaCha20 (nonce = SSRC‖ts‖seq) + etichetta 10 byte (BLAKE3 con chiave derivata dalla chiave di rete); STUN Binding con MESSAGE-INTEGRITY (HMAC-SHA1, password ICE derivata) e FINGERPRINT; QUIC con datagrammi da 1200 byte. **Chat da mixnet (0.3.19):** ritardo per salto nell'indirizzo Sphinx del salto successivo (IPv4: byte 7 = `M`, byte 8-9 = ms BE; per chiave: tipo 7 = 29 byte di chiave + 2 byte di ms), letto solo dal nodo che sbuccia lo strato; il nodo trattiene ogni cella in andata per quel tempo dal suo arrivo (coda FIFO), massimo 20 s. Richiesto dal client SOCKS5 con nome utente `mix` (RFC 1929; la password è ignorata). **Copertura costante (0.3.19):** cella di controllo `COSTANTE` (0x05, flusso 0, dati = ritmo u16 BE + modo 1 uso / 2 sempre) dal client all'uscita del gruppo conflux; in entrambi i versi un regolatore fa uscire a ogni 1/R s una cella vera o una RIEMPIMENTO (5-2000 celle/s).

**Distributore (0.3.10):** ogni cella va al circuito con la **coda più libera** (`scegli_strada`), non più a rotazione rigida, che rendeva la ragnatela lenta come «N × la strada più lenta»; la finestra QUIC per flusso è di 256 KB (quinn: 1,25 MB) così una strada lenta si sente subito. **Scelta dei nodi pesata per banda** (`banda=` nell'elenco firmato). **Ragnatela codificata** sperimentale (`codifica.rs`: k originali + r riserve MDS di Cauchy su GF(2^8), chiesta nel riempimento del LINK con la marca `NYCTCOD1`, le uscite vecchie la ignorano; rumore di fondo dell'uscita come riserve utili): misurata, non accelera perché ogni tratta QUIC recupera già i pacchetti persi e le strade condividono entrata e uscita. Misure: `simulazione/LEGGIMI.md`.

### 3.4 Formato del pacchetto ✅
```
 0        8                                                  8+3+D     8+3+D+16
 +--------+----------------------------------------------------+---------+
 |etichetta|  C = ChaCha20( segnali ‖ lunghezza ‖ dati ‖ 0… )  | tag MAC |
 +--------+----------------------------------------------------+---------+
   8 byte     1 byte     2 byte LE    ≤ D byte  riempimento      16 byte
```
- **AEAD:** ChaCha20-Poly1305 (RFC 8439), chiave `chiave_cifratura`.
- **Nonce (12 byte):** `i_LE64 ‖ 00 00 00 00`. È unico perché ogni flusso ha la sua chiave e ogni indice si usa una sola volta.
- **Dati associati (AAD):** l'etichetta. Resta in chiaro ma è autenticata.
- **Segnali:** bit 0 = ultimo frammento del flusso. Gli altri bit devono valere 0, altrimenti `ContenutoMalformato`.
- **Lunghezza:** byte utili reali, al massimo D.
- **Lunghezza totale fissa:** `8 + 3 + D + 16` byte per tutti i pacchetti del flusso.

Il segnale "ultimo" e la lunghezza reale sono **dentro** la parte cifrata: i nodi non vedono né dove finisce un flusso né quanti dati porta ogni pacchetto.

### 3.5 Ricomposizione ✅
- Buffer ordinato per indice; consegna tutti i dati contigui a partire dal prossimo indice atteso.
- Errori segnalati (non ignorati), così il livello superiore può valutare i percorsi:
  - `Duplicato`
  - `FuoriFinestra` (indice ≥ atteso + W)
  - `MemoriaEsaurita` (limite di byte in attesa = D · W)
  - `OltreLaFine`
  - `FineIncoerente`
- La fine del flusso è il frammento marcato "ultimo". Un "totale frammenti" dichiarato in chiaro non viene mai accettato.

### 3.6 Limiti noti di L2
- **Forward secrecy:** L2 non ce l'ha di per sé, la eredita dalla stretta di mano Noise (§5.1).
- **Rinnovo delle chiavi:** 📐 un flusso va rinnovato con un nuovo `id_flusso` prima di 2³² frammenti, un margine molto prudente rispetto al limite teorico di 2⁶⁴.
- ❓ D andrà ricalcolato quando si aggiunge l'intestazione Sphinx (§4), perché il pacchetto finale sulla rete deve restare di dimensione fissa.

---

## 4. Livello cipolla (L1) 📐

### 4.1 Formato
Si adotta **Sphinx** (Danezis–Goldberg 2009), il formato usato da Nym e Lightning:
- a ogni salto il nodo toglie uno strato e scopre solo il nodo successivo;
- l'intestazione mantiene **lunghezza costante** a ogni salto, quindi non rivela a che punto del percorso si trova il pacchetto;
- i pacchetti di risposta si costruiscono senza conoscere il mittente.

### 4.2 Percorsi e ragnatela
**Navigazione su Internet:**
```
                ┌─► medio₁ ─┐
 client ─► guard ├─► medio₂ ─┼─► uscita ─► sito
                └─► medioₖ ─┘
```
- **Guard:** uno fisso per ogni client, ruotato ogni qualche settimana ❓. È l'unico nodo che vede l'IP del client.
- **Medi:** k percorsi paralleli (k predefinito tra 4 e 8 ❓). La ragnatela sta qui.
- **Uscita:** una per flusso. Ricompone L2 e parla con il sito.

**Siti interni:** i percorsi del client e quelli del sito convergono su un **punto d'incontro** (§6); nessuna uscita.

### 4.3 Scelta dei nodi
- **Fase 1:** casuale, pesata per banda misurata, con vincoli di diversità: nessuna coppia di nodi nello stesso sistema autonomo (AS) o /16 IPv4 nello stesso percorso.
- **Fase 3:** in più, preferenza per percorsi a bassa latenza. Si sceglie **a caso tra i percorsi "abbastanza veloci"**, non sempre il più veloce, e con coordinate di rete verificate.

### 4.4 Controllo del flusso multipercorso
✅ **Già nel programma di prova:** il destinatario conferma periodicamente `prossimo_atteso` (8 byte LE, ogni 64 frammenti e alla fine) e il mittente non invia mai l'indice i se i ≥ confermato + W. Così nessun frammento esce dalla finestra.

📐 Il client misurerà per ogni percorso RTT e perdite. I frammenti successivi vanno sui percorsi sani, e un percorso degradato viene sostituito. La finestra W limita il disordine massimo.

---

## 5. Stretta di mano e chiavi

### 5.1 Tra i due estremi (L2) ✅
Codice: `motore/nyctalus-core/src/stretta.rs`.

- **Schema:** `Noise_NK_25519_ChaChaPoly_BLAKE2s` (libreria `snow`), con prologo `"Nyctalus v0 2026-09-27 stretta di mano"`.
  ```text
    <- s              il mittente conosce la chiave pubblica statica del destinatario
    ...
    -> e, es          48 byte
    <- e, ee          48 byte
  ```
- **Perché NK e non IK:** con NK il mittente **non ha chiave statica**, quindi resta anonimo anche verso il destinatario. IK gli farebbe rivelare la sua identità, e la versione 0.1 di questa specifica lo indicava per errore.
- **Autenticazione:** solo chi possiede la chiave privata del destinatario riesce a leggere il 1° messaggio e a rispondere. La chiave pubblica funziona da **indirizzo** e si può condividere liberamente.
- **Forward secrecy:** i segreti dipendono da `ee`, cioè da due chiavi effimere buttate subito dopo l'uso.
- **Risultato:** lo "split" Noise produce due segreti da 32 byte, `verso_destinatario` e `verso_mittente`, che diventano i `segreto_condiviso` (§3.2) dei due flussi della conversazione, uno per direzione.
- **Identità del destinatario:** 64 byte (privata ‖ pubblica) salvati con permessi 600.

### 5.2 Tra client e nodi del percorso (L1) 📐
Le chiavi dei singoli salti saranno incorporate nell'intestazione Sphinx: una sola chiave effimera, nessun giro di andata e ritorno.

### 5.3 Varie 📐
- **0-RTT:** solo per ricollegarsi al proprio guard, mai per dati applicativi sensibili.
- **Post-quantistico:** in seguito, X25519 + ML-KEM in modalità ibrida.

---

## 6. Siti interni e punto d'incontro 📐

- ✅ **Indirizzo `.nyct`** (`motore/nyctalus-core/src/indirizzo.rs`): `base32(chiave_pubblica[32] ‖ controllo[2] ‖ versione[1]) + ".nyct"`, 56 caratteri più il suffisso, come gli `.onion` v3. Il controllo sono i primi 2 byte di `BLAKE3-derive_key("Nyctalus indirizzi v0 controllo", chiave ‖ versione)` e intercetta gli errori di battitura. La chiave è quella X25519 della stretta di mano NK (§5.1): l'indirizzo **è** la chiave con cui si autentica il servizio. Esempio: `azcmipyj6qnj3sxljokv4wqnljonwrwer5ntsj7tkprjkn4tbu3puwqa.nyct`.
- ✅ **Punto d'incontro** (`motore/nyctalus/src/incontro.rs`, comandi `site`/`browse` + integrazione nel proxy SOCKS): un servizio si **registra** presso un nodo punto d'incontro con la sua chiave pubblica (il `.nyct`); il client chiede di essere **collegato** a quel `.nyct` e il nodo fa da **ponte opaco** (non vede il contenuto). Client e servizio fanno la **stretta Noise NK end-to-end** (§5.1: l'indirizzo *è* la chiave con cui si autentica il servizio) e sopra la sessione viaggia HTTP; il servizio serve una cartella di file (HTML5/JS/immagini…), in keep-alive.
  - **Scelta del punto d'incontro:** deterministica dall'indirizzo `.nyct` e dall'elenco firmato (`rendezvous.rs`: ordine da `BLAKE3(chiave ‖ epoca ‖ indice)`), con rotazione per epoca (giorno). Servizio e client vi arrivano senza descrittore.
  - **Modo onion (predefinito):** client e servizio raggiungono il punto d'incontro con un **circuito a più salti**. L'uscita del circuito, tolto il proprio strato, riconosce il controllo (marca `NYCTRDV1`, che la distingue da un LINK conflux) e **cuce i due circuiti**: così il punto d'incontro vede solo l'ultimo relè, non gli IP di client e servizio.
  - **Modo diretto:** con `--rendezvous IP:PORT --fingerprint` si usa una connessione diretta a salto singolo (per prove con un solo nodo); quel nodo vede gli IP.
  - **Da fare:** descrittore firmato pubblicato (oggi la scelta è deterministica), richieste in pipeline/POST, lettura per Content-Length.
- 📐 Evoluzione prevista: per pubblicare i punti d'incontro servirà un **descrittore** firmato (chiave Ed25519 del sito legata a quella X25519) nella tabella distribuita dei nodi, e un **segnale** cifrato che il client lascia al punto d'incontro (la "notizia" della VISIONE §5.3), così entrambi i lati arrivano con percorsi anonimi.
- Un web server tradizionale (Nginx, Apache…) può stare dietro al demone come alternativa al server di file incorporato.

### 6.0 Casella cifrata (posta per chi non è in linea) ✅
- **Busta:** un solo messaggio **Noise_X_25519_ChaChaPoly_BLAKE2s** (`nyctalus-core/src/casella.rs`), prologo `Nyctalus v0 2026-10-31 casella`: `-> e, es, s, ss` + carico. Solo il destinatario (chiave del suo `.nyct`) la apre; la chiave statica del mittente viaggia cifrata e autenticata (ss). Carico: `<id casuale> <ts> <testo>`, testo ≤ 16 KB; l'`id` serve a scartare le ripetizioni (X non le impedisce). Niente forward secrecy verso la chiave del destinatario: per questo la casella tiene la posta al più 14 giorni.
- **Deposito anonimo:** `POST /deposita?a=<destinatario>` all'indirizzo `.nyct` **della casella** (servizio a sé, diverso dalla rubrica) attraverso i circuiti, senza presentarsi: la casella sa solo che c'è posta per quell'indirizzo, quando e quanto pesa. Limiti: 64 KB a busta, 200 buste per destinatario, 50 MB in tutto.
- **Ritiro autenticato:** dalla sessione della chat con la **rubrica** (che autentica il mittente): `CASELLA RITIRA` → `CASELLA POSTA <id> <busta hex>…` (anche spontaneo appena il destinatario risulta in linea) → `CASELLA RICEVUTI <id>…` e la casella cancella. Il motore intercetta questi messaggi: non vanno in cronologia né all'interfaccia; le buste aperte diventano messaggi del mittente vero (`MSG`, `NEW`).
- Il motore deposita quando il contatto non risponde (o la sonda di presenza lo sa già irraggiungibile) ed emette `OUT` + `CAS <addr> <ts>`.
- **Spunte:** nella sessione cifrata il destinatario risponde `K <ts>` a ogni testo o allegato ricevuto (anche dalla casella) e `R <ts>` quando l'utente ha letto fino a quell'ora (comando `LETTO <addr> <ts>` dell'interfaccia); le conferme senza sessione aspettano in coda e la sonda di presenza contatta anche chi ne attende. Stati in `stati/<addr>.log`, righe `ts stato quando` (`1` spedito, `c` casella, `2` consegnato, `3` letto; `quando` = ora Unix in cui lo si è saputo, assente nelle righe dei motori vecchi), eventi `SP <addr> <ts> <n> [quando]` / `CAS <addr> <ts> [quando]`, rinviati con `HIST`; servono all'«Info» del messaggio. Un testo senza `K` quando la sessione cade, o dopo 45 s se il contatto conferma di solito, va nella casella con la sua ora originale; il destinatario scarta il doppione (riga identica in cronologia). Le versioni senza spunte ignorano `K`/`R`.
- **Chiamate vocali (v1):** nella sessione cifrata `CI|CA|CN|CF <id>` (invito, accettata, rifiutata, fine) e voce `CX <seq 4 byte BE><Opus>` (20 ms, 24 kbit/s, ≤ 1500 byte); all'interfaccia `CALLIN/CALLOK/CALLNO/CALLEND addr id` e `AUD addr seq hex`, comandi `CALL`, `CALLOK|CALLNO|CALLEND`, `AUD`. Se la coda della sessione è piena il pacchetto audio si scarta; chi riceve scarta i pacchetti più vecchi dell'ultimo suonato. Il motore ricorda per 45 s gli inviti che squillano e li ripete a chi si iscrive dopo (`SUB`). Passano dalla rete Nyctalus come i messaggi (nessun collegamento diretto). Possibile v2: canale a datagrammi lungo i nodi.
- **Trasferimenti di file grandi (programma «File», v1):** controllo nella sessione cifrata della chat — `XO <id> <ts> <dim> <pezzo> <mime> <nome hex> <radice>` (proposta), `XA`/`XN` (accettata/rifiutata), `XP`/`XR` (pausa/riprendi, da entrambi i lati), `XC` (annullata), `XF <id> [ts]` (arrivato tutto; `ts` = ora d'arrivo, usata da entrambi per il file in chat). Pezzi da 256 KB con impronta BLAKE3; `radice` = BLAKE3 della fila di impronte. **Dati** su canali paralleli (ognuno un collegamento `.nyct`, quindi un circuito diverso) aperti da chi riceve con saluto `T<indirizzo>` e la stessa stretta Noise NK della chat; record sigillati ChaCha20-Poly1305: richieste `X<id>` (quale trasferimento, solo se `peer` autenticato = destinatario della proposta), `M` (distinta), `G<n>`; risposte `H…`/`h`, `B<n><off><dati>` (32 KB), `Z<n>`, `E<msg>`. Chi riceve verifica ogni pezzo (sbagliato → richiesto di nuovo, al 4° errore stop), tiene la mappa dei pezzi su disco (ripresa dopo cadute/spegnimenti), controlla tutto il file prima di rinominarlo. Solo dai contatti; accettazione automatica fino alla soglia (100 MB predefinita). Interfaccia: `XSEND <addr> <percorso hex> [mime]`, `XOK|XNO|XPAUSA|XRIPRENDI|XANNULLA <id>`, `XVIA <id>` (toglie dall'elenco un concluso), `XPULISCI`, `XLIST`, `XCONF [soglia N] [canali N] [inchat 0|1]`; eventi `XNEW <id> <in|out> <addr> <dim> <mime> <nome hex> <stato> <fatti> <percorso hex|->` (stato `eliminato` se il file arrivato non c'è più), `XVIA <id>`, `XSTATO <id> <stato> [dettaglio hex]`, `XPROG <id> <fatti> <totale> <byte/s>`, `XFINE <id> <percorso hex>`; a fine trasferimento la chat riceve anche `FILE` (il file compare nella conversazione). Stato su disco in `trasferimenti/`.
- **Servizi `.nyct` e client contemporanei:** il servizio tiene `--pool` (4) registrazioni in attesa presso i punti d'incontro; quando un client arriva viene servito in un compito a parte e la registrazione si rinnova SUBITO. Tetto: 256 client serviti insieme per servizio (`CLIENT_MAX`). Prima un posto restava occupato per tutta la durata del client (massimo 4 sessioni lunghe).
- **Diagnosi:** `NYCTALUS_DIAGNOSI=1` fa scrivere al motore su stderr ritiri della posta, comandi di controllo, messaggi di sessione, client dei servizi (spenta di serie).
- **nyctalus-cli e collegamento alla porta di controllo:** chi accende il motore (avvio del PC, orchestratore del telefono/Windows) scrive `~/.nyctalus-chat/controllo` = `<porta> <segreto>` (permessi 600) e lo cancella allo spegnimento. `nyctalus-cli` (Python, solo libreria standard) usa i comandi dell'interfaccia già descritti (`AUTH`→`AUTHOK`, `WHOAMI`, `LIST`, `SUB`, `ADD`, `SEND`, `HIST`, `LETTO`, `X…`) senza estensioni del motore. `avvia-chat.sh` con `NYCTALUS_INTERFACCIA=nessuna` accende il motore senza finestre. Il ⌨ Terminale dell'app usa il WebSocket `/ws/terminale` dell'orchestratore, con messaggi JSON: `{t:"cmd", riga, colonne}`, `{t:"in", d}`, `{t:"stop"}`; risposte `{t:"out", d}`, `{t:"err", d}`, `{t:"fine", codice}`. La riga diventa argomenti con `shlex` ed esegue solo `nyctalus-cli`, con le pipe (niente PTY) e `COLUMNS` = larghezza del pannello.
- **Codice di rete e protezione (Tappa 4):** il JSON del codice `NYCT1:` ha, oltre a `v, nome, directory_url, authority, rubrica, casella`, i campi facoltativi `camuffamento` (64 hex, chiave di `--obfuscation`) e `rumore` (Hz di `--cover`, 0-10). Il gestore li tiene in `protezione.json` (`attiva`, chiave conservata anche da spenta); reti nuove: accesa con rumore 0,2. I client li salvano nella loro configurazione e li passano al motore (`start`/`circuit` con `--obfuscation --cover`, `site` con `--obfuscation`); l'installatore dei nodi li legge con `--codice`.
- **Modalità riservata (PIANO-ANONIMATO Tappa 2-bis):** comando `RISERVATA [0|1]` → `RISERVATA <accesa> <casella+rubrica disponibili> <in coda> <finte spedite> <finte tornate>` (anche come evento); stato in `riservata`, coda in `coda-riservata` («addr ts hex»). Accesa: `SEND` mette in coda (✓ «1»); a ritmo di Poisson (media 30 s) si deposita UNA busta con carico fisso di 4096 byte: il primo della coda (`p-<id> <ts> <len> <testo><riempitivo casuale>`, poi «c») oppure una finta a se stessi (`g-<id> <ts> 0 <riempitivo>`), che al ritiro si conta e si scarta. Niente `K`/`R` in uscita, niente sonda di presenza; ritiro ogni 30 s con `CASELLA RITIRA RISERVATA`, per cui la rubrica non consegna più all'arrivo della busta né all'entrata in linea (solo su richiesta). Testo massimo 3800 byte. Regolazioni solo per le prove: `NYCTALUS_RITMO_RISERVATO`, `NYCTALUS_RITIRO_RISERVATO`.
- **Videochiamate (v1):** invito video `CI <id> v` (all'interfaccia `CALL addr v` → `CALLIN addr id v`); immagini `CV <seq 4 byte BE><key 1 byte><VP8>` (≤ 60 000 byte; 320×240 o 240×320, 15 fps, ~200 kbit/s, immagine completa ogni 2 s), richiesta di immagine completa `CK`, telecamera accesa/spenta `CW 0|1`. All'interfaccia `VID addr seq key hex`, `VIDKEY addr`, `VIDSTATO addr 0|1` (comandi omonimi). Chi riceve, dopo un buco nella numerazione, scarta le immagini parziali fino alla prossima completa e la chiede con `VIDKEY` (al più una volta al secondo). Le righe `VID` superano i 64 KB: chi legge la porta di controllo deve accettare righe fino a 1 MB.
- **Battito:** ogni 25 s `PING` nella sessione, risposta `PONG`; se un contatto che ha già risposto almeno una volta tace per 75 s, la sessione è chiusa (contatto non più «in linea», testi in volo nella casella). Le versioni senza battito non rispondono e non vengono chiuse.

### 6.0-bis Nodi annunciati con un nome (IP dinamico) ✅
Una voce dell'elenco può essere `nome:porta` (es. `miacasa.ddns.net:4601`). Il client risolve il nome
(primo IPv4) per collegarsi; nel pacchetto Sphinx il «prossimo salto» verso un nodo con nome si
codifica **per chiave** (`[6][primi 31 byte della X25519]`): il nodo precedente lo cerca nell'elenco
firmato e risolve il nome in quel momento. Un cambio di IP non richiede di rifirmare l'elenco; la
sicurezza resta quella del pinning (chiave + impronta TLS dall'elenco): un DNS falso rende il nodo
irraggiungibile, non lo sostituisce. Le voci con IP restano codificate come prima (`[4][IPv4][porta]`).

### 6.1 Ponte verso i siti `.onion` di Tor ✅ (via Tor di sistema)
✅ **Implementato** (`motore/nyctalus/src/ponte.rs`): l'uscita riconosce gli indirizzi `.onion` e li raggiunge tramite **un Tor già presente sul sistema**, aprendo un tunnel al suo proxy SOCKS5 (default `127.0.0.1:9050`, opzione `--tor-socks`). Scelta volutamente **leggera**: non si incorpora il client Tor Arti (enorme e da bootstrappare), si riusa il Tor del sistema (es. quello di NexusSec). Dal nodo ponte in poi valgono le regole e la lentezza di Tor. Incorporare Arti come alternativa resta un possibile sviluppo futuro.

---

## 7. Rumore di fondo 📐
- ✅ **Celle a lunghezza fissa** (`motore/nyctalus/src/cella.rs`): ogni cella del circuito ha la **stessa dimensione** sul filo (`LUNGHEZZA` = intestazione + `CAPACITA`), con la lunghezza reale in un campo interno e il resto a zero. Così un osservatore non deduce la lunghezza dei contenuti e — condizione necessaria per il rumore di fondo — una cella fittizia è **indistinguibile** da una vera.
- ✅ **Celle fittizie (cover traffic)** a ritmo di **Poisson** (`cmd::RIEMPIMENTO`), in **entrambe le direzioni**: in salita dal client (demone `--cover HZ`) e in discesa dal nodo d'uscita (`node --cover HZ`); l'altro estremo le scarta. Mascherano *quando* si invia davvero. Il traffico inoltrato per conto di altri utenti funziona già da copertura naturale.
- ✅ **Riempimento del LINK**: il primo quadro conflux (set_id) è riempito alla dimensione di un'unità dati, così non si distingue per lunghezza.
- 📐 Da fare: riempimento del pacchetto di **apertura Sphinx** e uniformità dei quadri del tunnel rendezvous (sottosistema separato); **taratura** della frequenza con misure reali (costo in banda contro beneficio).

---

## 8. Camuffamento (L0) 📐

### 8.1 Porta e mimetizzazione
Stato verificato sul traffico vero il 07/11/2026 (`simulazione/verifica_paradigmi.py`, pacchetti
catturati in Shadow su tutte le macchine):
- ✅ **Trasporto QUIC su UDP**: fra utente e rete solo UDP, nessuna connessione TCP.
- **Porta:** di serie **UDP 4601** (installatore dei server) o 4600; si può scegliere **UDP 443**
  (`installa.sh --porta 443`), la porta di HTTP/3, che dà meno nell'occhio. La porta 80 non si usa.
- ✅ **Offuscamento** (`motore/nyctalus-core/src/offuscamento.rs`, `motore/nyctalus/src/trasporto.rs`):
  con la chiave della rete ogni datagramma è avvolto con ChaCha20 (nonce casuale): sul filo nessuna
  intestazione QUIC, byte distribuiti come rumore. Di serie nelle reti nuove (dalla 0.3.4).
- ✅ **Resistenza alle sonde** (dalla 0.3.8): l'offuscamento non autentica, e QUIC rispondeva a
  certi byte a caso (negoziazione di versione): misurato **63 risposte su 20.000 sonde**, abbastanza
  per scoprire un nodo. Ora da un indirizzo sconosciuto passa solo un vero `Initial` QUIC v1:
  **0 risposte su 20.000**. Un nodo sondato senza chiave sembra un indirizzo spento.
- ✅ **Ponti nascosti** (dalla 0.3.8): entrate fuori dall'elenco pubblico con una chiave propria
  (`node --hidden-bridge`, client `--bridge`/`NYCTALUS_PONTI`).
- ✅ **Nessun nome nel primo pacchetto** (dalla 0.3.8): prima il client mandava il nome
  `nyctalus.invalid`, leggibile da chiunque nel primo pacchetto QUIC (le cui chiavi sono pubbliche)
  quando il camuffamento era spento; ora ci si collega per indirizzo IP e il nome non c'è.
- ✅ **Forma HTTP/3** (dalla 0.3.9, scelta della rete): QUIC in chiaro con ALPN `h3`, nome del
  server scelto dal gestore, porta 443; la chiave della rete diventa un contrassegno di 16 byte
  scritto all'inizio di ogni flusso **dentro** TLS; chi non lo ha riceve una risposta HTTP/3 vera
  (SETTINGS sul flusso di controllo, HEADERS `:status 404` in QPACK statico, DATA con una pagina
  HTML). Certificati nuovi con nome e validità normali (il predefinito di rcgen era riconoscibile).
  Non mescolabile con i nodi in forma «rumore»: è una scelta di tutta la rete (rustls rifiuta
  ALPN diversi, `server/hs.rs`).
- ✅ **Ripiego su TCP** (0.3.13, `tcp.rs`): i nodi ascoltano anche in TCP sulla stessa porta (un
  socket ibrido per quinn); il client prova UDP e dopo 6 s passa a TCP (`NYCTALUS_TCP`). Quadri
  «lunghezza (2 byte, cifrata con flusso di chiave BLAKE3 per direzione) + datagramma»; davanti a
  dati senza senso il nodo legge e butta in silenzio (max 60 s) invece di chiudere; max 1024
  connessioni TCP per nodo.
- ✅ **TCP come HTTPS nella forma HTTP/3** (0.3.14): il ripiego è un vero TLS (rustls guidato a
  mano in `tcp::tubo_tls`, stesso certificato del nodo, ALPN http/1.1); dentro TLS il contrassegno;
  chi non lo ha riceve `HTTP/1.1 404 Not Found` e la chiusura, come da un sito.
- ✅ **Impronta TLS comune** (0.3.15): TLS 1.2 attivo (rustls `tls12`) e ALPN `h2, http/1.1` sul
  TCP: JA4 `t13d1011h2`, quella dei comuni programmi rustls, non più un'impronta propria
  (`t13d0310h1`). Misura: `simulazione/impronta_tls.py`.
- 📐 **Da fare:** impronta identica a Chrome (richiede BoringSSL); forma «videochiamata» (WebRTC).
- Dimensioni sul filo: le celle sono fisse **dentro** il circuito; i datagrammi QUIC invece hanno
  lunghezze diverse (pieni ~1200-1420 byte, più conferme e code più corte).

### 8.2 HTTP e HTTPS: due livelli distinti
- **Dentro la rete:** Nyctalus trasporta qualunque protocollo (come Tor con qualsiasi TCP). Il nodo d'uscita parla HTTP/HTTPS con il sito di destinazione. HTTP/HTTPS stanno quindi *sopra* la rete, verso il sito finale, non sono il protocollo di trasporto.
- **Fuori (camuffamento):** ci si traveste da HTTPS/HTTP-3 per non essere distinguibili né bloccati.

### 8.3 VPN: posizione del progetto ❓→decisa
Una VPN **non** aggiunge anonimato al protocollo: sposta soltanto la fiducia su un'azienda che vede l'IP reale e il fatto che usi Nyctalus. È l'opposto del principio "nessuno conosce insieme origine e destinazione".

- ❌ **Nessuna VPN come componente interno obbligatorio:** creerebbe un punto centrale di fiducia e di log, ridurrebbe l'anonymity set e contraddirebbe la decentralizzazione.
- ✅ **Opzionale, lato utente, VPN → Nyctalus** (la VPN *prima* del guard): scelta legittima per nascondere al proprio provider persino la connessione al guard, o dove Nyctalus è bloccato. Prezzo: ci si fida della VPN. Va solo documentato, non integrato.
- ❌ **Nyctalus → VPN** (VPN dopo l'uscita): inutile o dannoso (collo di bottiglia visibile).
- ✅ **Alternativa nativa migliore** per "nascondere che uso Nyctalus": i **bridge/ponti** (nodi d'ingresso non pubblici) uniti al camuffamento (§8.1), che non concentrano fiducia. Più salti e più nodi indipendenti aumentano l'anonimato più di una VPN.

---

## 9. Elenco dei nodi ✅ (prototipo) / ❓ (governance)

✅ **Implementato** (`motore/nyctalus-core/src/elenco.rs`): elenco testuale firmato da un'**autorità Ed25519**; ogni voce = `indirizzo, chiave X25519, impronta TLS [famiglia=NOME]` (la famiglia è facoltativa e firmata; senza, la riga è identica a prima).

✅ **Scelta dei percorsi** (`motore/nyctalus/src/percorsi.rs`, PIANO-ANONIMATO Tappa 2): **guardia** d'entrata scelta a caso una volta per rete (chiave = BLAKE3 dell'autorità) e tenuta 30-60 giorni, su file privato; **uscita** (circuiti verso Internet) e **intermedi** a caso; verso un punto d'incontro `.nyct` si entra dalla guardia, salvo che sia parente del punto d'incontro (allora un'altra entrata solo per quel percorso). **Parentela:** stesso nodo, stessa famiglia dichiarata, o indirizzi pubblici nella stessa /16 (IPv4) o /32 (IPv6); indirizzi privati/locali senza sottorete. Regola impossibile → percorso con avviso su stderr, oppure errore con `NYCTALUS_PARENTELA=rigida`. Prima: entrata e uscita erano il 1° e il 2° nodo dell'elenco per tutti, e verso i `.nyct` l'entrata cambiava a ogni collegamento. Client e nodi verificano la firma e ne ricavano le identità. È il meccanismo che rende possibile l'**autenticazione dei collegamenti** (§3-bis): il client pinna il guard, ogni relè pinna il nodo a valle cercandone l'impronta nell'elenco. Comandi: `nyctalus authority`, `nyctalus directory`, e `--directory/--authority` su nodi e client.

❓ **Da decidere (governance):** chi gestisce l'autorità e come si aggiorna l'elenco. Opzioni:
Chi pubblica l'elenco firmato dei nodi e delle loro chiavi. Opzioni:
- (a) autorità fidate in votazione, come Tor;
- (b) tabella distribuita con verifica incrociata;
- (c) un ibrido delle due.

Da decidere in fase 2 insieme alla difesa dai nodi falsi in massa (attacco Sybil).

---

## 10. Primitive crittografiche

| Uso | Primitiva | Stato |
|---|---|---|
| Derivazione chiavi, etichette | BLAKE3 (derive_key, keyed_hash) | ✅ |
| Cifratura end-to-end | ChaCha20-Poly1305 (RFC 8439) | ✅ |
| Scambio di chiavi | X25519 via Noise NK (+ BLAKE2s) | ✅ |
| Buste della casella cifrata | Noise X (X25519, ChaCha20-Poly1305, BLAKE2s) | ✅ |
| Identità di nodi e siti | Ed25519 | 📐 |
| Pacchetti a cipolla | Sphinx | 📐 |
| Trasporto | QUIC (RFC 9000) + TLS 1.3 | 📐 |
| Post-quantistico | ML-KEM (FIPS 203), ibrido | 📐 più avanti |

Regola: **nessuna primitiva inventata**. Si usano solo costruzioni pubbliche e analizzate.

---

## 11. Programma di prova ✅

`motore/nyctalus` (eseguibile `nyctalus`) trasferisce un file tra due estremi:
- **L0:** QUIC (quinn + rustls, backend ring, solo TLS 1.3). Il certificato è autofirmato e il mittente lo accetta solo se la sua impronta BLAKE3 coincide con quella attesa (pinning).
- **L2:** frammenti cifrati di `nyctalus-core`, distribuiti a turno su k corsie, cioè k stream QUIC unidirezionali in parallelo, con il controllo del flusso di §4.4.
- **Stretta di mano:** Noise NK su un primo stream bidirezionale QUIC (§5.1). Il ricevitore stampa la sua chiave pubblica (`--destinatario`); con `--identita FILE` la chiave resta la stessa tra un avvio e l'altro.
- **Limite:** non ci sono ancora nodi intermedi, e le conferme di avanzamento sono protette solo da L0.

Misura su un solo PC (loopback, compilazione release, file casuale da 200 MB): circa **110 MB/s con 1 corsia e 122 MB/s con 4**. File identici, 0 pacchetti scartati; con 4 corsie circa il 70% dei frammenti arriva in anticipo e viene ricomposto. Su loopback le corsie non hanno percorsi fisici diversi: il guadagno reale del multipercorso andrà misurato con Shadow e su reti vere.

---

## 12. Registro delle modifiche
- **0.1 (27/09/2026):** prima bozza; il livello L2 è implementato in `nyctalus-core` con 18 test; aggiunti il programma di prova QUIC e il controllo del flusso.
- **0.2 (27/09/2026):** stretta di mano Noise NK implementata (corretto IK → NK per l'anonimato del mittente); il segreto non si passa più a mano. Indirizzi `.nyct` implementati; ponte verso `.onion` progettato. 29 test.
- **0.4 (31/10/2026):** casella cifrata (§6.0): buste Noise X, deposito anonimo, ritiro autenticato tramite la rubrica.
