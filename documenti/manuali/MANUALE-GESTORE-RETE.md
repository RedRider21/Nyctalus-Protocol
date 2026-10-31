<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Manuale del gestore della rete

Il **gestore** decide quali nodi fanno parte della rete. Firma il loro elenco con una
chiave che solo lui possiede e dà ai client il **codice di rete**. Basta un PC, anche
quello che usi tutti i giorni. Quadro generale: `SCELTE-OPERATIVE.md`.

Strumento: **`nyctalus-rete`**. Lo installa `./installa.sh` del pacchetto PC (scelte 1 e 2).

## 1. In breve
```sh
nyctalus-rete crea --nome "La mia rete"             # una volta sola
nyctalus-rete aggiungi --file righe-nodi.txt        # le righe ricevute dai server
nyctalus-rete firma                                 # → ~/.nyctalus-rete/directory.txt
# pubblica directory.txt a un indirizzo web (§4)
nyctalus-rete codice --url https://…/directory.txt  # → NYCT1:…  da dare ai client
nyctalus-rete verifica                              # i nodi rispondono?
```

## 2. Creare la rete (una volta)
```sh
nyctalus-rete crea --nome "La mia rete"
```
- Crea l'**autorità**: una chiave segreta e una pubblica, in `~/.nyctalus-rete/` (cartella
  privata, file 600).
- **Fai una copia di sicurezza** di `~/.nyctalus-rete/autorita.json` su una chiavetta.
  Se la perdi, non puoi più firmare: dovresti creare una rete nuova e ridare il codice a
  tutti.
- Un secondo `crea` viene rifiutato. Per ricominciare da capo serve `--sostituisci`.

## 3. Aggiungere e togliere nodi
I server ti mandano righe come `203.0.113.5:4601,1f59…,2ecd…` (`MANUALE-SERVER.md`).
```sh
nyctalus-rete aggiungi 'RIGA1' 'RIGA2' …        # oppure: --file righe-nodi.txt
nyctalus-rete togli 203.0.113.5:4601
nyctalus-rete elenco                             # nodi attuali, e se l'elenco è da firmare
nyctalus-rete firma                              # SEMPRE dopo ogni cambio (minimo 3 nodi)
```
Una riga con lo stesso indirizzo **sostituisce** la vecchia. Le righe sbagliate vengono
rifiutate.

## 4. Pubblicare l'elenco
I client scaricano `~/.nyctalus-rete/directory.txt` da un indirizzo web. Il file è
firmato: anche se qualcuno lo alterasse, i client lo rifiuterebbero. Scegli tu dove:

| Dove | Come |
|---|---|
| **Questo PC, rete di casa** | `nyctalus-rete servi`, finché resta aperto: `http://IP-DEL-PC:18800/directory.txt` |
| **Il server dei nodi** | copia il file in una cartella web del server (Nginx/Apache), oppure lancia lì `nyctalus-rete servi` |
| **Un sito o hosting statico** | carica `directory.txt` (anche le GitHub Pages del progetto, quando potremo aggiornarle) |

Dopo ogni `firma` ricarica il file nello stesso posto.

## 5. Il codice per i client
```sh
nyctalus-rete codice --url https://tuo-sito/directory.txt
```
Stampa `NYCT1:…` e lo salva in `~/.nyctalus-rete/codice.txt`.
- Contiene solo **nome, URL e chiave pubblica**: puoi mandarlo per messaggio o mail.
- **Va rifatto solo se cambi l'URL.** Aggiungere o togliere nodi non cambia il codice.
- Rubrica e casella, se le hai accese, entrano nel codice (§6-bis).
- I client lo importano: PC `nyctalus-rete importa NYCT1:…` (`MANUALE-PC.md` §3), telefono
  **🔗 Rete** → Importa codice (`MANUALE-TELEFONO.md` §1).

## 6. Rete di casa (prove senza server)
```sh
nyctalus-rete casa            # opzioni: --nodi 4  --ip IP  --porta 18800  --porta-base 4701
```
Fa tutto da solo su questo PC:
- accende i nodi (porte UDP 4701-4704);
- firma l'elenco con una rete separata, «Rete di casa», che non tocca la tua rete vera;
- lo pubblica su `http://IP-DEL-PC:18800/`: aprila sul telefono e tocca **📲 Collega l'app Nyctalus**. L'app si apre e chiede conferma mostrando nome e impronta della rete (il pulsante usa il link «intent» di Chrome; sotto c'è un link di riserva per gli altri browser). C'è anche **📋 Copia il codice**, da importare con «🔗 Rete» nell'app;
- accende la **rubrica della rete** e mette il suo indirizzo nel codice: chi importa il
  codice trova nella chat il pulsante **👥** per iscriversi e vedere chi usa la rete;
- importa il codice anche su questo PC.

- accende la **casella cifrata**, con un suo indirizzo `.nyct` anch'esso nel codice: chi usa la
  rete può scrivere a chi non è in linea (`MANUALE-CHAT.md` §3-ter);

La rubrica tiene i suoi dati in `~/.nyctalus-rete/casa/rubrica/` (`rubrica.json`, privato,
600): solo l'elenco degli iscritti, mai la cronologia. La casella in `rubrica/casella/` (cartelle
700, buste 600, illeggibili anche per te, cancellate al ritiro o dopo 14 giorni; al massimo 200
per destinatario e 50 MB in tutto; nessun registro di chi deposita). Porte locali (solo
127.0.0.1) 18810-18814, cambiabili con `NYCTALUS_RUBRICA_PORTE=a,b,c,d,e`. **Dopo un
aggiornamento riaccendi `casa` e ricollega i client** (📲 sul telefono, `importa` sul PC): il
codice nuovo contiene la casella.

Lascia la finestra aperta; `Ctrl-C` ferma tutto.

**Se il telefono resta a caricare la pagina, è il firewall del PC.** Su Ubuntu ufw scarta in
silenzio ciò che arriva dalla rete locale. `casa` se ne accorge e stampa i comandi; con la
rete `192.168.1.x` sono:
```sh
sudo ufw allow from 192.168.1.0/24 to any port 18800 proto tcp
sudo ufw allow from 192.168.1.0/24 to any port 4701:4704 proto udp
```
Valgono solo per la rete di casa. Per richiuderle usa le stesse righe con `delete allow`.

**Impronta della rete:** `casa` e `codice` stampano un'impronta breve (es. `92E6-9EB0`). Dopo
l'importazione il telefono la mostra nella riga **🔗 Rete** della schermata iniziale: deve essere **la stessa**. Così
sei sicuro che nessuno sulla Wi-Fi ti abbia passato un codice diverso. La pagina è in
`http` perché è solo nella rete locale e distribuisce dati pubblici; l'elenco dei nodi è
firmato, e l'impronta protegge il codice.

## 6-bis. Rubrica e casella per la tua rete
La rete di casa le accende da sola. Per una rete vera vanno tenute accese su una macchina
sempre in rete:
- **su questo PC** (dove hai la rete): `nyctalus-rete rubrica` (sempre accese: `--servizio`), poi
  `nyctalus-rete codice --url URL`, che le include da solo;
- **sul server** (consigliato): là `nyctalus-rete rubrica --codice NYCT1:… --servizio`
  (`MANUALE-SERVER.md` §6-bis), poi qui
  `nyctalus-rete codice --url URL --rubrica INDIRIZZO --casella INDIRIZZO` con i due indirizzi
  che ha stampato. Il codice li ricorda.

Ridai il codice nuovo ai client (sul telefono 📲 o «🔗 Rete», sul PC `nyctalus-rete importa`).

## 7. Sicurezza in tre righe
- La chiave **segreta** non esce mai da `~/.nyctalus-rete`: il codice e l'elenco
  contengono solo la pubblica.
- Chi ha il codice può **usare** la rete, non modificarla.
- Rete con tutti i nodi su un solo server: funziona, ma **non è anonima** verso quel
  fornitore. Per l'anonimato servono nodi su server diversi (scenario D).
