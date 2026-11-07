<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Piano verso una rete anonima

Nyctalus **cifra** già tutto da un capo all'altro. L'**anonimato**, cioè che nessuno sappia
*chi parla con chi*, non dipende però solo dal programma. Dipende da **dove stanno i nodi**,
da **chi li gestisce** e da **quanta gente usa la rete**. Questo piano dice, tappa per tappa,
cosa si ottiene e cosa manca ancora.

## In breve

| Tappa | Cosa si fa | Cosa si ottiene | Cosa manca ancora |
|---|---|---|---|
| 0 · oggi | nodi tutti sul PC di casa | funziona, tutto cifrato | anonimato: nessuno |
| 1 | 3 piccoli server su 3 fornitori diversi | gli operatori non vedono più chi parla con chi | i nodi sono tutti tuoi |
| 2 ✅ | entrata e uscita scelte a caso, mai due nodi «parenti» | nessun nodo è fisso per tutti | i nodi sono ancora di una sola persona |
| 2-bis ✅ | **modalità riservata** per la chat: messaggi solo tramite casella, a ritmo casuale, con buste finte e tutte uguali | non si capisce **quando** né **a chi** scrivi, anche con pochi nodi | chiamate e file restano diretti |
| 3 🔧 | nodi gestiti da persone diverse | nessuno controlla tutto il percorso | servono più utenti |
| 4 🔧 | più utenti, rumore di fondo e camuffamento sempre accesi | ci si confonde fra tanti, i ritmi si vedono meno | misure e revisione |
| 5 | misure con la simulazione, browser pubblico | numeri veri, navigazione con impronta uniforme | revisione esterna |
| 6 | revisione indipendente, pentest | ci si può fidare davvero | — |

## Tappa 0 · Oggi: la prova di funzionamento

I 4 nodi e l'elenco stanno sul PC di chi ospita la rete (`GUIDA-INTERNET.md`).

- ✅ **Cifratura completa:** messaggi, file, chiamate e video non sono leggibili né dagli
  operatori né dai nodi. Lo verificano le prove, compresa «Wireshark in mezzo» (`SICUREZZA.md`).
- ✅ **Contatti autenticati:** nessuno può fingersi un tuo contatto.
- ❌ **Anonimato:** chi ospita vede l'IP degli altri, e gli operatori vedono che due
  indirizzi si parlano, quando e quanto.
- ❌ **Invisibilità:** si vede che c'è traffico cifrato verso un indirizzo.

Va benissimo per **provare**: collegamento, router, velocità, chiamate, file.

## Tappa 1 · Tre piccoli server su tre fornitori diversi

**Cosa si fa**
- **3 VPS** (server virtuali), uno per fornitore, possibilmente in **paesi diversi**. Basta il
  più piccolo: 1 CPU, 512 MB–1 GB di RAM, IP pubblico, porte UDP apribili; costa pochi euro al
  mese ciascuno.
- Su ognuno, **un nodo** con `Nyctalus-Server-Linux.zip` (`MANUALE-SERVER.md`). Il tuo PC può
  restare il quarto nodo, oppure no.
- Il **gestore** (la chiave che firma l'elenco) resta sul tuo PC: non va mai su un server.
- **Camuffamento** (`--obfuscation`) e **rumore di fondo** (`--cover`) accesi su tutti
  (`DEPLOY.md`).
- **Rubrica e casella** su uno dei server, così la posta per chi non è in linea non passa da
  casa tua.

**Cosa si ottiene:** tu e il tuo amico vi collegate a **server**, non l'uno all'altro.
Gli operatori di casa vedono «traffico cifrato verso un server», non «Daniele parla con
Mario». Il tuo IP non è più quello della rete.

**Cosa manca:** i nodi sono **tutti tuoi**. Tu, o chi riuscisse a entrare nei tuoi server,
potresti mettere insieme entrata e uscita. Ogni fornitore vede il traffico del proprio
server (cifrato), e l'abbonamento ai VPS è intestato a qualcuno.

## Tappa 2 · Percorsi davvero casuali (lavoro sul motore) ✅ fatta (07/11/2026)

Prima ogni circuito usava **il primo nodo dell'elenco come entrata e il secondo come
uscita**, e verso i siti `.nyct` (chat, file) l'entrata cambiava a ogni collegamento. Ora:

- **entrata scelta a caso** e poi tenuta a lungo (come le «guardie» di Tor), così non si
  ricomincia ogni volta il rischio di pescare un'entrata malevola;
- **uscita** e **nodi intermedi** scelti a caso;
- **regola di parentela:** mai due nodi dello stesso fornitore o della stessa sottorete nello
  stesso percorso, e mai due nodi che il gestore ha dichiarato «della stessa persona»;
- prove automatiche che controllino la distribuzione e la regola.

**Cosa si ottiene:** nessun nodo vede passare tutti, e un percorso non attraversa mai due
volte lo stesso proprietario dichiarato.

**Come si usa:** il gestore dichiara le famiglie con `nyctalus-rete aggiungi --famiglia NOME`
o `nyctalus-rete famiglia IP:PORTA NOME` (`MANUALE-GESTORE-RETE.md`). Se la regola non si può
rispettare (per esempio nodi tutti sul PC di casa) il motore funziona ma scrive un **avviso**
«cifrato ma NON anonimo»; con `NYCTALUS_PARENTELA=rigida` si rifiuta. La guardia sta in
`~/.nyctalus/guardie/` (un file privato per rete). Verificato dai test del motore e dalla prova
`25-percorsi`.

## Tappa 2-bis · Modalità riservata (chat) ✅ fatta (07/11/2026)

Anche con pochi nodi e pochi utenti si può nascondere **quando** e **a chi** scrivi, con le
tecniche delle «mixnet» (Loopix, Nym), al prezzo di qualche secondo di ritardo e di un po' di
banda. È un interruttore nelle impostazioni della chat.

- **Mai in diretta:** i messaggi non vanno al contatto, vanno nella **casella cifrata** della
  rete; lui la ritira a intervalli fissi. Mittente e destinatario non sono mai collegati nello
  stesso momento: guardare i tempi ai due capi non dice più niente.
- **Ritmo casuale, sempre uguale:** il motore deposita una busta ogni ~30 secondi in media
  (ritmo di Poisson), che tu scriva o no. Se c'è un tuo messaggio parte quello, altrimenti
  una **busta finta** indirizzata a te stesso («giro a vuoto»). Chi osserva vede lo stesso
  ritmo quando scrivi e quando non scrivi.
- **Buste tutte uguali:** riempite alla stessa misura (4 KB), vere o finte.
- **Niente ricevute né «in linea»:** in modalità riservata non mandi le spunte di consegna e
  lettura e non sondi i contatti, perché sarebbero collegamenti diretti e rivelerebbero i
  tempi. I tuoi messaggi arrivano a 📮 (nella casella).
- **I giri a vuoto** servono anche da controllo: se le buste finte non tornano, la casella o
  un nodo stanno perdendo posta.

**Cosa NON copre:** chiamate e file grandi (in tempo reale o troppo pesanti per la casella:
restano diretti, con un avviso), e il fatto che sei collegato (il tuo servizio `.nyct` resta
acceso per ricevere chiamate). Serve una rete **con la casella** (`nyctalus-rete rubrica`).
I contatti devono avere la versione 0.3.3 o successiva.

**Misurata in Shadow** (punto K): con la chat normale chi vede tutte le linee scopre il 100% delle
coppie; con la modalità riservata non va oltre il caso.

**Come si usa:** `MANUALE-CHAT.md` §3-ter·1. Verificato dalla prova `26-riservata`: messaggio
arrivato identico, nessuna ricevuta, buste finte spedite e tornate, **tutte le buste della
stessa misura** nella casella, rubrica che non consegna subito.

**Più avanti:** ritardi casuali anche **nei nodi** (mixing vero e proprio) e traffico di
copertura a ritmo costante fra client e guardia, utile anche per le chiamate ma costoso in
banda.

## Tappa 3 · Nodi di persone diverse 🔧 strumenti pronti (07/11/2026)

- **I PC come nodi intermedi** (scelta iniziale del progetto: ogni PC anche nodo): in una rete
  piccola moltiplica i nodi senza comprare server. I telefoni restano solo client.
- Altri gestiscono **i propri nodi**: il tuo amico di Pavia (ha l'IP statico, è perfetto),
  altri amici, volontari. Ognuno con il suo server o il PC sempre acceso.
- Il gestore li aggiunge all'elenco con la loro «famiglia» (Tappa 2).
- Regola d'oro: **nessuno gestisce più di un terzo dei nodi**.

**Gli strumenti ci sono:** l'installatore del nodo stampa le righe da mandare al gestore;
il gestore le aggiunge con la famiglia (`nyctalus-rete aggiungi --famiglia`, oppure, nella
rete di casa, `nyctalus-rete ospite aggiungi … --famiglia NOME`); la rete di casa marca i
propri nodi come famiglia «casa». Verificato dalla prova `27-nodi-ospiti`: con 3 gestori,
ogni percorso usa 3 gestori diversi e il traffico passa davvero dai nodi degli amici. Il resto
è organizzazione: trovare chi ospita i nodi (`GUIDA-INTERNET.md`, Passo 8).

**Cosa si ottiene:** per scoprire chi parla con chi bisogna mettere d'accordo i gestori di
entrata e uscita, che sono persone diverse.

## Tappa 4 · Più utenti, meno ritmi riconoscibili 🔧 protezione di serie fatta (07/11/2026)

- **Più persone** usano la rete: l'anonimato è nascondersi fra tanti. Con 2 utenti, «chi parla
  con chi» si indovina; con 200 molto meno.
- **Rumore di fondo** sempre acceso: le chiamate hanno un ritmo che si nota (circa 40 volte il
  traffico a riposo, misurato dalla prova del traffico); il rumore lo copre, a costo di banda.
- **Camuffamento** obbligatorio su tutta la rete.
- **Rubrica e casella** su server di persone diverse.

**Fatto:** le reti nuove nascono con **camuffamento** e **rumore di fondo** accesi (0,2 celle
finte al secondo); il codice di rete li porta a PC, telefono, Windows, rubrica, casella e
all'installatore dei nodi (`--codice`). Le reti vecchie si accendono con
`nyctalus-rete protezione si`. Verificato dalla prova `28-protezione`: con la chiave ci si
collega, senza no. Resta da fare la parte «più utenti», che è organizzativa.

## Tappa 5 · Misure e browser pubblico

- **Simulazione con Shadow:** una rete finta con centinaia di nodi e utenti, per misurare
  quanto è facile collegare entrata e uscita guardando i tempi, quanto è grande il «gruppo in
  cui ci si confonde», e confrontare tutto con Tor.
  - ✅ **Prime misure (07/11/2026)**, con il codice vero (`simulazione/LEGGIMI.md`):
    - **scelta dei percorsi**: con la Tappa 2 un singolo gestore disonesto non collega mai
      chi con chi (prima: 0% o 100% a seconda dell'ordine dell'elenco); due gestori d'accordo
      o 4 nodi infiltrati collegano ancora il 5-8% dei circuiti → servono molti gestori;
    - **Nyctalus vero in Shadow**: 2 MB in ~2,1 s su 3 salti con 30 ms fra i computer;
    - **attacco di correlazione dei tempi**: chi vede le linee degli utenti e del sito
      attribuisce giusti circa l'**80%** degli scaricamenti (a caso 17%), e il rumore di fondo a
      celle sparse non cambia molto. È il limite di tutte le reti in tempo reale, Tor compreso.
    - **chi scrive a chi** con un avversario che vede tutte le linee: chat normale **100%**
      delle coppie scoperte, **modalità riservata al livello del caso** (17-33%, a caso 20%);
    - la simulazione ha scoperto e fatto correggere un limite vero: ogni servizio `.nyct`
      serviva al massimo 4 client insieme.
    - **confronto con Tor 0.4.9** nella stessa simulazione: sugli scaricamenti siamo alla pari
      (l'avversario globale indovina 91% con Tor, 80% con Nyctalus; stessa velocità). Dove andiamo
      oltre Tor è la chat: con la modalità riservata l'avversario resta al livello del caso.
- **Browser pubblico** basato su Firefox, con impronta uniforme per tutti (finestre a
  misure fisse, canvas e caratteri uniformati), per navigare sul web normale senza essere
  riconoscibili dal browser.
  - ✅ **Fase 1 (07/11/2026): Nyctalus Firefox.** Il Firefox già installato, con un profilo
    separato e blindato (proxy anche per i nomi, mai diretto, `resistFingerprinting` e finestre a
    misure fisse, siti isolati, niente WebRTC/WebGL/telemetria, navigazione privata). Verificato
    con Firefox vero dalla prova `29-firefox`.
  - Fase 2 (scelta dell'utente per DOPO): **Mullvad Browser** incluso nel pacchetto, con la
    rete Nyctalus. È Tor Browser senza Tor (Tor Project + Mullvad): tutte le protezioni del motore
    di Tor Browser e impronta identica a milioni di utenti. Si apre come «Nyctalus Browser
    (Firefox)» con il nostro logo su menu, barra e pagina iniziale; dentro il programma restano
    nome e logo di Mullvad. Download ~105 MB, verificato con la firma ufficiale.
  - Più avanti, se servisse il nostro logo anche dentro: un Firefox compilato da noi come Tor
    Browser (pesante: compilazioni e aggiornamenti di sicurezza ogni 4 settimane).

## Nyctalus e Tor, a confronto

**Misurato** con Shadow (il simulatore usato nella ricerca su Tor): Tor 0.4.9 e Nyctalus veri
sulla stessa Internet simulata (30 ms fra due computer, linee 50/20 Mbit), stessi utenti e
stesso attacco, 3 ripetizioni (velocità: `simulazione/velocita.py`, file da 5 MB). Come rifarla: [simulazione/LEGGIMI.md](https://github.com/RedRider21/Nyctalus-Protocol/blob/main/simulazione/LEGGIMI.md).

| Misura | Tor 0.4.9 | Nyctalus |
|---|---|---|
| Scaricare 5 MB, linea pulita | 6,4 Mbit/s | **~10 Mbit/s · +55%** |
| … con l'1% di pacchetti persi (Wi-Fi, rete mobile) | 2,3 Mbit/s | 2,2-2,4 · alla pari (prima della 0.3.8: 1,3) |
| … con il 2% di pacchetti persi | 0,47 Mbit/s | **0,98 · ×2** |
| … nodi carichi (4 utenti insieme su nodi da 10 Mbit) | **2,9 Mbit/s** | 2,0-2,5 · Tor avanti |
| … nodi carichi e 1% di perdite | 1,4 Mbit/s | **1,8 · +25%** |
| Primo byte | 0,49 s | 0,50 s · alla pari |
| Avversario che vede tutte le linee: scaricamenti attribuiti a chi li ha fatti | 91% | 80% · alla pari (limite di tutte le reti in tempo reale) |
| Chat: lo stesso avversario scopre chi scrive a chi | — (Tor non ha una chat sua) | normale 100%; **modalità riservata al livello del caso** (17-33%, a caso 20%) |
| Un gestore di nodi disonesto collega entrata e uscita | no (famiglie e guardie) | no: 0% (famiglie e guardie, dalla 0.3.3) |

**Cosa c'è:**

| | Tor | Nyctalus |
|---|---|---|
| Maturità | 20 anni, revisioni indipendenti, milioni di utenti, migliaia di nodi | prototipo **non ancora revisionato**, reti piccole |
| Trasporto | TCP | QUIC/UDP (un pacchetto perso non ferma gli altri) |
| Più strade insieme | sì (conflux, dalla 0.4.8) | sì (2 circuiti di serie, fino a 8) |
| Camuffamento | ponti con obfs4, Snowflake, meek: collaudati contro la censura | chiave della rete di serie (rumore), oppure forma HTTP/3; ponti nascosti; nessuna risposta alle sonde |
| Rumore di fondo | limitato | regolabile per rete; buste finte nella modalità riservata |
| Chat, chiamate, videochiamate | con app esterne | integrate, con modalità riservata |
| File grandi | con app esterne (OnionShare) | integrati: pezzi verificati, più strade, pausa e ripresa |
| Browser | Tor Browser: il riferimento | impronta uniforme fra utenti Nyctalus + Nyctalus Firefox |
| Siti interni | `.onion` | `.nyct` |
| Linguaggio | C | Rust (meno errori di memoria) |

In breve: **velocità**: su linea pulita Nyctalus è più veloce di Tor, con perdite alla pari o
meglio; con nodi carichi Tor è ancora avanti. **Anonimato** sul tempo reale alla pari; sulla
**chat**, con la modalità riservata, va oltre. Tor resta avanti in maturità, numero di utenti e
camuffamento collaudato.

## Velocità: dove siamo e cosa viene dopo

Misure in Shadow con `simulazione/velocita.py` (dettagli in `simulazione/LEGGIMI.md`):

1. ✅ **BBR** (0.3.8): con i pacchetti persi non si dimezza più la velocità.
2. ✅ **Ragnatela con distributore intelligente** e **finestra per circuito di 256 KB** (0.3.10):
   ogni cella va alla strada più libera, le strade lente si sentono subito.
3. ✅ **Scelta dei nodi pesata per banda** (0.3.10): il guadagno più grande con nodi di velocità
   diverse (come nella realtà).
4. ❌ **Ragnatela codificata** (riserve con codice MDS): provata e misurata, **non accelera**
   (-5..-25%) perché ogni tratta QUIC recupera già i pacchetti persi e le strade condividono entrata
   e uscita. Resta sperimentale; da riprovare con uscite diverse per strada.
5. **«🦇 Aiuta la rete»** (prossimo): i PC degli utenti, se vogliono, fanno da **nodo intermedio**
   (mai uscita), con un tetto di banda. Più utenti, più capacità: la differenza vera con Tor nel
   mondo reale. In più il traffico degli altri copre in parte i tuoi ritmi (da misurare).
6. Uscite diverse per strada, per gli scaricamenti grandi (da pesare contro l'anonimato).

## Oltre l'anonimato: invisibilità

Anonimo vuol dire che non si sa **chi parla con chi**. Invisibile vuol dire che non si capisce
nemmeno **che** stai usando una rete anonima, né **quando** e **quanto** comunichi. Qui Tor è
debole, e le misure del punto K lo confermano anche per noi. I passi, in ordine:

1. ✅ **Ponti nascosti** (fatto il 07/11/2026, 0.3.8). Entrate private, non nell'elenco
   pubblico, date a mano a chi serve: chi ti osserva non vede «un nodo Nyctalus», vede un
   indirizzo qualunque. Ogni ponte ha una chiave sua, e chi lo sonda senza chiave **non riceve
   risposta** (lo stesso ora vale per tutti i nodi camuffati: prima uno su ~300 tentativi
   otteneva risposta). Nel primo pacchetto non c'è più nessun nome. Come si usa:
   `GUIDA-COMPLETA.md` §13; prova `32-ponte-nascosto`.
2. 🔧 **Trasporto mimetico** — ✅ **forma HTTP/3 fatta** (0.3.9): i nodi sembrano un sito HTTP/3
   sulla porta 443 con il nome scelto dal gestore, e chi li sonda (anche un vero Chrome) riceve una
   pagina 404. Restano: impronta identica a un browser, forma «videochiamata», ripiego su TCP 443.
   Il camuffamento di serie rende il traffico simile a rumore, e il rumore
   puro è sospetto (in alcuni paesi viene bloccato proprio così). Meglio farlo sembrare
   **traffico comune**: una normale videochiamata (WebRTC) o un normale sito HTTPS/HTTP-3
   (con ALPN `h3`, porta 443, dimensioni dei pacchetti come i browser), più il ripiego su TCP 443
   dove UDP è bloccato. Cambia la stretta di mano: va con un salto di versione del protocollo.
3. **Chat da mixnet.** La modalità riservata è il primo passo. Il prossimo: i nodi trattengono
   e **rimescolano** le buste con ritardi casuali, e si aggiungono buste finte calcolate apposta
   (privacy differenziale, come Vuvuzela/Karaoke e Loopix). Così nemmeno chi vede **tutta
   Internet** capisce chi scrive a chi. Per i messaggi è il punto in cui si può **superare Tor**.
4. **Copertura costante durante l'uso.** Il traffico resta piatto anche quando chiami o
   scarichi. Le misure dicono che la copertura deve essere **pari al traffico**, non a celle
   sparse: costa molta banda, quindi è un'opzione per chi ne ha bisogno.
5. **Igiene automatica.** Ritardi casuali sui messaggi (già nella modalità riservata), niente
   dati del dispositivo in uscita, attenzione agli orari di attività.

Il limite che resta: per chiamate e navigazione **in tempo reale** chi vede entrambi i capi
può confrontare i tempi. Nessuna rete al mondo lo evita senza enormi quantità di banda.

## Tappa 6 · Revisione indipendente

- **Revisione crittografica** da esperti esterni (la combinazione Sphinx + strati simmetrici
  + Noise + QUIC), pubblicazione a PETS/USENIX, analisi formale.
- **Pentest** pubblico e **bug bounty**.
- Possibili fondi: NLnet / NGI Zero.

Solo dopo questa tappa ha senso dire «fidatevi». Fino ad allora Nyctalus è un **prototipo
da provare**, non uno strumento da cui far dipendere la propria sicurezza.

## Cosa nessuna tappa può dare

- **Invisibilità totale.** Si può rendere il traffico simile a rumore o a un altro protocollo
  (`--obfuscation`, in futuro i «ponti»), ma chi guarda il filo vede comunque che passano dati.
  Vale anche per Tor.
- **Protezione da un apparecchio compromesso.** Se il telefono o il PC hanno un programma
  spia, la rete non può farci niente.
- **Protezione da ciò che scrivi.** Se in chat dici il tuo nome, la rete non lo nasconde. Lo
  stesso vale per le foto con i dati di posizione (Nyctalus li toglie da sé, ma una foto
  condivisa fuori dall'app no).
- **Un avversario che vede tutta Internet** e confronta i tempi di entrata e di uscita: il
  rumore di fondo lo ostacola, ma non lo rende impossibile. È il limite noto di tutte le reti
  a bassa latenza, Tor compreso.

## Cosa fare adesso

1. **Prova con il tuo amico** così com'è (Tappa 0), per collegamento, velocità, chiamate e file.
2. **Tappa 1:** scegliete 3 fornitori di VPS diversi e installate i nodi. Il tuo amico può
   ospitarne uno.
3. **Tappa 2** ✅ fatta: quando avrete i server, dichiarate le famiglie (un nome per
   persona o fornitore) con `nyctalus-rete famiglia`.
