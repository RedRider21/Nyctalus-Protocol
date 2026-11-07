<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol. -->

# Verificare sicurezza e anonimato

Questo documento spiega **cosa si può verificare da soli** (con gli strumenti
qui indicati e lo script [`strumenti/verifica.sh`](../strumenti/verifica.sh)) e **cosa
invece richiede una verifica indipendente** prima di considerare Nyctalus sicuro
per dati reali. Niente di qui sostituisce un **audit** di crittografi terzi.

> Principio: fidati, ma **verifica**. Le proprietà sotto sono testabili; le
> conclusioni forti sull'anonimato richiedono misure su rete reale e revisione.

---

## 1. Verifiche automatiche (subito, in locale)

```sh
./strumenti/verifica.sh
```
Controlla, e riporta PASS/FAIL:

1. **Correttezza del protocollo** — `cargo test` (tutti i test automatici: celle a
   lunghezza fissa = pacchetto Sphinx, cifratura, stretta Noise, indirizzi,
   conflux, rendezvous…).
2. **Firma della directory** — una directory **manomessa** viene **rifiutata**
   (il byte cambiato invalida la firma Ed25519 dell'autorità).
3. **Autorità sbagliata** — una directory firmata da **un'altra autorità** viene
   **rifiutata** (il client accetta solo la chiave attesa).
4. **Camuffamento obbligatorio** — con `--obfuscation`, un client **senza la
   chiave di rete giusta non riesce a connettersi**: sul filo il traffico è
   indistinguibile da rumore (nessuna firma di QUIC riconoscibile).

Questi quattro punti provano proprietà concrete: integrità/autenticità
dell'elenco dei nodi, e che l'offuscamento del trasporto sia davvero attivo.

### 1.1 «Wireshark in mezzo»: cosa passa davvero in rete

```sh
bash test/sicurezza/07-traffico.sh                                  # la prova
NYCT_CATTURA=~/nyctalus.pcap bash test/sicurezza/07-traffico.sh     # e tieni la cattura per Wireshark
```

Una rete di prova completa (4 nodi + due apparecchi, Alice e Bob, ognuno con il proprio client)
gira in uno **spazio di rete isolato** (senza permessi di amministratore, la tua rete non si
tocca) dove **dumpcap** — il motore di cattura di Wireshark — registra **ogni pacchetto**.
Alice e Bob si scambiano un **messaggio**, un **file** e una **videochiamata** (voce + immagini);
in ogni contenuto c'è una **parola spia**. Poi la cattura si legge con gli occhi di chi sta
in mezzo alla rete:

| Controllo | Esito |
|---|---|
| parole spia di testo, file, voce, video (in chiaro o in esadecimale) | **mai** nei pacchetti in rete |
| indirizzi `.nyct` di Alice e Bob | **mai** nei pacchetti in rete |
| controllo della ricerca | le parole spia **ci sono** nel traffico locale app ↔ motore (127.0.0.1, non esce dall'apparecchio): la ricerca funziona |
| chi parla con chi | ogni pacchetto va **a o da un nodo**: nessun collegamento diretto fra Alice e Bob |
| fughe | **nessun** pacchetto DNS, DNS su TLS, mDNS, SSDP, STUN/TURN (WebRTC), NTP |
| TCP | solo locale (app ↔ motore); **nessuna** connessione TCP in rete |
| aspetto | entropia 7,999 bit/byte = **rumore casuale**; ogni pacchetto ha la forma **QUIC v1** (come HTTP/3, cifrato TLS 1.3), con il «bit fisso» reso casuale di proposito (RFC 9287) |

**Browser del PC** (`test/sicurezza/08-browser-pc.sh`, stesso spazio isolato con cattura): una
scheda 🦇 **non apre nulla finché la rete non è pronta e il proxy non è applicato** («a prova
di errore»: prima, nei secondi in cui la rete si accendeva, la pagina partiva in chiaro — difetto
trovato da questa prova e corretto); poi tutto passa dal proxy, i nomi dei siti li risolve la
rete (zero pacchetti DNS), il sito non riceve visite dirette.

**HTTP in chiaro?** Solo *dentro* l'apparecchio: la pagina della chat parla con il proprio
motore su 127.0.0.1 (con un segreto d'accesso). In rete viaggia solo QUIC cifrato con TLS 1.3 —
come l'HTTPS — e **dentro** ci sono gli strati a cipolla (uno per nodo) e la cifratura fra i
due contatti (Noise).

**Cosa si capisce comunque (onestamente):** chi osserva la linea di un apparecchio vede
**quando** passa molto traffico: in videochiamata il ritmo è circa **40 volte** quello a
riposo. Non vede *cosa* passa né *con chi*. La maggior parte dei pacchetti ha la stessa
dimensione (~1420 byte, celle a lunghezza fissa). Per nascondere anche il ritmo esiste il
**rumore di fondo** (`--cover`, §2.5), a costo di banda.

### 1.1-ter Sonde, ponti nascosti e scelte di fondo verificate sul traffico vero

```sh
bash test/funzionamento/32-ponte-nascosto.sh          # ponti nascosti e sonde (in locale)
python3 simulazione/verifica_paradigmi.py CARTELLA [--camuffamento] [--rumore 0.2]   # in Shadow
```

**Sonde di un censore.** `test/strumenti/sonda_quic.py` manda a un nodo datagrammi come l'inizio
di un collegamento QUIC e byte a caso. Un nodo **senza** camuffamento risponde (si scopre); un nodo
camuffato e un ponte nascosto **non rispondono mai**. Misura su 20.000 sonde a raffica: motore
0.3.7 camuffato **63 risposte**, motore 0.3.8 **0**.

**Le scelte di fondo, controllate sui pacchetti** (Shadow, cattura su utente, 4 nodi e sito):

| Scelta | Verificato |
|---|---|
| trasporto QUIC su UDP | ✅ solo UDP fra utente e rete, 0 pacchetti TCP |
| entrata unica | ✅ l'utente parla solo con la sua entrata (o il suo ponte) |
| ragnatela (più circuiti) | ✅ i due nodi intermedi portano **metà ciascuno** dello stesso scaricamento |
| cipolla | ✅ il contenuto è in chiaro **solo** fra uscita e sito, mai su entrata, intermedi, linea dell'utente |
| camuffamento | ✅ nessuna intestazione QUIC visibile, primo byte distribuito come rumore |
| senza camuffamento | ⚠️ si riconosce QUIC; il **nome** nel primo pacchetto (`nyctalus.invalid`) è stato **tolto** nella 0.3.8 |
| rumore di fondo | ✅ traffico anche da fermi (1,3 contro 0,9 pacchetti/s a 0,2 celle/s) |
| celle fisse | ✅ dentro il circuito; i datagrammi QUIC sul filo hanno lunghezze diverse |

### 1.1-bis Impronta del browser del PC (schede 🦇)
Le schede Stealth hanno un'**impronta uniforme**: identità Safari/macOS, lingua en-US (anche
nell'intestazione), ora UTC, schermo arrotondato, 2 processori, niente WebGL, Web Audio e
WebRTC, canvas non leggibile, anche nelle cornici. Due PC con fuso e lingua diversi danno la
stessa impronta: lo verifica la prova `30-impronta-browser`. **Limiti:** i Web Worker non vedono
le coperture, i caratteri installati non sono uniformati e il motore WebKitGTK è raro; per i
casi delicati sul web normale c'è Nyctalus Firefox (profilo sul modello di Tor Browser, prova `29`).

### 1.2 Porta di controllo e ⌨ Terminale

- La porta di controllo del motore ascolta solo su `127.0.0.1` e risponde solo a chi presenta il
  **segreto** (confronto a tempo costante). Il segreto sta in `~/.nyctalus-chat/controllo` con
  permessi **600** (solo il tuo utente), e viene cancellato quando il motore si spegne.
  `nyctalus-cli` lo legge da lì.
- Il **⌨ Terminale** dell'app (telefono e Windows) esegue **solo** `nyctalus-cli`: gli argomenti
  sono una lista passata direttamente al programma, **mai a una shell**. `$(…)`, `;` e `|` restano
  testo. Il canale accetta solo la pagina dell'app (cookie del segreto + controllo dell'origine:
  un sito estraneo riceve 403). Al peggio, una pagina ingannata fa quello che fa già la chat:
  niente chiavi, niente resto di Termux.
- Verificato da `03-webapp` (passi del Terminale, compresa l'iniezione) e `24-cli`.

---

## 2. Verifiche manuali su **rete reale** (con un'uscita remota)

Alcune cose **non** si possono provare in locale (dove client, nodi e uscita sono
tutti sul tuo PC: l'uscita vede il tuo IP perché *sei tu* l'uscita). Servono nodi
su macchine diverse.

### 2.1 L'IP è davvero nascosto (niente leak)
Con lo **Stealth attivo** e una rete con **uscita su un'altra macchina**:
```sh
# IP reale (senza proxy)
curl -s https://api.ipify.org ; echo
# IP visto attraverso Nyctalus (deve essere DIVERSO, quello dell'uscita)
curl -s --socks5-hostname 127.0.0.1:1080 https://api.ipify.org ; echo
```
I due valori **devono differire**. Dal **browser**, con Stealth ON, apri una
pagina tipo `https://ip.me` o `https://whoer.net` e controlla che non mostri il
tuo IP/DNS reale.

### 2.2 Niente DNS leak
`--socks5-hostname` (e il browser in `socks5://`) manda il **nome host** al proxy,
che lo risolve **dentro la rete**: il tuo resolver locale non deve vedere i domini
che visiti. Verifica con un monitor DNS locale o osservando che le query non
escono dal tuo PC quando lo Stealth è attivo.

### 2.3 I nodi vedono solo ciò che devono
Su una rete a 3+ salti con nodi istrumentati:
- il **guard** (1º nodo) vede il tuo IP ma **non** la destinazione;
- il **nodo di mezzo** non vede **né** IP **né** destinazione;
- l'**uscita** vede la destinazione ma **non** il tuo IP.

Lo si controlla facendo girare i nodi con i log e verificando che l'indirizzo
del client **non** compaia oltre il guard, e che la destinazione **non** compaia
prima dell'uscita.

### 2.4 Osservare il filo
La versione automatica è la prova §1.1. A mano, con **Wireshark** o `tcpdump` sul collegamento di un nodo:
- il traffico è **QUIC/UDP** (o, con `--obfuscation`, **rumore casuale** senza
  firma di QUIC): un DPI non riconosce "Nyctalus";
- i siti `.nyct` viaggiano **cifrati end-to-end** (Noise): il nodo d'incontro e i
  relè non vedono il contenuto in chiaro.

> Nota: la **lunghezza fissa delle celle** (1372 byte) protegge dall'analisi di
> dimensione *da parte di un nodo che decifra il suo strato*; sul filo i
> datagrammi QUIC hanno una loro struttura. È una difesa a livello di circuito,
> verificata dai test, non osservabile direttamente con tcpdump (QUIC cifra).

### 2.5 Rumore di fondo
Con `--cover 5` su demone e nodi, il flusso di celle resta **uniforme anche da
fermo**: un osservatore non capisce *quando* invii davvero. Lo si vede dal volume
costante di datagrammi anche senza navigare.

### 2.6 Chat: autenticazione del contatto
La chat `.nyct` eredita tutte le proprietà del trasporto (anonimato di rete,
cifratura E2E verso il servizio, uniformità dei quadri). In più, quando qualcuno
ti scrive **non ci si fida dell'indirizzo dichiarato**: il motore esegue, dentro
il tunnel già cifrato, un **secondo handshake Noise NK** verso quell'indirizzo —
solo chi possiede la chiave privata del `.nyct` dichiarato lo conclude, quindi un
impostore che conosce l'indirizzo di un tuo contatto **non** riesce a spacciarsi
per lui. Il test `cargo test` include la prova che un impostore viene respinto; a
livello end-to-end lo si vede con `strumenti/demo-chat.sh` (i messaggi arrivano con
l'indirizzo autenticato del mittente). *Nota:* resta la verifica umana del Contact
ID (scambiatevelo per un canale fidato, come si fa con le onion di Ricochet).

**Allegati.** Viaggiano nella stessa sessione autenticata; il destinatario
verifica l'impronta **BLAKE3** dell'intero file e scarta tutto se non torna (o se
arrivano più byte del dichiarato). I nomi ricevuti sono ripuliti (niente `../`,
niente file nascosti, nessuna sovrascrittura) — c'è un test apposito
(`nomi_di_file_resi_innocui`). La finestra apre direttamente solo i tipi innocui
(immagini, video, audio, PDF, testo); eseguibili e archivi vengono solo mostrati
nella cartella. Le foto JPEG/PNG partono **senza EXIF** (niente GPS). Resta un
rischio generale, comune a tutte le chat: le anteprime usano i decodificatori di
immagini del sistema (gdk-pixbuf), quindi tenete il sistema aggiornato.

**Fotocamera e microfono.** Si accendono solo su un'azione esplicita («Scatta una
foto» / «Registra un video») e si spengono chiudendo quella finestra; foto e
video restano in una cartella locale (`~/.cache/nyctalus-chat/fotocamera/`) e
partono solo dopo la conferma nella finestra di anteprima. Le foto della webcam
non contengono EXIF; quelle scelte dal disco vengono ripulite per impostazione
predefinita.

---

## 3. Cosa serve l'uno (e cosa **non** basta da soli)

> Il piano a tappe (dai nodi tutti su un PC fino alla revisione esterna), con cosa dà e cosa
> non dà ciascuna tappa: [`manuali/PIANO-ANONIMATO.md`](manuali/PIANO-ANONIMATO.md).
> **Scelta dei percorsi (Tappa 2, fatta):** guardia d'entrata casuale e persistente, uscita e
> intermedi a caso, mai due nodi parenti (stessa famiglia o sottorete) nello stesso percorso;
> avviso «NON anonimo» se la regola è impossibile, rifiuto con `NYCTALUS_PARENTELA=rigida`.
> **Modalità riservata (Tappa 2-bis, fatta):** chat tramite casella a ritmo di Poisson con buste
> finte e di misura fissa, niente ricevute né presenza; prova `26-riservata`.

Le verifiche sopra mostrano che le difese **ci sono e funzionano**. Ma
**l'anonimato forte** dipende da cose che richiedono altro:

- **Revisione crittografica indipendente** del protocollo nel suo insieme
  (composizione di Sphinx + strati simmetrici + Noise + QUIC): i mattoni sono
  standard, ma la *combinazione* va esaminata da esperti (sottomissione a
  **PETS/USENIX Security**, analisi formale).
- **Pentest** pubblico e **bug bounty**.
- **Misure su rete reale e larga** (es. simulazione con **Shadow**, confronto con
  Tor) per correlazione dei tempi, dimensione dell'anonymity set, resistenza a un
  avversario che osserva gran parte della rete — contro il quale **nessuna** rete
  veloce può nulla (la protezione, lì, è pari a Tor, non superiore).
- **Diversità e numero dei nodi**: l'anonimato cresce con la rete. Pochi nodi,
  tutti dello stesso operatore o nella stessa rete, **non** danno anonimato reale.
- **Igiene dell'utente**: se ti identifichi (login, dati personali), nessuna rete
  ti protegge.

Finché questi passi non sono fatti, Nyctalus va usato per **provare**
funzionamento, usabilità e stabilità — **non** per proteggere dati reali.

---

## 4. Riferimenti rapidi agli strumenti

| Verifica | Strumento |
|---|---|
| Correttezza/regressioni | `cargo test`, `./strumenti/verifica.sh` |
| IP/DNS leak | `curl --socks5-hostname …`, `https://ip.me`, monitor DNS |
| Traffico sul filo | **Wireshark**, `tcpdump` |
| Nodi raggiungibili | `nyctalus nodes --probe` |
| Metriche del demone | `nyctalus monitor` |
| Confronto con Tor (futuro) | **Shadow** (simulatore di rete) |
| Revisione | crittografi indipendenti, PETS/USENIX, pentest |
