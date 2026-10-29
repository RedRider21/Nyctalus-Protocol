<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol. -->

# Guida al deploy e alla configurazione

Come **installare**, **configurare** e **mettere in piedi una rete** Nyctalus per i
test, e come far collegare i tester. Per il funzionamento del protocollo vedi
[README.md](README.md); per i livelli e il modello di minaccia [SPECIFICA.md](SPECIFICA.md);
per **verificare** sicurezza e anonimato [SICUREZZA.md](SICUREZZA.md).

> ⚠️ **Prototipo non ancora verificato da revisori indipendenti.** Usalo per
> provare funzionamento, usabilità e stabilità — **non** per proteggere dati
> reali. Vedi [SICUREZZA.md](SICUREZZA.md).

---

## 1. Cosa installare

### Per compilare il motore (demone, nodi)
- **Rust** (https://rustup.rs, profilo *minimal* sufficiente).
- Build:
  ```sh
  git clone https://github.com/RedRider21/Nyctalus-Protocol.git
  cd Nyctalus-Protocol
  cargo build --release        # produce target/release/nyctalus
  ```
- In alternativa, quando ci sarà una Release pubblica, si scaricano i binari
  già pronti (Linux x86_64/arm64, Windows, macOS).

### Per i client grafici (browser, launcher, chat)
- **Python 3**, **PyGObject (GTK 3)**; **WebKit2GTK** solo per il browser.
  ```sh
  sudo apt install python3-gi gir1.2-gtk-3.0 gir1.2-webkit2-4.1
  # su sistemi più vecchi: gir1.2-webkit2-4.0
  # opzionale, per i tasti rapidi GLOBALI del launcher:
  sudo apt install gir1.2-keybinder-3.0
  # opzionale, per la chat: fotocamera (foto/video), lettore audio, miniature video
  sudo apt install gir1.2-gstreamer-1.0 gstreamer1.0-gtk3 gstreamer1.0-plugins-good \
                   gstreamer1.0-plugins-ugly gstreamer1.0-libav ffmpeg
  ```
  Il **launcher** e la **chat** richiedono solo GTK 3 (non WebKit).
- Avvio:
  ```sh
  ./scripts/demo-locale-nav-normal.sh      # browser in navigazione normale
  ./scripts/demo-chat.sh                    # chat: due finestre in locale
  ```

### Per una prova completa in locale (tutto in uno)
```sh
./scripts/demo-locale.sh
```
Avvia 4 nodi locali, pubblica un sito `.nyct` dimostrativo, avvia il demone e
apre il browser. Serve a **provare subito** il software su una sola macchina —
non è una rete reale condivisa (vedi §3).

---

## 2. I componenti e i comandi

Il binario `nyctalus` fa tutto, con sotto-comandi (`nyctalus help` per l'elenco):

| Comando | A cosa serve |
|---|---|
| `nyctalus authority` | crea una **autorità** di directory (una chiave segreta + pubblica). Si fa **una volta sola**. |
| `nyctalus node --listen IP:PORTA` | avvia un **nodo-relè**. Stampa la sua *riga d'elenco* (indirizzo, chiave, impronta). |
| `nyctalus directory --authority-secret SEG --entries '...'` | **firma** l'elenco dei nodi con l'autorità → `directory.txt`. |
| `nyctalus nodes --directory FILE --authority PUB [--probe]` | mostra i nodi e, con `--probe`, quali sono raggiungibili ora. |
| `nyctalus start` / `nyctalus circuit` | il **demone**: proxy SOCKS5 che instrada attraverso la rete. |
| `nyctalus site --dir ./sito` / `--backend IP:PORTA` | pubblica un **sito `.nyct`** (cartella o web server esterno). |
| `nyctalus browse ADDR.nyct` | visita un sito `.nyct` da terminale. |
| `nyctalus chat` | **motore della chat `.nyct`** (senza server, senza metadati): parla con l'interfaccia grafica su una porta di controllo locale. |
| `nyctalus exec -- PROGRAMMA` | lancia un programma instradandolo nella rete (stile torsocks). |
| `nyctalus monitor` | cruscotto testuale delle metriche del demone. |

### Opzioni principali (chi le usa)
- `--hops N` (demone): nodi per percorso. **Min 1, max 5, default 3.** Sotto 3 niente vero anonimato.
- `--circuits N` (demone): strade parallele. **Min 1, max 8, default 2.**
- `--cover HZ` (demone **e** nodo): **rumore di fondo** (celle finte a ritmo di Poisson). `circuit/start --cover` = in salita, `node --cover` = in discesa. Default 0.
- `--obfuscation HEX` (demone **e** nodi): **camuffamento del trasporto** con una chiave di rete condivisa (ogni datagramma UDP reso rumore per il DPI). Deve essere **uguale su tutti** i nodi e sul client.
- `--socks IP:PORTA` (demone): dove ascolta il proxy (default `127.0.0.1:1080`).
- `--control IP:PORTA` (demone): porta di controllo per `monitor` (default `127.0.0.1:1090`).
- `--tor-socks IP:PORTA` (nodo): proxy del Tor di sistema per raggiungere i `.onion` (default `127.0.0.1:9050`).
- `--max-file MB` (chat): dimensione massima degli allegati inviati e accettati. **Min 1, max 4096, default 200.**

---

## 3. Mettere in piedi una rete per i test (condivisa, reale)

Perché **più persone** provino la **stessa** rete, servono nodi su **server
pubblici** (con IP pubblico) e una directory firmata **pubblicata**.

### 3.1 Una volta sola: l'autorità
Su una macchina sicura (la tieni tu):
```sh
nyctalus authority
#   secret:  <SEGRETO>      <- tienilo segreto, serve a firmare la directory
#   public:  <PUBBLICA>     <- va dato a tutti i client e nodi
```

### 3.2 I nodi (su ogni server pubblico)
Su ciascun VPS/server, con la porta aperta sul firewall:
```sh
nyctalus node --listen 0.0.0.0:4601
#   directory entry:   IP_PUBBLICO:4601,<x25519>,<impronta>
```
Copia la **riga `directory entry`** di ogni nodo.
(Opzionale: aggiungi `--obfuscation <HEX>` uguale su tutti per il camuffamento,
e `--cover 5` per il rumore di fondo in discesa.)

> Consiglio: all'inizio fai nodi **senza uscita verso Internet** e punta sui
> siti `.nyct` interni (niente implicazioni legali da nodo d'uscita). L'uscita
> verso Internet la abiliti più avanti, con volontari consapevoli.

### 3.3 Firma la directory
Sulla macchina dell'autorità, con tutte le righe raccolte:
```sh
nyctalus directory --authority-secret <SEGRETO> \
  --entries 'IP1:4601,x,fp;IP2:4601,x,fp;IP3:4601,x,fp' > directory.txt
```

### 3.4 Pubblica la directory a un URL
Metti `directory.txt` dove i client possono scaricarlo via HTTPS: per esempio un
hosting statico, le **GitHub Pages** del progetto, o un tuo server. L'URL sarà
qualcosa come `https://tuo-sito/directory.txt`.
Quando aggiungi/togli nodi, ri-firmi e ri-pubblichi il file.

### 3.5 Verifica
```sh
nyctalus nodes --directory directory.txt --authority <PUBBLICA> --probe
# deve elencare i nodi come 'online' con il tempo di risposta
```

---

## 4. Configurare i client (i tester)

I tester installano il **browser** e lo puntano alla tua rete. Basta un file di
configurazione `~/.nyctalus-browser/config.json`:
```json
{
  "directory_url": "https://tuo-sito/directory.txt",
  "authority": "<PUBBLICA>",
  "socks_host": "127.0.0.1",
  "socks_port": 1080
}
```
Poi: avviano il browser (`demo-locale-nav-normal.sh` o `python3 -m nyctalus_browser`),
cliccano **Stealth** → il browser **scarica la directory, la fa verificare al
demone** (che controlla la firma con la chiave dell'autorità) e instrada il
traffico. Il pulsante mostra **"Stealth ON"**.

- **Senza** `directory_url`/`authority` nel config, cliccando Stealth il browser
  avvia una **rete di prova locale** (sul PC del tester) → pulsante "Stealth ON ·
  prova locale": utile a provare, ma non è la rete condivisa.
- Chi preferisce il terminale può lanciare il demone a mano:
  ```sh
  curl -s https://tuo-sito/directory.txt > directory.txt
  nyctalus start --directory directory.txt --authority <PUBBLICA> --socks 127.0.0.1:1080
  ```

### I tester che vogliono far crescere la rete
Possono far girare un **nodo** (serve una porta pubblica) e mandarti la loro
riga `directory entry`: tu la aggiungi, ri-firmi e ri-pubblichi la directory.
(La registrazione automatica dei nodi è un miglioramento futuro.)

---

## 5. Pubblicare un sito `.nyct`

Su qualunque macchina connessa alla rete (serve `directory.txt` + autorità):
```sh
# una cartella di file HTML/JS…
nyctalus site --dir ./mio-sito --identity sito.key --directory directory.txt --authority <PUB>
#   address:  <ADDR>.nyct     <- l'indirizzo da condividere

# …oppure un web server esistente (Nginx/Apache) dietro al demone
nyctalus site --backend 127.0.0.1:8080 --directory directory.txt --authority <PUB>
```
I visitatori aprono `http://<ADDR>.nyct` dal browser (con lo Stealth attivo) o con
`nyctalus browse <ADDR>.nyct`.

---

## 6. La chat `.nyct` (senza server, senza metadati)

Ogni utente **è** un indirizzo `.nyct`: quello è il suo **Contact ID**, l'unica cosa
da scambiarsi. Non esiste un server centrale né una rubrica globale: i messaggi
viaggiano punto-a-punto nella rete (circuito → punto d'incontro → servizio),
cifrati end-to-end. Il mittente è **autenticato crittograficamente** (un secondo
handshake Noise NK dentro il tunnel: solo chi possiede la chiave privata
dell'indirizzo che dichiara può concludere la conversazione), quindi nessuno può
spacciarsi per un tuo contatto.

**Serve la rete condivisa.** La chat è a due parti: per parlarsi, **entrambi**
devono essere sulla **stessa rete** (quella pubblica di §3, configurata nei client
come in §4 — `directory_url` + `authority`). Senza configurazione la finestra si
apre comunque, ma su una rete di prova **locale** (non raggiungi altre persone).

**Avvio (uso reale):**
```sh
./avvia-chat.sh          # nel pacchetto; oppure scripts/avvia-chat.sh in sviluppo
```
Fa tutto da sé: avvia/riusa il proxy SOCKS, pubblica il tuo servizio `.nyct`, avvia
il motore e apre la finestra. Al primo avvio crea la tua identità in
`~/.nyctalus-chat/identita` (il tuo Contact ID non cambia più). Nella finestra:
copia il tuo Contact ID e dallo agli amici; premi « + » per aggiungere il loro
indirizzo; scrivi.

**Provarla da solo su un PC** (due identità, due finestre, rete locale):
```sh
./scripts/demo-chat.sh
```

**A mano** (se vuoi controllare i pezzi): servono `nyctalus start` (il SOCKS),
`nyctalus site --backend 127.0.0.1:1082 --identity ~/.nyctalus-chat/identita …`
(l'entrata) e `nyctalus chat` (il motore); l'interfaccia è
`python3 -m nyctalus_chat` con `PYTHONPATH` su `chat/` e `browser/`.

---

## 7. Riassunto delle opzioni (cosa scegliere)

| Vuoi… | Fai così |
|---|---|
| Provare tutto su un PC | `./scripts/demo-locale.sh` |
| Provare la **chat** su un PC | `./scripts/demo-chat.sh` (due finestre) |
| Usare la **chat** per davvero | `./avvia-chat.sh` (serve la rete condivisa, §3–4) |
| Solo il browser normale | `./scripts/demo-locale-nav-normal.sh` |
| Una rete condivisa per i tester | §3 (nodi pubblici + directory pubblicata) + §4 (config dei client) |
| Più anonimato (più lento) | `--hops 4` o `5`, `--circuits 3`+ |
| Nascondere i tempi | `--cover 5` su demone e nodi |
| Eludere un DPI/censura | `--obfuscation <HEX>` uguale ovunque |
| Siti interni senza uscita Internet | usa solo `.nyct`, nessun nodo d'uscita |

Per **verificare** che tutto ciò protegga davvero: **[SICUREZZA.md](SICUREZZA.md)**.
