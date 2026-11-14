<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Tutti i comandi

L'elenco **completo** dei comandi di Nyctalus, diviso per tipologia. È l'unico elenco
completo: gli altri manuali spiegano *come* fare le cose e rimandano qui per le opzioni.
Aggiornato alla 0.3.21; ogni comando stampa anche il suo aiuto (`--help`, o `aiuto` per
`nyctalus-cli`).

| Tipologia | Comandi |
|---|---|
| [1. Programmi con la finestra](#1-programmi-con-la-finestra) | `nyctalus-launcher`, `nyctalus-chat`, `nyctalus-browser`, `nyctalus-file`, `nyctalus-firefox` |
| [2. Chat e file dal terminale](#2-chat-e-file-dal-terminale-nyctalus-cli) | `nyctalus-cli` |
| [3. Rete: collegarsi e gestirla](#3-rete-collegarsi-e-gestirla-nyctalus-rete) | `nyctalus-rete` |
| [4. Siti .nyct](#4-siti-nyct-nyctalus-sito) | `nyctalus-sito` |
| [5. Controlli](#5-controlli-collaudo-e-integrita) | `nyctalus-collaudo`, `nyctalus-integrita` |
| [6. Server e nodi](#6-server-e-nodi) | `installa.sh` del server, `nyctalus-server` |
| [7. Telefono (Termux)](#7-telefono-termux) | script unico, `nyctalus-mobile` |
| [8. Installare e disinstallare](#8-installare-e-disinstallare) | `installa.sh`, `disinstalla.sh` |
| [9. Il motore, per esperti](#9-il-motore-per-esperti-nyctalus) | `nyctalus …` |
| [10. Variabili d'ambiente](#10-variabili-dambiente) | `NYCTALUS_…` |
| [11. Per chi sviluppa](#11-per-chi-sviluppa) | prove, pacchetti, release |
| [12. Corrispondenze con Tor](#12-corrispondenze-con-tor) | `tor`, `nyx`, `torsocks`… |

---

## 1. Programmi con la finestra

Installati sul PC Linux (nel menu delle applicazioni e come comandi); senza installare, dalla
cartella del pacchetto, con gli script `avvia-….sh`.

| Comando | Senza installare | Cosa apre |
|---|---|---|
| `nyctalus-launcher` | `./avvia-launcher.sh` | il **mini menu**: accende la rete, apre gli altri programmi, ⚙ impostazioni |
| `nyctalus-chat` | `./avvia-chat.sh` | la **chat** (accende il motore se è spento) |
| `nyctalus-browser` | `./avvia-browser.sh` | il **browser**: normale o 🦇 attraverso la rete, siti `.nyct` |
| `nyctalus-file` | `./avvia-file.sh` | il programma **File** (file grandi ai contatti) |
| `nyctalus-firefox` | `./avvia-firefox.sh` | **Nyctalus Firefox**: il tuo Firefox con un profilo blindato |
| — | `./avvia-terminale.sh` | ⌨ un terminale pronto per `nyctalus-cli` (dal mini menu) |

Telefono: l'app **Nyctalus**. Windows: `Nyctalus.cmd`.

---

## 2. Chat e file dal terminale (`nyctalus-cli`)

Usa lo stesso motore della chat. Nel ⌨ Terminale del mini menu e del telefono si scrive senza
`nyctalus-cli` davanti. Guida con esempi: `MANUALE-TERMINALE.md`.

| Comando | Cosa fa |
|---|---|
| `io` | il tuo Contact ID |
| `contatti` | i contatti, ● se in linea |
| `aggiungi <indirizzo> <nome>` | aggiunge un contatto |
| `scrivi <contatto> <testo…>` | manda un messaggio |
| `leggi <contatto> [-n N]` | la conversazione (ultimi N, di solito 20) |
| `ascolta` | messaggi, file e proposte in arrivo, in diretta (Ctrl-C esce) |
| `invia <contatto> <file…>` | file grandi (programma File) |
| `file` · `file -s` | i trasferimenti con l'avanzamento · seguiti in diretta |
| `accetta <id>` · `rifiuta <id>` | un file in arrivo che chiede conferma |
| `pausa <id>` · `riprendi <id>` · `annulla <id>` | un trasferimento |
| `togli <id>` · `pulisci` | dall'elenco dei trasferimenti (i file restano) |
| `impostazioni [soglia MB] [canali N] [inchat si\|no] [riservata si\|no]` | soglia di accettazione automatica, collegamenti in parallelo, file anche nella chat, modalità riservata |
| `avvia` | accende il motore senza finestre (server, SSH, Raspberry; Ctrl-C lo spegne) |
| `sblocca` | apre i dati protetti da passphrase (da dare a ogni avvio) |
| `cassaforte` · `cassaforte attiva\|cambia\|togli` | protezione dei dati con passphrase: com'è · accendila, cambia passphrase, toglila |
| `aiuto` | l'elenco dei comandi |

`<contatto>` = nome (anche solo l'inizio, senza maiuscole) o indirizzo `.nyct`; `<id>` = basta l'inizio.

---

## 3. Rete: collegarsi e gestirla (`nyctalus-rete`)

Dati del gestore in `~/.nyctalus-rete` (privata): la chiave segreta dell'autorità non esce mai
da lì. Guida: `MANUALE-GESTORE-RETE.md`.

**Per chi usa una rete (client)**

| Comando | Cosa fa |
|---|---|
| `importa CODICE\|FILE` | collega questo PC alla rete del codice `NYCT1:…` |
| `importa --prova` | torna alla rete di prova locale |
| `reti` · `usa N` · `stato` | le reti salvate (● in uso) · passa alla N · a quale rete sei collegato |
| `aiuta si --indirizzo NOME\|IP [--porta 4800] [--tetto 10]` · `aiuta no` · `aiuta stato` | «🦇 Aiuta la rete»: il PC fa anche da nodo solo intermedio, con un tetto di banda (Mbit/s) |
| `ponte aggiungi RIGA…` · `ponte togli IP:PORTA\|tutti` · `ponte elenco` | ponti nascosti: si entra sempre da lì |

**Rete di casa e con gli amici**

| Comando | Cosa fa |
|---|---|
| `casa [--nodi 4]` | rete di casa tutta su questo PC, con il codice per il telefono (Ctrl-C ferma) |
| `casa --internet NOME` | la stessa, raggiungibile da Internet (NOME = No-IP/DuckDNS o IP pubblico) |
| `ospite aggiungi RIGA… --famiglia NOME [--internet]` | aggiunge il nodo di un amico |
| `ospite togli IP:PORTA [--internet]` · `ospite elenco [--internet]` | toglie / elenca i nodi ospiti |

**Per chi gestisce una rete**

| Comando | Cosa fa |
|---|---|
| `crea [--nome NOME]` | crea l'autorità (le chiavi che firmano l'elenco) |
| `aggiungi RIGA… \| --file F [--famiglia NOME] [--banda MBIT] [--intermedio]` | aggiunge nodi (righe «IP:PORTA,chiave,impronta») |
| `togli IP:PORTA` | toglie un nodo |
| `famiglia IP:PORTA NOME\|-` | mette (o toglie) un nodo in una famiglia: mai due nello stesso percorso |
| `banda IP:PORTA MBIT\|-` | banda del nodo: i più capaci si scelgono più spesso |
| `stabilita [--sonda] [--applica]` | stabilità dei PC che aiutano; `--applica` promuove a guardia i più stabili |
| `guardia IP:PORTA si\|no\|auto` | promozione a guardia decisa a mano |
| `elenco` · `verifica` | rete, nodi e stato dell'elenco · prova a raggiungere ogni nodo |
| `firma` | firma l'elenco dei nodi → `directory.txt` |
| `codice --url URL [--rubrica ADDR --casella ADDR]` | il codice di rete da dare ai client |
| `servi [--porta 18800]` | pubblica `directory.txt` da questo PC (rete locale) |
| `protezione [si\|no] [--rumore HZ] [--forma rumore\|h3\|webrtc [--nome SITO]] [--casa\|--internet]` | camuffamento e rumore di fondo di tutta la rete (finiscono nel codice) |
| `rubrica [--codice NYCT1:…] [--servizio [--togli\|--stampa]]` | rubrica e casella cifrata della rete, sempre accese |

---

## 4. Siti .nyct (`nyctalus-sito`)

Dati in `~/.nyctalus-siti/NOME/` (privata). Guida: `MANUALE-SITI.md`.

| Comando | Cosa fa |
|---|---|
| `nyctalus-sito` | guida passo passo (menu) |
| `crea NOME [--cartella DIR \| --backend 127.0.0.1:PORTA] [--codice NYCT1:…] [--chiave FILE] [--si]` | sito nuovo (pagine fisse o un server web locale) |
| `avvia NOME` | lo mette online finché non premi Ctrl-C |
| `servizio NOME [--togli] [--stampa]` | sempre acceso, come servizio dell'utente |
| `elenco` · `indirizzo NOME` | i siti con indirizzo e stato · solo l'indirizzo `.nyct` |
| `rete NOME [--codice NYCT1:…]` | su quale rete sta |
| `esporta NOME FILE` | copia di sicurezza della chiave (= l'indirizzo del sito) |
| `togli NOME [--si]` | lo cancella (la chiave resta in una copia di sicurezza) |

---

## 5. Controlli: collaudo e integrità

**`nyctalus-collaudo`**: il collaudo della **tua rete vera**, con un rapporto ✓/✗. Guida: `MANUALE-COLLAUDO.md`.

| Opzione | Cosa fa |
|---|---|
| *(nessuna)* | integrità, configurazione, nodi e sonde, tunnel, IP, DNS, velocità, chat e servizi |
| `--guidato` | in più le domande sulle prove a mano (browser, chiamata, file…) |
| `--rapporto FILE` · `--json` | salva il rapporto · gli esiti anche in JSON |
| `--solo-rete` | niente verso Internet (server, rete di casa) |
| `--senza-diretto` · `--senza-velocita` · `--mb N` · `--senza-integrita` | salta il confronto con il tuo IP vero · la prova di velocità (o ne sceglie i MB) · il controllo d'integrità |
| `--contatto NOME` | manda anche un messaggio di prova |
| `--socks 127.0.0.1:PORTA` · `--config FILE` | un tunnel o una configurazione precisi |

**`nyctalus-integrita`**: i programmi installati sono quelli pubblicati? (`firma/LEGGIMI.md`)

| Comando | Cosa fa |
|---|---|
| `nyctalus-integrita` | controlla l'installazione di questo dispositivo |
| `nyctalus-integrita verifica CARTELLA` | controlla una cartella (installata o un pacchetto scompattato) |
| `nyctalus-integrita firma CARTELLA` | (chi pubblica) crea e firma l'elenco `INTEGRITA` |

Esce con 0 = tutto a posto, 1 = qualcosa non torna, 3 = controllo non possibile.

---

## 6. Server e nodi

**Installare i nodi** (`installa.sh` del pacchetto server). Guida: `MANUALE-SERVER.md`.

| Opzione | Cosa fa |
|---|---|
| `sudo ./installa.sh` | chiede IP pubblico, quanti nodi, porte |
| `--ip IP` · `--nodi N` (1-8) · `--porta PRIMA` · `--si` | senza domande |
| `--utente` | senza root: servizi dell'utente |
| `--codice NYCT1:…` | prende dal codice della rete camuffamento, rumore e forma |
| `--camuffamento HEX` · `--rumore HZ` · `--http3 NOME` · `--webrtc` | camuffamento e forma a mano |
| `--ponte` | ponte nascosto: fuori dall'elenco, stampa la riga segreta |
| `--nuova-identita` | rifà chiavi e certificato (cambiano le righe: rimandale al gestore) |

**Gestire i nodi installati** (`nyctalus-server`)

| Comando | Cosa fa |
|---|---|
| `stato` | servizi attivi e porte |
| `righe` | le righe d'elenco da mandare al gestore (o quelle segrete del ponte) |
| `avvia` · `ferma` · `riavvia` | i servizi dei nodi |
| `log [PORTA]` | ultimi messaggi dei nodi |

Disinstallare: `sudo /opt/nyctalus/disinstalla.sh [--anche-identita]` (le identità restano, se non lo chiedi).
Sul server si possono usare anche `nyctalus-rete rubrica …` (rubrica e casella) e `nyctalus-sito …`.

---

## 7. Telefono (Termux)

Di solito fa tutto l'app (**Strumenti ▾**). A mano:

| Comando | Cosa fa |
|---|---|
| `curl -fsSL https://github.com/RedRider21/Nyctalus-Protocol/releases/latest/download/installa-termux.sh \| bash` | installa o aggiorna, verificando firma e integrità |
| `nyctalus-mobile` · `--sfondo` · `--ferma` · `--stato` | il servizio del telefono: in primo piano · in sottofondo · fermalo · è acceso? |
| `bash installa.sh [--avvio-automatico]` | dal pacchetto scompattato (con Termux:Boot riparte all'accensione) |
| `bash ~/nyctalus/disinstalla.sh` | toglie solo Nyctalus (i dati solo se confermi) |

In Termux ci sono anche `nyctalus-cli`, `nyctalus-collaudo` e `nyctalus-integrita`.

---

## 8. Installare e disinstallare

| Dove | Comando |
|---|---|
| PC Linux | `./installa.sh [--ruolo client\|gestore\|nodo] [--si] [--avvio-automatico]` (controlla l'integrità del pacchetto prima di copiare) |
| PC Linux | `~/.local/share/nyctalus/disinstalla.sh [--si]` |
| Server | sezione 6 · Telefono: sezione 7 · Windows: si scompatta e si apre `Nyctalus.cmd` |

Passo passo: `MANUALE-INSTALLAZIONE.md`.

---

## 9. Il motore, per esperti (`nyctalus`)

I programmi sopra usano il motore da soli: questi comandi servono per usi a mano, server
particolari e prove. `nyctalus help` stampa l'elenco.

**Usare la rete**

| Comando | Cosa fa |
|---|---|
| `start [--config F] [--directory F --authority HEX] [--socks 127.0.0.1:1080] [--circuits 2] [--hops 3] [--cover 0] [--bridge RIGA]` | demone con proxy SOCKS5 (come `tor`) |
| `status [--config F]` | lo stato del demone |
| `monitor [--control 127.0.0.1:1090]` | cruscotto testuale (come `nyx`) |
| `exec [--socks ADDR] [--config F] -- PROGRAMMA [ARG…]` | un programma dentro la rete (come `torsocks`) |
| `nodes [--directory F --authority HEX] [--probe] [--timeout 3] [--obfuscation HEX]` | l'elenco firmato; `--probe` prova chi risponde |
| `circuit (--directory F --authority HEX \| --nodes 'addr,pub;…') [--socks …] [--circuits 1-8] [--hops 1-5] [--cover 0] [--exits 1-4] [--constant-rate N [--constant-always]]` | come `start` con i nodi indicati |
| `chat [--data DIR] [--identity F] [--backend 127.0.0.1:1082] [--control 127.0.0.1:1091] [--socks 127.0.0.1:1080] [--max-file 200] [--control-token SEGRETO] [--rubrica ADDR] [--casella ADDR]` | il motore della chat |

**Siti .nyct**

| Comando | Cosa fa |
|---|---|
| `site (--dir CARTELLA \| --backend IP:PORTA) [--directory F --authority HEX \| --rendezvous IP:PORTA --fingerprint HEX] [--identity F \| --identity-chat 127.0.0.1:1091] [--hops 3] [--pool 4]` | pubblica un sito (senza `--identity` la chiave è nuova a ogni avvio, e così l'indirizzo) |
| `browse ADDR.nyct [--directory F --authority HEX \| --rendezvous … --fingerprint …] [--path /] [--hops 3]` | visita un sito (stampa la risposta HTTP) |
| `address --identity F` | la chiave di un sito (creata se manca) e il suo indirizzo |

**Nodi ed elenco firmato**

| Comando | Cosa fa |
|---|---|
| `node --listen 0.0.0.0:4601 [--data DIR] [--announce IP:PORTA] [--secret HEX] [--directory F --authority HEX] [--obfuscation HEX] [--tor-socks 127.0.0.1:9050] [--cover 0] [--hidden-bridge] [--middle-only] [--max-rate MBIT]` | un nodo della rete |
| `authority` | crea le chiavi di un'autorità |
| `directory --authority-secret HEX --entries 'addr,x,fp;…'` | firma un elenco di nodi |

**Prove e studio**

| Comando | Cosa fa |
|---|---|
| `test-circuit (--directory F --authority HEX \| --nodes …) [--message DEST]` | prova un circuito |
| `simulate-paths (--directory F --authority HEX) --adversary FAMIGLIA,… [--users 1000] [--days 90] [--circuits-per-day 20]` | quanto spesso un avversario (alcune famiglie) sta ai due capi, con la scelta dei percorsi vecchia, senza guardia e attuale |
| `receive --out F [--listen 0.0.0.0:4433] [--identity F]` · `send --to IP:PORTA --fingerprint HEX --recipient ADDR [--lanes 4] FILE` | trasferimento diretto (livello L2) |
| `single-hop --to IP:PORTA --fingerprint HEX [--socks …]` · `exit --listen 0.0.0.0:4600` | proxy a un salto (prova, nessun anonimato) |

**Opzioni comuni** (valgono dove hanno senso)

| Opzione | Cosa fa |
|---|---|
| `--obfuscation HEX` | camuffamento con la chiave della rete |
| `--mimic [--mimic-name SITO]` | forma HTTP/3 (sembra un sito) |
| `--webrtc` | forma videochiamata (STUN + SRTP) |
| `--bridge RIGA[;RIGA]` | entrare sempre da questi ponti nascosti |
| `--exits N` | uscite diverse (1-4): ogni sito resta sulla sua; non accelera |
| `--constant-rate N [--constant-always]` | copertura costante: N celle/s per verso durante l'uso (o sempre) |

**File di configurazione** di `start`/`status` (`~/.config/nyctalus/config`, righe `chiave = valore`):
`directory`, `authority`, `socks`, `control`, `circuits`, `hops`, `cover`, `obfuscation`, `bridge`, `exits`.

---

## 10. Variabili d'ambiente

Di solito le imposta da solo chi avvia il motore (impostazioni a schede). Utili a mano:

**Motore e rete**

| Variabile | Cosa fa | Di serie |
|---|---|---|
| `NYCTALUS_TCP=1` / `=0` | entrata sempre in TCP / mai | TCP solo se UDP non risponde |
| `NYCTALUS_PONTI` | ponti nascosti (come `--bridge`) | nessuno |
| `NYCTALUS_MIMETICO=1` · `NYCTALUS_NOME_MIMETICO` | forma HTTP/3 e nome del sito imitato | spenta |
| `NYCTALUS_WEBRTC=1` | forma videochiamata | spenta |
| `NYCTALUS_USCITE` | uscite diverse (1-4) | 1 |
| `NYCTALUS_COSTANTE=N` · `NYCTALUS_COSTANTE_SEMPRE=1` | copertura costante | spenta |
| `NYCTALUS_FINESTRA_KB` | finestra per circuito (KB) | 256 |
| `NYCTALUS_CONGESTIONE` | `bbr`, `cubic`, `newreno` | bbr |
| `NYCTALUS_RIUSO=0` | un collegamento per circuito invece che per coppia di nodi | riuso acceso |
| `NYCTALUS_CODIFICA=k,r` | ragnatela codificata (sperimentale, non accelera) | spenta |
| `NYCTALUS_PARENTELA=rigida` | rifiuta i percorsi con nodi parenti invece di avvisare | avviso |
| `NYCTALUS_GUARDIE` | cartella delle guardie primarie | `~/.nyctalus/guardie` |
| `NYCTALUS_DIAGNOSI=1` | righe di diagnosi su stderr | spenta |
| `NYCTALUS_REGISTRA_COMPATIBILE=1` | il nodo accetta anche servizi senza registrazione provata (passaggio dalla 0.3.18) | spento |

**Chat**

| Variabile | Cosa fa | Di serie |
|---|---|---|
| `NYCTALUS_CHAT_PORT` · `NYCTALUS_CHAT_TOKEN` · `NYCTALUS_CHAT_HOST` | porta, segreto e host della porta di controllo (altrimenti da `~/.nyctalus-chat/controllo`) | 1091 |
| `NYCTALUS_MIX_MS` | ritardo medio per salto delle buste riservate (chat da mixnet) | 800 |
| `NYCTALUS_RITMO_RISERVATO` · `NYCTALUS_RITIRO_RISERVATO` | ritmo d'invio e di ritiro della modalità riservata (secondi) | — |
| `NYCTALUS_CAMERA_DEV` · `NYCTALUS_CAMERA_SRC` · `NYCTALUS_MIC_SRC` · `NYCTALUS_ALTOPARLANTE` | webcam, microfono e altoparlante (GStreamer) | `/dev/video0`, quelli di sistema |
| `NYCTALUS_MANUALE_URL` | dove apre il manuale il pulsante «?» | il sito |

**Programmi, cartelle e porte**

| Variabile | Cosa fa | Di serie |
|---|---|---|
| `NYCTALUS_BIN` | quale motore usare | quello installato |
| `NYCTALUS_RETE_DATI` · `NYCTALUS_SITI_DATI` | dati di `nyctalus-rete` · di `nyctalus-sito` | `~/.nyctalus-rete` · `~/.nyctalus-siti` |
| `NYCTALUS_RUBRICA_PORTE` | porte locali di rubrica e casella | 18810-18814 |
| `NYCTALUS_STAB_GIORNI` · `NYCTALUS_STAB_PROVE` · `NYCTALUS_STAB_OGNI` | misura di stabilità per le guardie promosse | 14 giorni, 20, 3600 s |
| `NYCTALUS_FIREFOX` · `NYCTALUS_FIREFOX_PROFILO` | quale Firefox · dove sta il profilo | quello di sistema · `~/.nyctalus-firefox/profilo` |
| `NYCTALUS_MOBILE_PORT` · `NYCTALUS_GALLERIA` · `NYCTALUS_AVVIO_AUTOMATICO=0` | (telefono/Windows) porta della web-app · cartelle della galleria · non accendere la rete all'avvio | — |

Le variabili usate solo dalle prove automatiche (`…_SOLO`, `NYCTALUS_SENZA_SUONI`,
`NYCTALUS_RADICE`, `NYCTALUS_FIRMA_*`, `NYCTALUS_CHIAVE_RELEASE`…) sono descritte in `MANUALE-PROVE.md`.

---

## 11. Per chi sviluppa

Dalla cartella dei sorgenti.

| Comando | Cosa fa |
|---|---|
| `cargo build --release` · `cargo test --release` | compila il motore · test del motore |
| `test/esegui-tutti.sh [sicurezza\|funzionamento]` | tutte le prove (`NYCT_TIENI=1` conserva i log) |
| `strumenti/verifica.sh` | solo le prove di sicurezza |
| `strumenti/demo-locale.sh` · `strumenti/demo-chat.sh` · `strumenti/demo-locale-nav-normal.sh` | rete di prova con browser e sito · due chat · browser normale |
| `strumenti/audit.sh` | vulnerabilità note delle dipendenze (RustSec) |
| `strumenti/crea-installazioni.sh` · `strumenti/pacchetto-termux.sh DIR` · `strumenti/compila-apk.sh` | pacchetti (firmati per l'integrità) · pacchetto Termux · app Android |
| `strumenti/firma-release.sh DIR` · `strumenti/verifica-release.sh DIR` | firma / verifica `SHA256SUMS` di una release |
| `strumenti/manuali-html.py CARTELLA` · `strumenti/crea-screenshot.sh` | manuali per il sito · screenshot su dati dimostrativi |
| `simulazione/velocita.py`, `simulazione/correlazione.py`, … | misure in Shadow (`simulazione/LEGGIMI.md`) |

---

## 12. Corrispondenze con Tor

| Tor | Nyctalus | A cosa serve |
|---|---|---|
| `tor` | `nyctalus start` | demone + proxy SOCKS5 |
| porta di controllo | `nyctalus status` | interrogare il demone |
| `nyx` (monitor del Tor Project) | `nyctalus monitor` | cruscotto testuale |
| `torsocks` | `nyctalus exec` | un programma dentro la rete |
| servizio `.onion` (`HiddenServiceDir`) | `nyctalus-sito` (o `nyctalus site`) | pubblicare un sito nascosto |
| `obfs4proxy`, ponti | `--obfuscation`, `--mimic`, `--webrtc`, `nyctalus-rete ponte` | camuffamento e ponti |
| Tor Browser | Nyctalus Browser 🦇, `nyctalus-firefox` | navigare |
| — | `nyctalus-cli`, `nyctalus-chat`, `nyctalus-file` | chat, chiamate e file (Tor non li ha) |
| — | `nyctalus-rete` | creare e gestire una rete propria |
| — | `nyctalus-collaudo`, `nyctalus-integrita` | collaudo della rete, integrità dei programmi |
