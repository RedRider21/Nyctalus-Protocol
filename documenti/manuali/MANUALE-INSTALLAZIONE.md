<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Manuale d'installazione

> ⚠️ **Prototipo** non ancora verificato da revisori indipendenti: usalo per provare
> funzionamento, usabilità e stabilità, **non** per proteggere dati reali.

Tutto ciò che si installa sta nella cartella **`installazioni/`**, una sottocartella per
dispositivo (la crea `strumenti/crea-installazioni.sh` partendo dai sorgenti):

| Dispositivo | Cartella | Cosa contiene |
|---|---|---|
| PC Linux (x86_64) | `installazioni/PC-Linux/` | `Nyctalus-PC-Linux.zip`: motore, browser, chat, launcher, `nyctalus-rete`; l'installatore chiede il **ruolo** |
| Server / VPS (nodi) | `installazioni/Server-Linux/` | `Nyctalus-Server-Linux.zip`: solo il motore, per i nodi della rete (`MANUALE-SERVER.md`) |
| Telefono Android | `installazioni/Android/` | `nyctalus-termux.tar.gz` (dentro Termux) · `Nyctalus.apk` (l'app, facoltativa) |
| Manuali | `installazioni/Manuali/` | questi manuali |

Prima di iniziare: **cosa installare e dove** dipende da cosa vuoi fare. Lo spiega
`LEGGIMI.md` (Inizia qui: client, nodo server, gestore della rete).

---

## 1. PC Linux

### 1.1 Cosa installare dal sistema
Debian, Ubuntu, Linux Mint e derivate:
```sh
sudo apt install python3-gi gir1.2-gtk-3.0 gir1.2-webkit2-4.1
```
Facoltativi:
```sh
sudo apt install gir1.2-keybinder-3.0                  # tasti rapidi GLOBALI del launcher
sudo apt install gir1.2-gstreamer-1.0 gstreamer1.0-gtk3 \
     gstreamer1.0-plugins-good gstreamer1.0-plugins-ugly \
     gstreamer1.0-libav ffmpeg                          # fotocamera, audio, miniature video
```
Sui sistemi più vecchi `gir1.2-webkit2-4.0` al posto di `-4.1`.

### 1.2 Installare
Nessun installatore: si estrae e si usa.
```sh
unzip Nyctalus-PC-Linux.zip             # crea la cartella Nyctalus/
cd Nyctalus
```

All'inizio l'installatore chiede **che cosa vuoi fare con questo computer**:
**1) Usare Nyctalus** (il caso normale: ti colleghi alla rete di qualcuno), **2) Ospitare una
rete** (come 1, e in più questo PC può essere la tua rete), **3) Server / nodo** (una macchina
sempre accesa che fa da nodo: vedi `MANUALE-SERVER.md`). Senza domande:
`./installa.sh --ruolo client --si`.

Al **primo avvio** il mini menu ripropone la scelta in modo semplice (**🧭 Che cosa vuoi fare?**:
mi collego alla rete di qualcuno · ospito la mia rete · aiuto una rete · provo da solo) e apre
la finestra giusta (`MANUALE-PC.md`).

**Aggiornare (dalla 0.3.25):** mini menu → **⚙ → ⬆ Aggiorna Nyctalus…**, oppure
`nyctalus-aggiorna`: scarica l'ultima release, verifica la firma e l'impronta del pacchetto e
la installa al posto della vecchia (i tuoi dati restano). Fino alla 0.3.24: scarica il pacchetto
nuovo e rilancia `./installa.sh`.

Prima di copiare qualunque cosa l'installatore controlla l'**integrità** del pacchetto: ogni file
deve avere l'impronta scritta nell'elenco firmato dalla release (`firma/LEGGIMI.md`). Se un file è
stato cambiato o aggiunto si ferma con «controllo d'integrità fallito» e non installa nulla:
riscarica il pacchetto dalla release ufficiale. Dopo, in ogni momento: `nyctalus-integrita`.

### 1.3 Avviare
| Comando | Cosa fa |
|---|---|
| `./avvia-launcher.sh` | il pannello da cui parte tutto (logo → guida, Browser, Chat) |
| `./avvia-browser.sh` | solo il browser |
| `./avvia-chat.sh` | solo la chat (motore + servizio `.nyct` + finestra) |

Uso dei programmi: **`MANUALE-PC.md`** e **`MANUALE-CHAT.md`**.

### 1.4 Disinstallare
Cancella la cartella `Nyctalus/`. I dati personali stanno nella home, in cartelle a parte:
`~/.nyctalus-chat` (identità, contatti, messaggi, allegati), `~/.nyctalus-browser`,
`~/.nyctalus-launcher`, `~/.cache/nyctalus-chat`. Cancellale solo se vuoi perdere
identità e chat.

---

## 2. Telefono Android

**Dalla 0.4.0 basta l'app.** `Nyctalus.apk` contiene già tutto: il motore (lo stesso del PC),
la chat in stile Telegram, chiamate, file, browser e rubrica. **Termux non serve**: è un'aggiunta
facoltativa per chi vuole usare Nyctalus anche scrivendo comandi (§2.3).

### 2.0 Installare (dalla 0.4.0)
1. Installa **`Nyctalus.apk`** (dalla release, o da `Nyctalus-Android.zip`) e aprila. Android
   chiede di consentire «Installa app sconosciute» per il browser o per i file.
2. Al primo avvio chiede, se vuoi, il permesso di mostrare **foto e video** recenti negli allegati
   (puoi dire di no: si sceglie dal selettore di Android).
3. **Se sul telefono c'era Nyctalus in Termux** (versioni fino alla 0.3.x), l'app propone
   **«Porto qui i tuoi dati?»**: tocca **Porta i dati**. Termux si apre un momento, impacchetta
   identità, contatti, messaggi, file ricevuti e la tua rete e li consegna all'app, sul telefono
   stesso (niente esce dal telefono). Il **Contact ID resta lo stesso**: gli amici non devono rifare
   nulla. I dati restano anche in Termux finché non li cancelli tu.
4. Collegati a una rete: `MANUALE-TELEFONO.md` §1.

Tutto sta nella cartella **privata** dell'app; una notifica fissa «Nyctalus attivo» tiene accesa la
rete anche a schermo spento (le notifiche si spengono in Impostazioni → 📱 App). L'app pesa circa 6 MB
e porta il motore per i telefoni ARM a 64 e a 32 bit.

**Per aggiornare:** **Strumenti ▾ → ☁ Aggiornamenti da GitHub**, come prima.

### 2.1 Fino alla 0.3.x: con l'app e Termux
1. Installa **`Nyctalus.apk`** (dal pacchetto `Nyctalus-Android.zip` della release) e aprila.
   Android chiede di consentire «Installa app sconosciute» per il browser o per i file.
2. Tocca **⚡ Prepara Nyctalus**: l'app fa tutto da sola, un passo alla volta, e ti chiede solo:
   - **«Installa»** su **Termux**, che l'app scarica lei stessa dalla release ufficiale (GitHub),
     nella versione giusta per il tuo dispositivo (la prima volta Android chiede anche di
     consentire le installazioni da Nyctalus: attiva e torna indietro);
   - **«Consenti»** al permesso «Esegui comandi in Termux»;
   - **una volta sola**, se Termux è appena installato: si apre Termux, aspetti che si prepari
     (compare `$`), **tieni premuto → Incolla → Invio**. Il comando è **già copiato**: non scrivi
     niente. È sempre lo stesso, una riga sola:
     ```
     curl -fsSL https://github.com/RedRider21/Nyctalus-Protocol/releases/latest/download/installa-termux.sh | bash
     ```
     Lo script scaricato fa tutto da solo: permesso all'app, aggiornamento di Termux se è nuovo,
     pacchetto di Nyctalus da GitHub **verificato con l'impronta SHA-256**, motore giusto per il
     processore, installazione, ritorno all'app (che avvia il servizio).
     È l'unica cosa che l'app non può fare al posto tuo: Termux, per sicurezza, vuole che sia tu a
     dargli il permesso di ricevere comandi da altre app.
3. Collegati a una rete: `MANUALE-TELEFONO.md` §1 (a casa: `GUIDA-RETE-DI-CASA.md`).

Se usi già Termux per altro va bene quello: «⚡ Prepara Nyctalus» lo trova e salta il passo.

**Ogni processore** (dalla 0.3.17): l'app porta il motore per ARM a 64 e a 32 bit (molti tablet,
per esempio diversi Samsung Galaxy Tab A, hanno Android a 32 bit) e per Intel/AMD a 64 e 32 bit;
l'installatore sceglie da solo quello giusto. Se il motore non parte, Termux mostra processore,
versione di Android ed errore: mandali a chi sviluppa Nyctalus.

**Per aggiornare:** **Strumenti ▾ → ☁ Aggiornamenti da GitHub**: l'app controlla l'ultima
release, scarica la nuova app e apre l'installazione (tocchi «Aggiorna»). Riaprendola, propone da
sola di aggiornare anche Nyctalus in Termux (un tocco).

**Convive con gli altri progetti in Termux:** usa solo `~/nyctalus`, `~/.nyctalus-mobile`,
`~/.nyctalus-chat`, porte 18xxx e comandi `nyctalus-*`; installa solo i pacchetti che mancano,
non aggiorna mai quelli presenti, in `termux.properties` aggiunge una sola riga.

Come arriva il pacchetto: l'app lo offre per qualche minuto su `127.0.0.1` a un indirizzo
casuale; Termux lo scarica e ne **verifica l'impronta SHA-256** prima di installarlo.

**Senza l'app, o per riparare:** in Termux incolla la stessa riga qui sopra: installa o aggiorna
Nyctalus da sola (serve Internet).

### 2.3 Termux, facoltativo: i comandi da terminale (dalla 0.4.0)
Con l'app nuova Termux serve **solo** a chi vuole anche:

| Con Termux puoi… | Esempio |
|---|---|
| usare Nyctalus scrivendo comandi | `nyctalus-cli`: `scrivi mario ciao`, `file invia foto.jpg mario` |
| automatizzare | messaggi o file programmati con gli script di Termux |
| far passare dalla rete altri programmi | `curl`, `git`, `ssh` attraverso il tunnel (come `torsocks`) |
| pubblicare un sito `.nyct` dal telefono | `nyctalus-sito` |
| collaudo e diagnosi | `nyctalus-collaudo`, `nyctalus-integrita` |
| gestire le reti dal terminale | `nyctalus-rete` |

Si installa dall'app: **Strumenti ▾ → Termux (facoltativo) → Installa Termux**, poi **🦇 Installa i
comandi in Termux** (Termux scarica il pacchetto firmato dalla release e ne verifica la firma). Il
pulsante **Terminale** della schermata iniziale apre Termux. Per altre app che vogliono passare
dalla rete non serve Termux: l'app espone il proxy SOCKS `127.0.0.1:18080`.

### 2.2 A mano, senza l'app (facoltativo)
Serve `nyctalus-termux.tar.gz` (nel pacchetto Android). Poi si apre la chat nel browser del
telefono all'indirizzo che stampa `nyctalus-mobile`.

#### Portare l'archivio nel telefono
L'archivio deve finire nella cartella **Download** del telefono. Scegli il modo più comodo:
- **cavo USB**: collega il telefono al PC, scegli «Trasferimento file», copia
  `nyctalus-termux.tar.gz` nella cartella `Download`;
- **mandalo a te stesso** (mail, Telegram «Messaggi salvati», Drive…) e sul telefono
  salvalo in Download;
- **in rete locale** (PC e telefono sulla stessa Wi-Fi): sul PC, dalla cartella
  `installazioni/Android/`, lancia `python3 -m http.server 8080`; in Termux
  `curl -O http://IP-DEL-PC:8080/nyctalus-termux.tar.gz` (poi in «Installare» usa questo
  file al posto di quello in Download). Alcune reti aziendali lo bloccano.

#### Installare (una volta sola)
In Termux:
```sh
termux-setup-storage          # appare la richiesta di accesso ai file: concedila
cd ~
tar xzf ~/storage/downloads/nyctalus-termux.tar.gz
bash nyctalus-termux/installa.sh
```
- `termux-setup-storage` crea `~/storage/downloads` (la tua cartella Download) e
  permette anche di vedere le **foto del telefono** nel selettore degli allegati.
- Dalla versione **0.25** l'archivio (e quindi l'app) contiene già il **motore compilato per il
  telefono**, preparato dai server di GitHub dagli stessi sorgenti pubblicati: l'installazione
  dura pochi secondi.
- Se l'archivio non contiene già il motore compilato per il telefono, l'installatore lo
  **compila sul telefono**: scarica Rust (~300 MB) e impiega 10-30 minuti, **solo la
  prima volta**. Meglio sotto Wi-Fi e col telefono in carica.
- Opzione: `bash nyctalus-termux/installa.sh --avvio-automatico` fa partire Nyctalus
  all'accensione (serve l'app **Termux:Boot**).

#### Avviare
```sh
nyctalus-mobile
```
Stampa un indirizzo come `http://127.0.0.1:18731/?t=…`: tienilo premuto, copialo e
**aprilo in Chrome** sul telefono. La prima volta il browser ricorda l'accesso; dal
menu di Chrome puoi **«Aggiungi a schermata Home»** per avere l'icona Nyctalus.

Con l'**app Nyctalus** non serve copiare indirizzi: avvia il servizio in Termux da sola
(la prima volta Android chiede il permesso «Esegui comandi in Termux»: concedilo).
L'app usa un **suo segreto** d'accesso, che scrive in Termux all'avvio: da quel momento
l'indirizzo aperto prima nel browser non vale più (si riapre con `nyctalus-mobile --stato`).
Se l'app resta su «Nyctalus non risponde»: tocca **«📋 Comando di avvio»**, in
Termux tieni premuto → Incolla → Invio, poi torna nell'app.

| Comando | Cosa fa |
|---|---|
| `nyctalus-mobile` | avvia (in primo piano; Ctrl-C per fermare) |
| `nyctalus-mobile --sfondo` | avvia in sottofondo |
| `nyctalus-mobile --stato` | dice se è attivo e ristampa l'indirizzo da aprire |
| `nyctalus-mobile --ferma` | ferma tutto |

Uso dell'app: **`MANUALE-TELEFONO.md`** e **`MANUALE-CHAT.md`**.

### 2.3 Disinstallare
```sh
bash ~/nyctalus/disinstalla.sh
```
Toglie **solo** Nyctalus (programma, comando, avvio automatico). Chiede prima di
cancellare i tuoi dati (identità, contatti, messaggi). **Non** disinstalla pacchetti come
python o rust, che potrebbero servire ad altri progetti.

---

## 3. La rete: prova locale o condivisa
Appena installati, PC e telefono usano una **rete di prova locale** (solo su quel dispositivo).
Per parlare con altri ci si collega a una rete con il suo **codice** (`NYCT1:…`): PC
`MANUALE-PC.md` §3, telefono `MANUALE-TELEFONO.md` §1. Provare telefono ↔ PC senza server:
`GUIDA-RETE-DI-CASA.md`. Con un amico via Internet: `GUIDA-INTERNET.md`. Creare una rete: `LEGGIMI.md`.

## 4. Problemi frequenti

| Sintomo | Causa e rimedio |
|---|---|
| «porte già usate da un altro programma» | un altro progetto usa una delle porte 18xxx: cambiale in `~/.nyctalus-mobile/config.json` → `"porte": {…}` |
| la web-app dice «Accesso negato» | apri l'indirizzo con `?t=…` stampato da `nyctalus-mobile --stato` |
| nel selettore allegati non vedo le foto | in Termux: `termux-setup-storage` e consenti l'accesso ai file |
| le miniature dei video non compaiono | facoltativo: `pkg install ffmpeg` |
| la chat dice «rete di prova LOCALE» | normale senza configurazione: vedi il capitolo 3 |
| sul PC la fotocamera è disattivata | installa i pacchetti GStreamer del punto 1.1 |
