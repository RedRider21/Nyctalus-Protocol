<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Guida completa: scaricare, configurare, opzioni

Tutto in un posto, con passi brevi:
1. **[Cosa scaricare](#parte-1-cosa-scaricare)**, apparecchio per apparecchio;
2. **[I tre ruoli](#parte-2-i-tre-ruoli-client-nodo-gestore)**: chi **usa** la rete (client), chi la **fa
   funzionare** (nodo, su un server o un PC sempre acceso), chi la **tiene in piedi** (gestore);
3. **[Le opzioni](#parte-3-le-opzioni)**: a cosa serve ciascuna, cosa costa, come si accende.

Per i dettagli, ogni passo rimanda al manuale apposito.

---

## Parte 1 · Cosa scaricare

Tutto dalla **[release più recente](https://github.com/RedRider21/Nyctalus-Protocol/releases/latest)**.
Usate **la stessa versione** su tutti gli apparecchi.

| Apparecchio | File | Serve per |
|---|---|---|
| PC Linux | `Nyctalus-PC-Linux.zip` | client, gestore, e anche nodo |
| Telefono Android | `Nyctalus-Android.zip` (dentro: l'app `Nyctalus.apk` e il pacchetto per Termux) | client |
| Windows (anteprima) | `Nyctalus-Windows.zip` | client |
| Server o VPS Linux (x86_64) | `Nyctalus-Server-Linux.zip` | nodo |
| Raspberry Pi, server ARM | `Nyctalus-Server-Linux-arm64.zip` | nodo |
| Solo il motore | `nyctalus-<sistema>` (Linux, Android, Windows, macOS) | uso da riga di comando, per esperti |

Sul telefono serve anche **Termux**, da F-Droid o da GitHub (non dal Play Store):
`MANUALE-INSTALLAZIONE.md` §2.

---

## Parte 2 · I tre ruoli: client, nodo, gestore

```text
   CLIENT (tu, i tuoi amici)  ──►  NODI (3 salti, su server o PC sempre accesi)  ──►  il contatto o il sito
                                  ▲
   GESTORE: firma l'elenco dei nodi e dà a tutti il CODICE DI RETE «NYCT1:…»
```

Per **provare** basta un PC: fa da gestore, da nodi e da client insieme (rete di casa,
[§10](#10-rete-di-casa-e-via-internet)). Per una rete **vera** servono nodi su macchine di persone diverse
(`PIANO-ANONIMATO.md`).

### Client: usare la rete

**PC Linux**
1. `sudo apt install python3-gi gir1.2-gtk-3.0 gir1.2-webkit2-4.1` (una volta).
2. Scompatta `Nyctalus-PC-Linux.zip`, entra nella cartella `Nyctalus` e lancia `./installa.sh` →
   scelta **1 · Client**.
3. Collegati alla rete: `nyctalus-rete importa NYCT1:…` (il codice te lo dà il gestore).
   Controlla che l'**impronta** mostrata sia uguale a quella del gestore.
4. Apri il mini menu (**Nyctalus** nel menu delle applicazioni) → **Chat**.
5. Per navigare: il **Browser** del mini menu (scheda 🦇). In più, per il web normale senza
   farti riconoscere fra una visita e l'altra, **Nyctalus Firefox** ([§8](#8-browser-modalita-nyctalus)).

**Telefono Android**
1. Installa **Termux** e aprilo una volta.
2. Installa `Nyctalus.apk` e aprila; consenti «Esegui comandi in Termux».
3. **Strumenti ▾ → 🦇 Installa / aggiorna Nyctalus in Termux**.
4. **🔗 Rete** → incolla il codice `NYCT1:…` (oppure apri nel browser l'indirizzo della rete
   e tocca **📲 Collega l'app Nyctalus**). Controlla l'impronta.
5. **Chat**.

**Windows (anteprima)**
1. Scompatta `Nyctalus-Windows.zip`, doppio clic su `Nyctalus.cmd`.
2. **🔗 Rete** → incolla il codice. Poi **Chat**.

**Per aggiornare:** stesso pacchetto, versione nuova. PC: `./installa.sh` di nuovo. Telefono:
nuova APK, poi di nuovo il punto 3. I tuoi dati restano.

Dettagli: `MANUALE-INSTALLAZIONE.md`, `MANUALE-PC.md`, `MANUALE-TELEFONO.md`, `MANUALE-WINDOWS.md`.

### Nodo: far funzionare la rete (server o PC sempre acceso)

Un nodo **inoltra** traffico cifrato: non vede cosa passa né chi lo manda. Serve una macchina
sempre accesa con un **IP pubblico** (anche dinamico, con un nome No-IP) e le porte **UDP**
aperte.

1. Porta sul server `Nyctalus-Server-Linux.zip` (`-arm64` per Raspberry) e scompattalo.
2. Fatti dare dal gestore il **codice di rete**, poi:
   ```sh
   sudo ./installa.sh --codice NYCT1:…      # senza root: ./installa.sh --utente --codice NYCT1:…
   ```
   Chiede IP pubblico, quanti nodi e la prima porta; dal codice prende camuffamento e
   rumore di fondo della rete.
3. Se il server è dietro un router: inoltra le porte **UDP** indicate verso quella macchina.
4. L'installatore stampa delle **righe** (`IP:PORTA,chiave,impronta`): mandale al gestore.
   Non sono segrete.
5. Gestione: `nyctalus-server stato | righe | riavvia | log`.

Dettagli: `MANUALE-SERVER.md`.

### Gestore: tenere in piedi la rete

Firma l'elenco dei nodi con una chiave che **non esce mai** dal suo PC, e dà a tutti il
codice di rete.

1. Sul PC: `./installa.sh` → scelta **2 · Gestore** (è anche client).
2. **Crea la rete:**
   ```sh
   nyctalus-rete crea --nome "La nostra rete"
   ```
   Nasce **protetta** (camuffamento e rumore di fondo, [§12](#12-protezione-camuffamento-e-rumore-di-fondo)): il comando stampa la chiave per i
   nodi. Fai una copia di sicurezza di `~/.nyctalus-rete`.
3. **Aggiungi i nodi** con le righe ricevute, ognuno con la **famiglia** del suo gestore ([§13](#14-famiglie-e-nodi-di-amici)):
   ```sh
   nyctalus-rete aggiungi --famiglia mario 'RIGA1' 'RIGA2'
   nyctalus-rete firma
   nyctalus-rete verifica          # i nodi rispondono?
   ```
4. **Pubblica l'elenco** dove i client lo scaricano: `nyctalus-rete servi` su questo PC, una
   cartella web del server, o un hosting statico. Dopo ogni `firma` ricaricalo nello stesso posto.
5. **Accendi rubrica e casella** ([§11](#11-rubrica-e-casella)): `nyctalus-rete rubrica --servizio`.
6. **Crea il codice:** `nyctalus-rete codice --url https://…/directory.txt` → `NYCT1:…`.
   Dallo ai client e a chi installa i nodi, insieme all'**impronta** (`nyctalus-rete elenco`).

Il codice va rifatto solo se cambia l'URL o la protezione; aggiungere o togliere nodi non lo
cambia. Dettagli: `MANUALE-GESTORE-RETE.md`.

---

## Parte 3 · Le opzioni

Per ognuna: **a cosa serve**, **cosa costa** e i **passi** su PC, telefono (o Windows) e
terminale. Le prime sono per chi usa la chat; quelle in fondo per chi **gestisce la rete**.

**Indice delle opzioni**

| Opzione | A cosa serve | Di serie |
|---|---|---|
| [Tema e sfondo](#1-tema-e-sfondo) | l'aspetto, uguale su tutti i programmi | Notte |
| [Foto senza metadati](#2-foto-senza-metadati) | non far sapere dove hai scattato una foto | accesa |
| [Notifiche](#3-notifiche) | sapere che è arrivato un messaggio o una chiamata | accese |
| [Avvio all'accensione](#4-avvio-allaccensione-telefono) | ricevere anche dopo aver riacceso il telefono | spento |
| [Modalità riservata](#5-modalita-riservata) | non far capire **quando** né **a chi** scrivi | spenta |
| [File: soglia, collegamenti, chat](#6-programma-file) | quanto accettare da solo, velocità, traccia in chat | 100 MB · 3 · sì |
| [Rubrica della rete](#7-rubrica-della-rete) | farsi trovare dagli altri della rete | non iscritto |
| [Browser 🦇 e Nyctalus Firefox](#8-browser-modalita-nyctalus) | navigare attraverso la rete; con Firefox anche con impronta uniforme | da scegliere |
| [Terminale](#9-terminale) | chat e file scrivendo comandi | — |
| [🦇 Aiuta la rete](#9-bis-aiuta-la-rete-pc) | il tuo PC fa anche da nodo intermedio: rete più capace, i tuoi ritmi più coperti | spento |
| **Per chi gestisce la rete** | | |
| [Rete di casa / via Internet](#10-rete-di-casa-e-via-internet) | provare senza server | — |
| [Rubrica e casella](#11-rubrica-e-casella) | posta per chi è spento, elenco di chi c'è | accese nella rete di casa |
| [Protezione: camuffamento e rumore](#12-protezione-camuffamento-e-rumore-di-fondo) | traffico simile a rumore (o a un sito HTTP/3), ritmi nascosti | accesa nelle reti nuove |
| [UDP bloccato: ripiego su TCP](#13-bis-reti-con-udp-bloccato-ripiego-su-tcp) | collegarsi anche dove UDP non passa | automatico |
| [Ponti nascosti](#13-ponti-nascosti) | entrare da un indirizzo privato, fuori dall'elenco; invisibile alle sonde | nessuno |
| [Banda dei nodi](#14-famiglie-e-nodi-di-amici) | i nodi più veloci si usano di più (molto più veloce con nodi diversi) | non indicata |
| [Famiglie e nodi di amici](#14-famiglie-e-nodi-di-amici) | mai due nodi dello stesso gestore in un percorso | — |
| [Modalità rigida](#15-modalita-rigida) | rifiutare i percorsi non anonimi | spenta |
| [Opzioni del motore](#16-opzioni-avanzate-del-motore) | salti, strade parallele (per esperti) | 3 · 2 |

---

### 1. Tema e sfondo
**A cosa serve:** l'aspetto. Il tema vale per **tutti** i programmi dell'apparecchio (chat,
File, browser, mini menu). Ci sono Notte, Ardesia, Viola, Chiaro, Menta, Cielo e
**Personalizzato** (scegli un colore e chiaro/scuro; «↺ Ripristina» lo riporta all'inizio).

- **PC:** mini menu → ultimo pulsante a destra (⚙) → **Tema**; oppure nella chat, menu **Aspetto**.
- **Telefono / Windows:** **Impostazioni** → Tema; lo sfondo della chat poco sotto.

### 2. Foto senza metadati
**A cosa serve:** le foto contengono spesso la **posizione GPS** e il modello del telefono.
Con l'opzione accesa Nyctalus li toglie prima di spedire.
**Costo:** nessuno. Lasciala accesa.

- **PC:** chat → menu **Aspetto** → «Foto senza metadati».
- **Telefono / Windows:** **Impostazioni** → «Foto senza metadati».

### 3. Notifiche
**A cosa serve:** sapere che è arrivato un messaggio, un file o una chiamata. Sul telefono
funzionano **anche con l'app chiusa**, senza passare da Google. Le chiamate hanno i pulsanti
**Rispondi** (verde) e **Rifiuta** (rosso).
**Costo:** un po' di batteria sul telefono (in Android: App → Nyctalus e Termux → Batteria →
**Senza restrizioni**, altrimenti Android le ferma).

- **PC:** chat → menu **Aspetto** → «Notifiche sul desktop».
- **Telefono:** **Impostazioni** → App → «🔔 Notifiche…». La prima volta Android chiede il
  permesso: **Consenti**.

### 4. Avvio all'accensione (telefono)
**A cosa serve:** riaccendendo il telefono Nyctalus riparte da solo, così ricevi messaggi e
chiamate senza aprire l'app.
**Costo:** un po' di batteria.

- **Telefono:** **Impostazioni** → App → «⏻ Riparti da solo all'accensione».

### 5. Modalità riservata
**A cosa serve:** non far capire **quando** e **a chi** scrivi, anche in una rete piccola.
I messaggi passano dalla casella cifrata a ritmo casuale, insieme a buste finte, tutte della
stessa misura. Niente spunte e niente «in linea».
**Costo:** i messaggi arrivano con **qualche decina di secondi** di ritardo; circa 12 MB al giorno
di buste. Serve una rete **con la casella**, e anche i contatti devono avere la 0.3.3 o
successiva. Chiamate e file restano diretti.

- **PC:** chat → menu **Aspetto** → «Modalità riservata» → leggi la spiegazione → **OK**.
- **Telefono / Windows:** **Impostazioni** → Riservatezza → «🕶 Modalità riservata» → **Accendi**.
- **Terminale:** `impostazioni riservata si` (per spegnerla: `impostazioni riservata no`).

Dettagli: `MANUALE-CHAT.md` §3-ter·1.

### 6. Programma File
**A cosa serve:**
- **soglia**: i file fino a questa misura arrivano da soli, quelli più grandi chiedono
  «Accetta»;
- **collegamenti in parallelo**: più strade = più veloce, ma più carico sulla rete;
- **file anche nella chat**: i file compaiono anche nella conversazione con quel contatto.

- **PC:** programma File → ⚙ in alto.
- **Telefono / Windows:** File → ⚙.
- **Terminale:** `impostazioni soglia 200 canali 4 inchat no`.

### 7. Rubrica della rete
**A cosa serve:** un elenco di chi usa la rete, per trovarsi senza scambiarsi a mano il
Contact ID. Ti iscrivi con un nome; puoi cancellarti quando vuoi.
**Costo:** chi è iscritto vede che sei «in linea».

- **PC:** mini menu → **Rubrica** (o il pulsante 👥 nella chat) → iscriviti.
- **Telefono / Windows:** pulsante **Rubrica** → iscriviti.

### 8. Browser — 🦇 modalità Nyctalus
**A cosa serve:** navigare **attraverso la rete** (il sito non vede il tuo IP) e aprire i siti
`.nyct`. Nelle schede 🦇 non esce nulla finché la protezione non è attiva, e il browser ha
un'**impronta uniforme**: a un sito sembrano tutti uguali gli utenti di Nyctalus (ora UTC,
lingua inglese, schermo arrotondato, niente WebGL e audio, immagini disegnate non leggibili).
**Costo:** pagine un po' più lente.

- **PC:** mini menu → **Browser** → scheda 🦇.
- **Telefono:** pulsante **Browser** → scegli **🦇 Nyctalus**.

**In più, sul PC: Nyctalus Firefox.** Non sostituisce il nostro browser, che resta quello di
tutti i giorni e dei siti `.nyct`. Serve quando sul **web normale** non vuoi essere
**riconosciuto fra una visita e l'altra**. La modalità Stealth nasconde il tuo IP, ma il suo
motore (WebKitGTK) è usato da pochi, quindi un sito può riconoscerne l'«impronta». Firefox con le
protezioni di Tor Browser ti fa somigliare a milioni di altri. È il tuo Firefox, con un **profilo
separato e blindato** (il Firefox di tutti i giorni non viene toccato):
- tutto passa dalla rete, anche i nomi dei siti; se la rete è spenta **non esce niente**;
- **impronta uguale per tutti** (finestra a misure fisse, ora UTC, lingua inglese, niente
  WebGL): il browser non ti distingue dagli altri;
- niente WebRTC, posizione e sensori; niente telemetria né contatti a Mozilla o Google;
- navigazione privata: chiudendo non resta niente.

Si apre da: menu delle applicazioni → **Nyctalus Firefox**, oppure mini menu → ⚙ → **🦊
Nyctalus Firefox**, oppure il comando `nyctalus-firefox`. Se la rete non è accesa la accende
(serve una rete importata con il codice, o la chat aperta). Non installare estensioni: rendono
il browser riconoscibile.

### 9. Terminale
**A cosa serve:** chat e file scrivendo comandi (`contatti`, `scrivi mario ciao`, `file -s`…).

- **PC:** mini menu → **⌨ Terminale**.
- **Telefono:** seconda fila della schermata iniziale → **⌨ Terminale** (scrivi `aiuto`).
- **Windows:** pulsante **⌨️ Terminale**.

Dettagli: `MANUALE-TERMINALE.md`.

---

### 9-bis. Aiuta la rete (PC)
**A cosa serve:** il tuo PC fa anche da **nodo intermedio** della rete: passa il traffico cifrato
degli altri. Più utenti aiutano, più la rete è capace; e il traffico degli altri copre in parte i
tuoi ritmi.

**Regole fisse:** solo **intermedio**, mai uscita (il traffico degli altri non esce **mai** su
Internet dal tuo indirizzo: il nodo rifiuta anche se un client glielo chiede), mai guardia né
punto d'incontro (un PC si spegne). Un **tetto di banda** che scegli tu.

**Costo:** un po' di banda in upload (fino al tetto) e una porta UDP da aprire sul router.

**Passi:**
- **PC:** launcher → ⚙ → **🦇 Aiuta la rete** → indirizzo pubblico (il nome No-IP/DuckDNS o l'IP di
  casa), porta (4800), tetto (es. 10 Mbit/s) → **Accendi**. Oppure da terminale:
  `nyctalus-rete aiuta si --indirizzo miacasa.ddns.net --tetto 10` · `aiuta stato` · `aiuta no`.
- **Router:** inoltra la porta UDP scelta verso il PC.
- **Gestore:** riceve la riga che il PC mostra e la aggiunge con
  `nyctalus-rete aggiungi 'RIGA' --intermedio --banda 10 --famiglia NOME`, poi `firma`
  (serve che i client abbiano la 0.3.11).

Misurato in simulazione: con i nodi della rete saturi l'aiuto come soli intermedi accelera poco
(+5%), perché la strettoia sono entrata e uscita; conta quando gli utenti sono molti.

### Per chi gestisce la rete

I comandi sono di `nyctalus-rete`, sul PC del gestore. Dopo ogni cambio che tocca il
**codice di rete**, va ridato a chi si collega (importandolo di nuovo).

### 10. Rete di casa e via Internet
**A cosa serve:** provare tutto senza server: il tuo PC tiene accesi 4 nodi, l'elenco, la
rubrica e la casella.

```sh
nyctalus-rete casa                       # nella tua Wi-Fi
nyctalus-rete casa --internet NOME       # raggiungibile da Internet (NOME = No-IP o IP pubblico)
```
Passo passo: `GUIDA-RETE-DI-CASA.md` e `GUIDA-INTERNET.md`. Con i nodi tutti su un PC la rete
**non è anonima** (il motore lo dice nei registri).

### 11. Rubrica e casella
**A cosa serve:** la **casella cifrata** tiene i messaggi per chi è spento (e serve alla
modalità riservata); la **rubrica** è l'elenco di chi c'è. Nella rete di casa sono già accese.
Per una rete su server:

```sh
nyctalus-rete rubrica --servizio         # sempre accese su questo PC (o con --codice NYCT1:… sul server)
nyctalus-rete codice --url URL           # poi il codice le include
```

### 12. Protezione: camuffamento e rumore di fondo
**A cosa serve:**
- **camuffamento**: ogni pacchetto è reso simile a rumore con una chiave della rete. Chi
  guarda il traffico non riconosce Nyctalus, e chi non ha la chiave non riesce a collegarsi.
  Dalla 0.3.8 un nodo camuffato **non risponde a nessuna sonda** (prima rispondeva circa una
  volta ogni 300 tentativi, e così si faceva scoprire);
- **rumore di fondo**: celle finte a ritmo casuale, così i momenti in cui usi davvero la
  rete si notano meno.

**Costo:** il rumore consuma banda (di serie 0,2 celle al secondo, circa 20 MB al giorno). La
chiave viaggia nel codice di rete: chi ha il codice ce l'ha.
**Di serie:** accesa nelle **reti create dalla 0.3.4**. Le reti più vecchie restano come sono
finché non la accendi (poi va ridato il codice a tutti).

```sh
nyctalus-rete protezione                       # com'è adesso (e la chiave per i server)
nyctalus-rete protezione si [--rumore 0.5]     # accende (la rete principale)
nyctalus-rete protezione si --casa             # … la rete di casa  (--internet: quella via Internet)
nyctalus-rete protezione no                    # spegne (la chiave resta da parte)
```
Dopo il cambio: riavvia la rete (o la rete di casa), ridai il codice e reinstalla i nodi dei
server con `./installa.sh --codice NYCT1:…` (oppure `--camuffamento CHIAVE --rumore N`). Client
PC, telefono e Windows prendono tutto dal codice, da soli.

**Due forme, a scelta del gestore** (dalla 0.3.9):

| Forma | Cosa vede chi osserva | Quando usarla |
|---|---|---|
| **rumore** (di serie) | pacchetti UDP che sembrano rumore puro; chi sonda non riceve risposta | quasi sempre |
| **HTTP/3** (`--forma h3`) | un normale **sito HTTP/3**: QUIC sulla porta 443, con il nome che scegli tu (es. `www.miosito.it`); chi sonda il nodo riceve una pagina «404 Not Found», come da un sito qualunque | dove il traffico «tutto rumore» viene bloccato (alcuni paesi, alcune reti aziendali) |

```sh
nyctalus-rete protezione si --forma h3 --nome www.miosito.it   # forma HTTP/3
nyctalus-rete protezione si --forma rumore                     # di nuovo rumore
```
Nella forma HTTP/3 la chiave della rete non cifra i pacchetti: diventa un **contrassegno** che i
client mandano dentro la cifratura TLS. Chi non lo ha vede solo il «sito». Sui server:
`sudo ./installa.sh --codice NYCT1:… --nuova-identita` (porta 443 proposta da sola; la nuova
identità dà ai nodi un certificato con il nome del sito, quindi **rimanda le righe al gestore**).
I nodi della rete di casa si rifanno l'identità da soli. **Limite onesto:** chi analizza a fondo la
stretta di mano può capire che il programma non è un browser (impronta TLS/QUIC diversa da quella
di Chrome o Firefox); il nome scelto è solo un'etichetta (non serve possedere il dominio, ma un
dominio vero tuo è più credibile).

### 13. Ponti nascosti
**A cosa serve:** di solito si entra nella rete da un nodo dell'**elenco pubblico**: chi osserva
la tua linea (il fornitore Internet, la rete dell'ufficio, un paese che censura) può confrontare
l'indirizzo con l'elenco e capire che usi Nyctalus. Un **ponte** è un'entrata **privata**:

- non è nell'elenco, quindi il suo indirizzo non dice niente;
- ha una **chiave tutta sua**: chi lo sonda senza la chiave (anche chi ha il codice della rete)
  non riceve **nessuna risposta**, come se l'indirizzo fosse spento;
- con almeno un ponte si entra **sempre** da lì, mai dai nodi dell'elenco.

**Costo:** nessuno in velocità (il ponte prende il posto della guardia). Serve qualcuno che
tenga acceso il ponte. La **riga del ponte è segreta**: chi la ha può trovarlo.
**Di serie:** nessun ponte.

**1) Chi ospita il ponte** (un server o un PC sempre acceso, anche di un amico):
```sh
sudo ./installa.sh --ponte --codice NYCT1:…      # pacchetto server; stampa la RIGA DEL PONTE
nyctalus-server righe                             # la ristampa
```
La riga (`IP:PORTA,chiave,impronta,chiave-del-ponte`) **non** va al gestore della rete: va
solo, a mano, a chi deve entrare da lì.

**2) Chi usa il ponte:**
- **PC:** `nyctalus-rete ponte aggiungi 'RIGA'` · `nyctalus-rete ponte elenco` ·
  `nyctalus-rete ponte togli IP:PORTA` (o `tutti`). Poi chiudi e riapri chat e browser.
- **Telefono / Windows:** ⚙️ Impostazioni → **Rete condivisa** → **🌉 Ponti nascosti** →
  incolla la riga → **Aggiungi** (la rete riparte da sola).

I ponti valgono **per la rete** a cui li hai aggiunti: cambiando rete restano da parte. Il
motore li riceve per variabile d'ambiente, mai sulla riga di comando (che altri utenti del
computer potrebbero leggere). A mano: `NYCTALUS_PONTI='RIGA' nyctalus start …` oppure
`--bridge 'RIGA'`; un nodo diventa ponte con `nyctalus node … --hidden-bridge`.

### 13-bis. Reti con UDP bloccato (ripiego su TCP)
**A cosa serve:** Nyctalus viaggia su UDP (QUIC). Alcune reti (aziendali, Wi-Fi pubblici, alcuni
paesi) bloccano UDP: lì il client, se l'entrata non risponde in **6 secondi**, passa **da solo** a
**TCP sulla stessa porta**. Sopra non cambia niente: cipolla, ragnatela, camuffamento.
**Costo:** su TCP un pacchetto perso ferma tutto il collegamento (come in Tor): più lento con
reti instabili. È un ripiego, non la strada normale.
**Di serie:** acceso (automatico). `NYCTALUS_TCP=1` per usare sempre TCP, `NYCTALUS_TCP=0` per non
usarlo mai. I nodi ascoltano sempre anche in TCP; sui server l'installatore apre le porte in UDP
e TCP (nella rete di casa, sul router inoltra anche la porta TCP se vuoi il ripiego).
Chi sonda la porta TCP con dati a caso non riceve niente e il nodo non chiude: nessuna impronta.

### 14. Famiglie e nodi di amici
**A cosa serve:** un percorso fa 3 salti. Se due salti sono **dello stesso gestore**, lui
vede entrata e uscita. Con le **famiglie** dichiari quali nodi sono della stessa persona o
dello stesso fornitore: un percorso non ne userà mai due insieme. Servono **almeno 3
famiglie** perché ogni percorso sia di 3 gestori diversi.

```sh
# rete su server
nyctalus-rete aggiungi --famiglia mario 'RIGA1' 'RIGA2'
nyctalus-rete famiglia IP:PORTA NOME|-           # mette o toglie un nodo da una famiglia
nyctalus-rete firma
# rete di casa: i nodi di un amico, ognuno con la sua famiglia (i tuoi = «casa»)
nyctalus-rete ospite aggiungi 'RIGA' … --famiglia mario [--internet]
nyctalus-rete ospite elenco [--internet]
```
Le righe dei nodi le stampa l'installatore sulla macchina dell'amico. Le famiglie vanno
usate quando **tutti** hanno la 0.3.3 o successiva. Dettagli: `MANUALE-GESTORE-RETE.md`.

**Banda dei nodi** (dalla 0.3.10). Indica quanta banda ha ogni nodo (in Mbit/s, quella reale in
upload del server): i percorsi sceglieranno più spesso i nodi più capaci, come fa Tor. Con nodi
di velocità diverse è il miglioramento più grande: in simulazione da 3,9 a 9,9 Mbit/s (Tor 6,2).
```sh
nyctalus-rete banda IP:PORTA 100      # 100 Mbit/s;  «-» per toglierla
nyctalus-rete firma
```
I nodi senza banda contano come la media degli altri. ⚠️ Un elenco con le bande lo leggono i
motori dalla **0.3.10**: aggiorna prima tutti i client (quelli vecchi rifiuterebbero l'elenco).

### 15. Modalità rigida
**A cosa serve:** se la regola delle famiglie non si può rispettare (per esempio nodi tutti
su un PC), di solito il motore **funziona lo stesso** e avvisa «cifrato ma NON anonimo». In
modalità rigida invece **si rifiuta**: meglio niente che un percorso non anonimo.

```sh
NYCTALUS_PARENTELA=rigida nyctalus-chat          # o davanti a qualunque comando del motore
```

### 16. Opzioni avanzate del motore
Per chi usa il motore a mano (`nyctalus start`, `nyctalus circuit`…). Tutte in `DEPLOY.md`.

| Opzione | A cosa serve | Valori |
|---|---|---|
| `--hops N` | nodi per percorso; sotto 3 niente anonimato | 1-5, di serie 3 |
| `--circuits N` | strade parallele (più veloce) | 1-8, di serie 2 |
| `--cover HZ` | rumore di fondo (celle finte al secondo) | di serie 0, o quello della rete |
| `--obfuscation HEX` | camuffamento con la chiave della rete | quella del codice |
| `--bridge RIGA` / `NYCTALUS_PONTI` | entrare da un ponte nascosto | nessuno |
| `--mimic [--mimic-name SITO]` / `NYCTALUS_MIMETICO=1` | forma HTTP/3 (con `--obfuscation`) | spenta |
| `--hidden-bridge` (nodo) | il nodo fa da ponte nascosto | spento |
| `--middle-only` (nodo) | solo intermedio: mai uscita («Aiuta la rete») | spento |
| `--max-rate MBIT` (nodo) | tetto della banda inoltrata | nessuno |
| `NYCTALUS_TCP=1` / `=0` | entrata sempre in TCP / mai (di serie: TCP solo se UDP non risponde) | automatico |
| `NYCTALUS_RIUSO=0` (nodo) | un collegamento per circuito invece di uno per coppia di nodi | riuso acceso |
| `NYCTALUS_FINESTRA_KB` | finestra per circuito (KB): più piccola = la ragnatela sente prima le strade lente | 256 |
| `NYCTALUS_CONGESTIONE` | controllo di congestione: `bbr`, `cubic`, `newreno` | bbr |
| `NYCTALUS_CODIFICA=8,2` | ragnatela codificata **sperimentale** (k celle + r riserve); misurata: non accelera | spenta |
