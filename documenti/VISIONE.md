# Nyctalus Protocol — Documento di visione

> Versione 0.4 · 27 settembre 2026 · Autore: Daniele Deplano
> Stato: idea consolidata, prima della specifica tecnica.
> Nome: **Nyctalus Protocol** (in breve *Nyctalus*), dal genere scientifico delle nottole: pipistrelli che volano veloci di notte e si orientano al buio. Dal greco *nyx* (notte). Scelto il 27/09/2026 al posto del nome di lavoro "NyxShift", troppo vicino a "nyx", il monitor dei relay del Tor Project. Prima della pubblicazione va verificato come marchio su EUIPO/WIPO.

---

## 1. In una frase

**Nyctalus Protocol è una rete anonima in cui ogni utente è anche un nodo, il traffico è camuffato da normale navigazione, e i dati viaggiano spezzati in tanti frammenti su molte strade in parallelo ("a ragnatela"), per ricomporsi solo a destinazione.**

L'obiettivo è offrire la protezione di Tor con una velocità adatta all'uso quotidiano: navigazione, streaming, chiamate.

---

## 2. Perché serve

Tor funziona, ma è lento per ragioni strutturali:

| Limite di Tor | Conseguenza |
|---|---|
| Circa 7.000 nodi volontari per milioni di utenti | La banda disponibile è poca e la rete è congestionata |
| Un solo percorso di 3 nodi per connessione | Se un nodo rallenta, rallenta tutto |
| Trasporto su TCP | Un pacchetto perso blocca tutti quelli dopo |
| Nodi scelti senza guardare le distanze | I dati possono fare Italia → Australia → Islanda per aprire un sito francese |
| Traffico riconoscibile come "Tor" | I provider e i governi lo individuano e lo bloccano |

---

## 3. Principi fondamentali

1. **Crittografia standard, mai inventata.** La sicurezza sta nelle chiavi, non nel segreto dell'algoritmo. Si usano solo algoritmi pubblici e collaudati da anni di attacchi (vedi §9).
2. **Camuffamento sempre attivo.** Visto da fuori, il traffico Nyctalus sembra una normale navigazione HTTPS o una videochiamata.
3. **Più utenti = più velocità.** Ogni installazione su PC aggiunge capacità alla rete, invece di consumarla soltanto.
4. **Nessuno sa tutto.** Ogni nodo conosce solo il nodo precedente e quello successivo. Nessun pacchetto contiene l'IP di chi lo ha mandato o di chi lo riceverà.
5. **Niente log, niente dati su disco.** I nodi tengono tutto in RAM e cancellano le chiavi appena una sessione finisce.
6. **Codice sicuro per costruzione.** Si scrive in Rust, che impedisce all'origine i bug di memoria.

---

## 4. Come viaggia un dato

```
                       ┌──► nodo ──► nodo ──┐
 Tu ──► [INGRESSO] ────┼──► nodo ──► nodo ──┼──► RICOMPOSIZIONE ──► destinazione
       (fisso per      ├──► nodo ──► nodo ──┤    (uscita verso Internet
        settimane)     └──► nodo ──► nodo ──┘     oppure sito interno)
```

1. **Camuffamento.** Il tuo computer si collega al nodo d'ingresso con traffico che sembra una normale navigazione HTTPS.
2. **Ingresso fisso.** Il primo nodo resta lo stesso per settimane. Se cambiasse spesso, prima o poi capiteresti su un nodo controllato da un attaccante, e quel nodo vedrebbe il tuo IP. Con un ingresso fisso quel rischio lo corri una volta sola.
3. **Ragnatela.** Dopo l'ingresso i dati vengono spezzati in molti frammenti, che viaggiano in parallelo su strade diverse. Se una strada rallenta, i frammenti successivi vengono spostati sulle altre.
4. **Cipolla.** Ogni frammento è chiuso in più strati di cifratura. Ogni nodo toglie il proprio strato e scopre solo a chi passare il frammento dopo.
5. **Ricomposizione.** A destinazione i frammenti, arrivati in ordine sparso, vengono rimessi in ordine e ricomposti.

---

## 5. I sei meccanismi chiave

### 5.1 Camuffamento del traffico
Due strati:
- **Dentro:** cifratura standard, che protegge il contenuto.
- **Fuori:** un "vestito" che rende il traffico indistinguibile da un normale sito HTTPS o da una chiamata.

Il provider vede solo qualcuno che naviga: non può capire che usi Nyctalus, quindi non può bloccarlo. In Tor è un'aggiunta opzionale (i "bridge"), in Nyctalus è attivo **di serie**.

### 5.2 Ingresso fisso + ragnatela
Descritti al §4. La velocità viene dalla ragnatela; l'ingresso fisso impedisce che la ragnatela esponga il tuo IP.

### 5.3 Punto d'incontro ("cassetta postale")
Per contattare qualcuno dentro la rete (una persona o un sito interno):
1. Il mittente lascia un **piccolo segnale cifrato** in un nodo neutro, il punto d'incontro.
2. Il destinatario lo ritira.
3. Da lì si costruiscono le strade per lo scambio vero.

Nessuno dei due scopre mai dove si trova fisicamente l'altro.

### 5.4 Etichette segrete e sempre diverse
Per ricomporre i frammenti, ogni pacchetto porta pochi byte di etichetta. Queste etichette:
- **non** contengono indirizzi IP;
- per i nodi che le vedono sono **rumore casuale**;
- **cambiano a ogni pacchetto**, come il codice di un telecomando d'auto.

Solo il destinatario, con una chiave concordata con il mittente, riconosce "questo è mio, pezzo 347 del flusso X". Così nessuno può seguire i pacchetti lungo la rete.

```
 Pacchetto 1:  [ a7 3f 9c 12 ]   ← per i nodi: rumore
 Pacchetto 2:  [ 04 e8 b1 5d ]   ← per i nodi: rumore
 Pacchetto 3:  [ 9b 22 f0 c7 ]   ← per i nodi: rumore
                     │
                     ▼
 Destinatario (con la chiave):  "flusso X, pezzi 1-2-3 → ricompongo"
```

### 5.5 Rumore di fondo
Anche il **ritmo** del traffico può tradire. Per esempio, "segnale, pausa, poi raffica di 10.000 pacchetti" è una sequenza riconoscibile. Per questo i nodi si scambiano di continuo anche **pacchetti finti**, indistinguibili da quelli veri. Dato che ogni utente è un nodo, gran parte di questo rumore c'è già senza costi: il traffico che inoltri per gli altri copre il tuo.

### 5.6 Siti interni
Come i siti `.onion` di Tor, ma più veloci:
- L'indirizzo è ricavato dalla chiave crittografica del sito: nessun DNS e nessun registro centrale.
- Il server resta invisibile e non serve nessun nodo d'uscita.
- La ragnatela funziona al massimo, perché i frammenti si ricompongono direttamente nel sito.
- Chi ha già un sito lo pubblica mettendo davanti il demone Nyctalus; il web server (Nginx, Apache…) resta quello di sempre.

---

## 6. Chi fa cosa: i ruoli dei dispositivi

Scelte fatte per ottenere la **massima velocità** della rete.

| Dispositivo | Ruolo | Note |
|---|---|---|
| **PC fisso / portatile** | Client + **nodo interno** attivo di default | L'utente sceglie quanta banda cedere |
| **Server dedicati** (del progetto e di volontari fidati) | **Nodo d'uscita** ad alta banda | Il nucleo che garantisce la velocità verso Internet |
| **Utenti volontari** | Nodo d'uscita **solo se lo scelgono** | Ammessi solo se la rete misura che hanno banda sufficiente |
| **Telefoni** | **Solo client** | Fanno da nodo solo in Wi-Fi e sotto carica |

**Perché le uscite non sono di tutti:** chi fa da uscita "mette la faccia" verso Internet. Se qualcuno commette un reato, risulta l'IP del nodo d'uscita. Nessuno deve diventarlo senza saperlo.

**Perché i telefoni sono solo client:** un telefono in 4G che appare e scompare rende la rete instabile invece di aiutarla.

---

## 7. Cosa offre: due usi

1. **Navigare Internet in modo anonimo:** browser → rete → nodo d'uscita → sito normale.
2. **Siti e servizi interni:** browser → rete → sito interno, senza nessuna uscita. È **l'uso più veloce e più sicuro**.

---

## 8. Il browser

Serve un browser dedicato, come per Tor. La rete protegge il **percorso**, ma il browser può comunque tradirti con cookie, "impronta digitale" del dispositivo (fingerprinting), WebRTC e così via.

**Regola d'oro dell'anonimato nel browser:** tutti gli utenti devono sembrare **identici**. Un browser raro o personalizzato rende unici, anche se naviga su una rete anonima.

### Strategia
- **Browser pubblico (tutti i sistemi operativi):** basato su Firefox ESR, riusando le protezioni già sviluppate e verificate da Tor Browser e Mullvad Browser (codice open source). Non si scrive un browser da zero.
- **Browser di NexusSec (nxs_browser, Python + GTK3 + WebKit2GTK):** vedi §8.1.

### 8.1 Il browser di NexusSec si può usare?

**Sì, ma come banco di prova e come browser per NexusSec, non come browser anonimo pubblico.**

| Aspetto | Valutazione |
|---|---|
| Interfaccia, schede, preferiti, anti-tracciamento, permessi, traduzione | ✅ Ottima base, già pronta |
| Collegamento alla rete Nyctalus | ✅ Facile: WebKit2GTK supporta un proxy (`WebKit2.NetworkProxySettings`), basta puntarlo al demone locale |
| Uso nella **fase 1** (prototipo, test, demo) | ✅ Ideale: si prova subito la rete in un ambiente che conosci già |
| Anonimato davanti ai siti | ⚠️ Debole. Pochissime persone al mondo usano WebKitGTK su Linux, quindi chi lo usa è **riconoscibile** proprio perché raro. Inoltre WebKit non ha le protezioni anti-fingerprinting di Firefox/Tor Browser |
| Tutti i sistemi operativi | ❌ WebKitGTK di fatto esiste solo su Linux |

In pratica: si usa `nxs_browser` per sviluppare e provare Nyctalus dentro NexusSec. Per il pubblico serve un browser basato su Firefox. L'integrazione in NexusSec resta un ottimo canale di lancio.

### 8.2 Uso da terminale (demone e strumenti CLI)

Il browser è **solo uno** dei client. Il cuore di Nyctalus è un **demone** che gira in background e apre un proxy SOCKS5 locale, come fa `tor`. Il terminale è quindi l'interfaccia principale, e la GUI ci si appoggia sopra. Il parallelo con l'ecosistema di Tor:

| Tor | Nyctalus | A cosa serve |
|---|---|---|
| `tor` (demone) | `nyctalus avvia` | Fa girare nodo+client, apre il proxy SOCKS5 locale |
| file `torrc` | `~/.config/nyctalus/config` | Configurazione (banda ceduta, se fare da uscita, ecc.) |
| control port | `nyctalus stato`, `nyctalus circuiti` | Interrogare e gestire il demone in esecuzione |
| `nyx` (monitor) | `nyctalus monitor` | Cruscotto testuale: banda, circuiti, nodi (curses) |
| `torsocks <prog>` | `nyctalus esec <prog>` | Lanciare un programma qualsiasi dentro la rete |
| servizi `.onion` | `nyctalus sito ./cartella` | Pubblicare un sito `.nyct` da riga di comando |

Poiché il demone espone un **SOCKS5 standard**, i programmi che già sanno usare un proxy (curl, git, i browser via impostazioni) funzionano subito. `nyctalus esec` serve per i programmi che un proxy non lo prevedono, come fa `torsocks`.

Ordine di lavoro: **prima il SOCKS5** (sblocca sia il terminale sia il browser), poi i sottocomandi `avvia`/`stato`/`sito`, infine `monitor` ed `esec`. I comandi attuali `ricevi`/`invia` restano come strumenti di test.

---

## 9. Tecnologie

### Linguaggio: Rust
Scelto per due motivi:
- previene all'origine i bug di memoria: in una rete anonima un solo bug può esporre tutti gli utenti;
- un solo codice si compila per Linux, Windows, macOS, Android e iOS.

Tor stesso sta riscrivendo il suo client in Rust (progetto Arti).

### Valutazione dei riferimenti citati negli appunti

Gli appunti iniziali (`appunti-nuova-rete-tor-progetto-nyctalus-protocol.md`) nominano molti programmi. Ecco quali servono davvero:

**✅ Da usare**

| Riferimento | Uso in Nyctalus |
|---|---|
| **QUIC** (librerie `quinn` o `s2n-quic`) | Trasporto su UDP: nessun blocco per i pacchetti persi, connessione che sopravvive al passaggio da Wi-Fi a 5G |
| **Tokio** | Il "motore" che gestisce migliaia di connessioni contemporanee in Rust |
| **X25519 (Curve25519)** | Scambio di chiavi tra i nodi |
| **ChaCha20-Poly1305 / AES-GCM** | Cifratura dei dati (standard, veloce, accelerata dall'hardware) |
| **rustls / ring** | Librerie crittografiche Rust affidabili |
| **Nginx / Apache** | Per i siti interni: il web server resta quello di sempre, il demone Nyctalus sta davanti |
| **Tor, I2P, Nym** | Da **studiare**, non da copiare: Tor per ingresso fisso e browser, I2P per "ogni utente è un nodo", Nym per il rumore di fondo |

**➕ Da aggiungere (non c'erano negli appunti, ma sono i più importanti)**

| Riferimento | Perché |
|---|---|
| **Noise Protocol Framework** (libreria `snow`) | Il modo standard e verificato per le "strette di mano" crittografiche. Lo usano WireGuard, WhatsApp e Lightning |
| **Sphinx** (formato dei pacchetti a cipolla) | Formato di pacchetto anonimo studiato matematicamente. Lo usano Nym e Lightning. Sostituisce gli header inventati nella conversazione |
| **Simulatore Shadow** | Il simulatore di reti usato dal Tor Project: permette di misurare la velocità contro Tor in modo onesto, prima di avere migliaia di utenti |
| **Pluggable transports** (obfs4, WebTunnel, Snowflake) | Esempi reali di camuffamento del traffico da cui partire |

**⏳ Utili più avanti (solo per i nodi più grossi)**

| Riferimento | Nota |
|---|---|
| **eBPF** (libreria `aya`) / **DPDK** | Accelerano i nodi ad altissimo traffico. Non servono per partire: il collo di bottiglia vero è la banda, non la CPU |
| **Vivaldi** (coordinate di rete) | Utile per scegliere strade brevi, ma con cautela: le latenze possono rivelare la posizione, e le coordinate possono essere falsificate |
| **Crittografia post-quantistica** (es. ML-KEM) | Da affiancare a X25519 in modalità "ibrida" quando il protocollo sarà stabile |

**❌ Inutili o da scartare**

| Riferimento | Motivo |
|---|---|
| **C++, Asio, Seastar, Valgrind** | Si è scelto Rust |
| **redbpf** | Progetto abbandonato; al suo posto `aya` |
| **libsodium** | Ottima, ma è in C: in Rust ci sono equivalenti nativi |
| **Orchid, token e criptovalute** | Esclusi per scelta: niente economia, solo tecnica |
| **IPFS** | Tutt'altro scopo (archiviazione distribuita); al massimo un'ispirazione per i siti interni statici |
| **WebAssembly (client nel browser)** | Rischioso per l'anonimato: il client deve girare fuori dal browser |
| **Brave, LibreWolf** | La base giusta è Firefox ESR con le protezioni di Tor/Mullvad Browser |
| **WireGuard** (come programma) | Se ne riusa l'idea (il protocollo Noise), non il programma |
| **Pulizia degli header / falsificazione dello User-Agent via eBPF** | Impossibile: con HTTPS gli header sono cifrati. Il problema si risolve nel browser |
| **Codice C++ della conversazione** | Solo illustrativo: crittografia finta, header inventati, buffer attaccabili con un DoS. Le idee (riassemblaggio, controllo del flusso, tabella volatile) restano valide e vanno riscritte in Rust |

---

## 10. Cosa promettiamo e cosa no

**Nyctalus protegge da:**
- provider Internet, reti pubbliche Wi-Fi, datori di lavoro;
- tracciamento pubblicitario e profilazione;
- censura e blocchi (grazie al camuffamento);
- siti che vogliono conoscere il tuo IP;
- un singolo nodo malevolo, o un piccolo gruppo di nodi malevoli.

**Nyctalus NON promette:**
- protezione totale contro un'agenzia che sorveglia **contemporaneamente** gran parte di Internet. Contro un avversario del genere nessuna rete veloce può farlo: servirebbero ritardi volutamente lunghi. Sotto questo aspetto Nyctalus sarà protetto quanto Tor, non di più;
- anonimato se l'utente si identifica da solo (login, dati personali, file scaricati e aperti fuori dal browser).

**Il compromesso onesto:** molto più veloce di Tor, invisibile al provider, stessa robustezza di Tor contro gli avversari più potenti.

---

## 11. Problemi aperti (da risolvere nella specifica)

1. **Nodi falsi in massa (attacco "Sybil"):** un attaccante può accendere migliaia di nodi finti. Servono reputazione, anzianità dei nodi e limiti per rete di provenienza.
2. **Router domestici (NAT):** molti PC non accettano connessioni in entrata. Servono tecniche di "attraversamento" o nodi ponte.
3. **Elenco dei nodi:** chi pubblica la lista dei nodi affidabili? Tor usa 9 "autorità" fidate. Va scelto un modello.
4. **Responsabilità legale delle uscite:** servono una guida per i gestori e una politica sugli abusi.
5. **Nome e suffisso:** il nome Nyctalus va verificato come marchio su EUIPO/WIPO prima della pubblicazione. Resta da scegliere il suffisso dei siti interni.

---

## 12. Tappe

| Fase | Contenuto | Risultato |
|---|---|---|
| **0 — Specifica** | Modello di minaccia, formato dei pacchetti, protocolli | Documento tecnico v0 |
| **1 — Prototipo** | Client + 3 nodi in Rust, QUIC + Noise + Sphinx, accesso tramite proxy SOCKS5 | Si naviga con `nxs_browser` attraverso Nyctalus in laboratorio |
| **2 — Misure** | Simulazione con Shadow: confronto di velocità con Tor | Numeri reali, non promesse |
| **3 — Ragnatela** | Frammentazione multipercorso, etichette segrete, rumore di fondo | Il cuore delle prestazioni |
| **4 — Siti interni** | Punto d'incontro, indirizzi ricavati dalle chiavi | Primi servizi interni |
| **5 — Camuffamento** | Traffico indistinguibile da HTTPS | Resistenza ai blocchi |
| **6 — Rete pubblica di prova** | Integrazione in NexusSec, primi nodi volontari | Primi utenti reali |
| **7 — Browser pubblico** | Basato su Firefox ESR, tutti i sistemi operativi | Uso di massa |
| **Poi** | eBPF/DPDK per i nodi grandi, crittografia post-quantistica ibrida, app per telefoni | Ottimizzazioni |

---

## 13. Rapporto con NexusSec OS

Nyctalus Protocol è un **progetto indipendente** (repository, sito e rilasci propri, multipiattaforma) che ha in **NexusSec OS la sua casa**:
- **Perché indipendente:** una rete anonima protegge solo se la usano in tanti; se funzionasse solo dentro una distro resterebbe di nicchia.
- **Perché NexusSec:** è il banco di prova e la vetrina. Nyctalus sarà preinstallato, integrato nella modalità anonima e nel pannello di controllo, e sarà provato per primo con `nxs_browser`.

---

## 14. Verso il protocollo di riferimento

**Obiettivo dichiarato:** fare di Nyctalus Protocol il protocollo di riferimento per l'anonimato veloce. Per una rete anonima la velocità non basta: diventa di riferimento quando gli esperti **si fidano** di lei.

### Cosa serve
1. **Specifica pubblica e completa:** chiunque deve poter scrivere un'implementazione compatibile leggendo solo `SPECIFICA.md`.
2. **Codice aperto e verificabile:** licenza AGPL e test ci sono già. Mancano build riproducibili (chiunque ricompila e ottiene lo stesso identico programma) e rilasci firmati.
3. **Revisione indipendente** (il passaggio decisivo):
   - audit di sicurezza da parte di società specializzate;
   - pubblicazione accademica (PETS, USENIX Security): se i ricercatori lo attaccano e non lo rompono, diventa credibile;
   - numeri misurati, non promessi: simulazione con Shadow contro Tor, con risultati pubblicati.
4. **Utenti e nodi:** più persone usano la rete, più protegge. NexusSec è il primo canale di lancio; poi servono app semplici per tutti i sistemi operativi.
5. **Una casa neutrale:** col tempo un'associazione o fondazione no-profit che custodisca il protocollo, e la presenza nel gruppo di ricerca sulla privacy di IETF/IRTF.
6. **Finanziamenti:** in Europa **NLnet / NGI Zero** finanzia progetti open source di privacy e di rete, anche di singoli sviluppatori. Un finanziamento coprirebbe anche l'audit.

### Tappe
| Quando | Cosa |
|---|---|
| Adesso | Prototipo funzionante, specifica completa, misure con Shadow |
| Con il prototipo in mano | Pubblicazione del repository e domanda a NLnet / NGI Zero |
| Con i fondi | Audit di sicurezza, articolo scientifico, rete di prova pubblica con NexusSec |
| Poi | App per tutti, associazione no-profit, standardizzazione |

**Regola:** in questo campo la reputazione si perde in un attimo. Non si promette mai più di quanto il protocollo garantisce: la sezione 10 ("cosa promettiamo e cosa no") va mantenuta onesta anche nella comunicazione pubblica.

---

## 15. Glossario

- **Nodo:** un computer che inoltra il traffico degli altri.
- **Ingresso (guard):** il primo nodo del percorso; è l'unico che vede il tuo IP, ma non sa dove vai.
- **Uscita (exit):** l'ultimo nodo, quello che parla con il sito su Internet; sa dove vai, ma non chi sei.
- **Cipolla (onion):** cifratura a strati; ogni nodo ne toglie uno.
- **Frammento (shard):** un pezzo di dati che viaggia per conto suo.
- **Camuffamento (offuscamento):** far sembrare il traffico qualcos'altro.
- **Fingerprinting:** riconoscere un utente dalle caratteristiche del suo browser o dispositivo.
- **QUIC:** protocollo di trasporto moderno su UDP (lo usa HTTP/3).
- **Noise, Sphinx:** "ricette" crittografiche standard per strette di mano e pacchetti anonimi.
- **Sybil:** attacco in cui una sola persona controlla molti nodi finti.
