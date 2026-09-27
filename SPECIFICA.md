# Nyctalus Protocol — Specifica tecnica v0

> Versione 0.1 · 27 settembre 2026 · Stato: bozza di lavoro
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

Codice: `crates/nyctalus-core/src/` — moduli `chiavi`, `etichette`, `cifratura`, `ricomposizione`, `flusso`.

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

📐 Il `segreto_condiviso` arriverà dalla stretta di mano **Noise** (§5). Per ora i test usano un segreto fisso.

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
- **Nessuna forward secrecy di per sé:** la eredita dalla stretta di mano che produce il segreto (§5).
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

## 5. Stretta di mano e chiavi 📐

- **Framework:** Noise Protocol Framework (libreria `snow`) su X25519, ChaCha20-Poly1305, BLAKE2s/BLAKE3 ❓.
- **Client ↔ nodi del percorso:** le chiavi dei salti sono incorporate nell'intestazione Sphinx: una sola chiave effimera, nessun giro di andata e ritorno.
- **Client ↔ uscita o sito interno (L2):** Noise **IK** (il client conosce la chiave pubblica statica della destinazione). Il risultato è il `segreto_condiviso` di §3.2, con forward secrecy grazie alle chiavi effimere.
- **0-RTT:** solo per ricollegarsi al proprio guard, mai per dati applicativi sensibili.
- **Post-quantistico:** in seguito, X25519 + ML-KEM in modalità ibrida.

---

## 6. Siti interni e punto d'incontro 📐

- **Indirizzo del sito:** codifica base32 della sua chiave pubblica Ed25519, più un suffisso ❓.
- Il sito pubblica, firmato, un **descrittore** con i suoi punti d'incontro nella tabella distribuita dei nodi.
- Il client lascia al punto d'incontro un **segnale** di pochi byte, cifrato per il sito (la "notizia" della VISIONE §5.3). Il sito lo ritira e i due costruiscono i percorsi fino al punto d'incontro.
- Il web server resta quello di sempre (Nginx, Apache…): il demone Nyctalus gli sta davanti come proxy.

---

## 7. Rumore di fondo 📐
- Ogni nodo invia pacchetti fittizi a ritmo di Poisson verso i nodi vicini.
- Un pacchetto fittizio ha la stessa lunghezza di uno vero e un'etichetta casuale: per chi lo osserva è identico, e per il destinatario è solo un'etichetta sconosciuta, da scartare.
- Il traffico inoltrato per conto di altri utenti funziona già da copertura naturale.
- ❓ La frequenza va tarata con misure reali (costo in banda contro beneficio).

---

## 8. Camuffamento (L0) 📐
- Il collegamento tra nodi usa QUIC con una stretta di mano TLS 1.3 **indistinguibile da quella dei browser comuni**, e sulla porta 443.
- Un nodo sondato da un censore risponde come un normale sito web; solo chi presenta un segreto valido accede al protocollo, come fanno WebTunnel e obfs4.
- In alternativa si può passare su TCP/TLS dove UDP è bloccato.

---

## 9. Elenco dei nodi ❓
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
| Scambio di chiavi | X25519 via Noise | 📐 |
| Identità di nodi e siti | Ed25519 | 📐 |
| Pacchetti a cipolla | Sphinx | 📐 |
| Trasporto | QUIC (RFC 9000) + TLS 1.3 | 📐 |
| Post-quantistico | ML-KEM (FIPS 203), ibrido | 📐 più avanti |

Regola: **nessuna primitiva inventata**. Si usano solo costruzioni pubbliche e analizzate.

---

## 11. Programma di prova ✅

`crates/nyctalus` (eseguibile `nyctalus`) trasferisce un file tra due estremi:
- **L0:** QUIC (quinn + rustls, backend ring, solo TLS 1.3). Il certificato è autofirmato e il mittente lo accetta solo se la sua impronta BLAKE3 coincide con quella attesa (pinning).
- **L2:** frammenti cifrati di `nyctalus-core`, distribuiti a turno su k corsie, cioè k stream QUIC unidirezionali in parallelo, con il controllo del flusso di §4.4.
- **Limite:** il segreto del flusso è generato dal ricevitore e passato a mano (§5 lo sostituirà con Noise). Non ci sono ancora nodi intermedi.

Misura su un solo PC (loopback, compilazione release, file casuale da 200 MB): circa **110 MB/s con 1 corsia e 122 MB/s con 4**. File identici, 0 pacchetti scartati; con 4 corsie circa il 70% dei frammenti arriva in anticipo e viene ricomposto. Su loopback le corsie non hanno percorsi fisici diversi: il guadagno reale del multipercorso andrà misurato con Shadow e su reti vere.

---

## 12. Registro delle modifiche
- **0.1 (27/09/2026):** prima bozza; il livello L2 è implementato in `nyctalus-core` con 18 test; aggiunti il programma di prova QUIC e il controllo del flusso.
