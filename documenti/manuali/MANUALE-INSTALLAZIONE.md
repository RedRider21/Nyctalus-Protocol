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
`SCELTE-OPERATIVE.md` (client, nodo server, gestore della rete).

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
tar xzf Nyctalus-PC-Linux.tar.gz        # crea la cartella Nyctalus/
cd Nyctalus
```

All'inizio l'installatore chiede **cosa stai installando**:
**1) Client** (il caso normale), **2) Gestore** (come il client, in più gestisci una rete),
**3) Nodo server** (la macchina fa da nodo: vedi `MANUALE-SERVER.md`). Senza domande:
`./installa.sh --ruolo client --si`.

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

Nyctalus sul telefono funziona come **Termux-NexusSEC-OS**: dentro **Termux** gira il
motore (lo stesso del PC) con un piccolo servizio, e l'interfaccia è una **web-app in
stile Telegram**, che apri nel browser del telefono oppure nell'app **Nyctalus**.

**Convive con gli altri progetti in Termux:** usa solo la cartella `~/nyctalus`, i dati
in `~/.nyctalus-mobile` e `~/.nyctalus-chat`, porte del blocco **18xxx**, comandi che
iniziano con `nyctalus-`; installa **solo i pacchetti che mancano** e **non aggiorna
mai** quelli già presenti; in `termux.properties` aggiunge una sola riga, senza
riscrivere il file.

### 2.1 Cosa scaricare
1. **Termux** — **non** dal Play Store (versione vecchia e ferma):
   - F-Droid: <https://f-droid.org/packages/com.termux/>
   - oppure GitHub: <https://github.com/termux/termux-app/releases> → il file
     `termux-app_…_arm64-v8a.apk` (quasi tutti i telefoni recenti sono *arm64-v8a*).

   Se usi già Termux per altri progetti, va bene quello che hai.
2. **`nyctalus-termux.tar.gz`** dalla cartella `installazioni/Android/`.
3. Facoltativa: **`Nyctalus.apk`**, l'app Nyctalus (circa 170 KB). Senza, usi il browser
   del telefono (funziona allo stesso modo). Per installarla Android chiede di consentire
   «Installa app sconosciute» all'app con cui la apri (File, Chrome…).

### 2.1-bis Il modo più semplice: con l'app
`Nyctalus.apk` contiene già il pacchetto per Termux, quindi **non serve copiare archivi**:
1. installa **Termux**, aprilo una volta e aspetta che finisca di prepararsi;
2. installa **`Nyctalus.apk`** e aprila; concedi il permesso «Esegui comandi in Termux»;
3. tocca **«🦇 Installa / aggiorna Nyctalus in Termux»**. Termux si apre e mostra
   l'installazione (lo stesso installatore del punto 2.3, che convive con gli altri
   progetti); alla fine l'app torna davanti da sola: tocca **Entra**.

Solo la **prima volta** Termux può non eseguire il comando da solo (manca ancora
l'impostazione `allow-external-apps`, che l'installatore poi aggiunge): in Termux tieni
premuto → **Incolla** → Invio. Il comando è già negli appunti.

Come funziona: l'app serve il pacchetto per qualche minuto su `127.0.0.1`, a un indirizzo
casuale; Termux lo scarica con `curl` e ne **verifica l'impronta SHA-256** prima di
installarlo (un pacchetto diverso viene rifiutato e non si installa nulla). Durante il
trasferimento compare la notifica «Nyctalus — Passo il pacchetto a Termux…»: serve a non
far «congelare» l'app da Android mentre Termux è in primo piano (problema delle versioni
fino alla 0.3, in cui `curl` restava appeso o falliva). Sparisce da sola a consegna fatta.

Se in Termux compare *«nyctalus-mobile: No such file or directory»* (APK 0.2), Nyctalus
non è ancora installato: tocca prima «🦇 Installa / aggiorna Nyctalus». Dalla 0.3 il
comando copiato se ne accorge da solo e installa.

Gli altri pulsanti dell'app:

| Pulsante | Cosa fa |
|---|---|
| ⬇ Installa Termux | apre F-Droid (consigliato) o la pagina GitHub di Termux |
| ↻ Aggiorna Termux | l'app Termux (F-Droid) oppure i pacchetti (`pkg upgrade`, chiede conferma: vale per **tutti** i progetti in Termux) |
| 🦇 Installa / aggiorna Nyctalus | come sopra; per aggiornare: installa la nuova APK e ritocca il pulsante |
| ⌨ Apri Termux · 📋 Comando di avvio | apre Termux; copia un comando da incollare in Termux che **avvia** Nyctalus e, se non è ancora installato, prima lo **installa** (vale 15 minuti) |
| ☁ Aggiornamenti da GitHub | in arrivo, quando il progetto sarà pubblicato |

Se preferisci fare a mano, segui i punti qui sotto.

### 2.2 Portare l'archivio nel telefono
L'archivio deve finire nella cartella **Download** del telefono. Scegli il modo più comodo:
- **cavo USB**: collega il telefono al PC, scegli «Trasferimento file», copia
  `nyctalus-termux.tar.gz` nella cartella `Download`;
- **mandalo a te stesso** (mail, Telegram «Messaggi salvati», Drive…) e sul telefono
  salvalo in Download;
- **in rete locale** (PC e telefono sulla stessa Wi-Fi): sul PC, dalla cartella
  `installazioni/Android/`, lancia `python3 -m http.server 8080`; in Termux
  `curl -O http://IP-DEL-PC:8080/nyctalus-termux.tar.gz` (poi al punto 2.3 usa questo
  file al posto di quello in Download). Alcune reti aziendali lo bloccano.

### 2.3 Installare (una volta sola)
In Termux:
```sh
termux-setup-storage          # appare la richiesta di accesso ai file: concedila
cd ~
tar xzf ~/storage/downloads/nyctalus-termux.tar.gz
bash nyctalus-termux/installa.sh
```
- `termux-setup-storage` crea `~/storage/downloads` (la tua cartella Download) e
  permette anche di vedere le **foto del telefono** nel selettore degli allegati.
- Se l'archivio non contiene già il motore compilato per il telefono, l'installatore lo
  **compila sul telefono**: scarica Rust (~300 MB) e impiega 10-30 minuti, **solo la
  prima volta**. Meglio sotto Wi-Fi e col telefono in carica.
- Opzione: `bash nyctalus-termux/installa.sh --avvio-automatico` fa partire Nyctalus
  all'accensione (serve l'app **Termux:Boot**).

### 2.4 Avviare
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

### 2.5 Disinstallare
```sh
bash ~/nyctalus/disinstalla.sh
```
Toglie **solo** Nyctalus (programma, comando, avvio automatico). Chiede prima di
cancellare i tuoi dati (identità, contatti, messaggi). **Non** disinstalla pacchetti come
python o rust, che potrebbero servire ad altri progetti.

---

## 3. La rete: prova locale o condivisa

Senza configurazione, PC e telefono avviano una **rete di prova locale**: tutto
funziona, ma solo su quel dispositivo, e non raggiungi altre persone. Per parlare con
gli altri ci si collega a una **rete condivisa** col **codice di rete** (`NYCT1:…`) che
dà chi la gestisce:
- **PC:** `nyctalus-rete importa NYCT1:…`, poi riapri chat e browser
  (`nyctalus-rete importa --prova` per tornare alla rete di prova);
- **telefono:** ⚙ Impostazioni → **Rete condivisa** → **Importa codice**.

Per provare **telefono ↔ PC** senza server: sul PC `nyctalus-rete casa` (rete di casa).
Come mettere in piedi una rete vera: `SCELTE-OPERATIVE.md`, `MANUALE-SERVER.md`,
`MANUALE-GESTORE-RETE.md`.

## 4. Problemi frequenti

| Sintomo | Causa e rimedio |
|---|---|
| «porte già usate da un altro programma» | un altro progetto usa una delle porte 18xxx: cambiale in `~/.nyctalus-mobile/config.json` → `"porte": {…}` |
| la web-app dice «Accesso negato» | apri l'indirizzo con `?t=…` stampato da `nyctalus-mobile --stato` |
| nel selettore allegati non vedo le foto | in Termux: `termux-setup-storage` e consenti l'accesso ai file |
| le miniature dei video non compaiono | facoltativo: `pkg install ffmpeg` |
| la chat dice «rete di prova LOCALE» | normale senza configurazione: vedi il capitolo 3 |
| sul PC la fotocamera è disattivata | installa i pacchetti GStreamer del punto 1.1 |
