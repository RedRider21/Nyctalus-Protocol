<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol. -->

# Guida al deploy e alla configurazione

Come **installare**, **configurare** e **mettere in piedi una rete** Nyctalus per i
test, e come far collegare i tester. Per il funzionamento del protocollo vedi
[README.md](../README.md); per i livelli e il modello di minaccia [SPECIFICA.md](SPECIFICA.md);
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
  ./strumenti/demo-locale-nav-normal.sh      # browser in navigazione normale
  ./strumenti/demo-chat.sh                    # chat: due finestre in locale
  ```

### Per una prova completa in locale (tutto in uno)
```sh
./strumenti/demo-locale.sh
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

> **Modo semplice (consigliato):** pacchetto `installazioni/Server-Linux` sui server
> (`MANUALE-SERVER.md`) e strumento `nyctalus-rete` sul PC del gestore
> (`MANUALE-GESTORE-RETE.md`): fanno da soli i passi qui sotto, con nodi come servizi
> systemd e un **codice di rete** da dare ai client. Quadro: `SCELTE-OPERATIVE.md`.
> Qui sotto restano i comandi a mano, per chi vuole capire o personalizzare.

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
nyctalus node --listen 0.0.0.0:4601 --data /var/lib/nyctalus/nodo-4601 --announce IP_PUBBLICO:4601
#   directory entry:   IP_PUBBLICO:4601,<x25519>,<impronta>
# --data: identità conservata, la riga resta uguale ai riavvii (senza, cambia ogni volta)
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

Dai ai tester il **codice di rete** (`nyctalus-rete codice --url URL`, `MANUALE-GESTORE-RETE.md`):
- **PC:** `nyctalus-rete importa NYCT1:…` (`MANUALE-PC.md` §3);
- **telefono:** riga **🔗 Rete** dell'app (`MANUALE-TELEFONO.md` §1).

A mano, da terminale (senza grafica):
```sh
curl -s https://tuo-sito/directory.txt > directory.txt
nyctalus start --directory directory.txt --authority <PUBBLICA> --socks 127.0.0.1:1080
```
Senza codice, chat e browser usano una **rete di prova locale** (solo su quel dispositivo).

### I tester che vogliono far crescere la rete
Possono far girare un **nodo** (`MANUALE-SERVER.md`, anche a casa con IP dinamico) e mandarti la
loro riga: tu la aggiungi, firmi e ripubblichi `directory.txt`. Il codice non cambia.

---

## 5. Siti `.nyct` e chat
- **Siti:** `MANUALE-SITI.md` (`nyctalus-sito`, cartella o Apache). A mano:
  `nyctalus site --dir ./mio-sito --identity sito.key --directory directory.txt --authority <PUB>`
  oppure `--backend 127.0.0.1:8080`.
- **Chat:** `MANUALE-CHAT.md`. A mano servono `nyctalus start` (il SOCKS),
  `nyctalus site --backend 127.0.0.1:1082 --identity ~/.nyctalus-chat/identita …` (l'entrata) e
  `nyctalus chat` (il motore, con `--rubrica`/`--casella` se la rete le ha); l'interfaccia è
  `python3 -m nyctalus_chat`. Su un PC: `./strumenti/demo-chat.sh` (due identità, rete locale).
- **Rubrica e casella** della rete: `nyctalus-rete rubrica` (`MANUALE-GESTORE-RETE.md` §6-bis).

---

## 7. Riassunto delle opzioni (cosa scegliere)

| Vuoi… | Fai così |
|---|---|
| Provare tutto su un PC | `./strumenti/demo-locale.sh` |
| Provare la **chat** su un PC | `./strumenti/demo-chat.sh` (due finestre) |
| Usare la **chat** per davvero | `nyctalus-chat` (installata) o `./avvia-chat.sh`, sulla rete del codice (§4) |
| Solo il browser normale | `./strumenti/demo-locale-nav-normal.sh` |
| Una rete condivisa per i tester | `MANUALE-SERVER.md` + `MANUALE-GESTORE-RETE.md` (a mano: §3) + §4 |
| Più anonimato (più lento) | `--hops 4` o `5`, `--circuits 3`+ |
| Nascondere i tempi | `--cover 5` su demone e nodi |
| Eludere un DPI/censura | `--obfuscation <HEX>` uguale ovunque |
| Siti interni senza uscita Internet | usa solo `.nyct`, nessun nodo d'uscita |

Per **verificare** che tutto ciò protegga davvero: **[SICUREZZA.md](SICUREZZA.md)**.
