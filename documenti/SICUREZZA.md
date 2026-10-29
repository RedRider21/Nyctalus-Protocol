<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol. -->

# Verificare sicurezza e anonimato

Questo documento spiega **cosa si può verificare da soli** (con gli strumenti
qui indicati e lo script [`strumenti/verifica.sh`](strumenti/verifica.sh)) e **cosa
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
Con **Wireshark** o `tcpdump` sul collegamento di un nodo:
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
