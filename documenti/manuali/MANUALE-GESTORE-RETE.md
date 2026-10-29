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
- I client lo importano così:
  - **PC:** `nyctalus-rete importa NYCT1:…`, poi chiudi e riapri chat e browser
    (`nyctalus-rete importa --prova` per tornare alla rete di prova; `stato` per vedere
    la rete in uso);
  - **telefono:** ⚙ Impostazioni → **Rete condivisa** → **Importa codice** (la rete si
    riavvia da sola; «↩ Rete di prova» per tornare indietro).

## 6. Rete di casa (prove senza server)
```sh
nyctalus-rete casa            # opzioni: --nodi 4  --ip IP  --porta 18800  --porta-base 4701
```
Fa tutto da solo su questo PC:
- accende i nodi (porte UDP 4701-4704);
- firma l'elenco con una rete separata, «Rete di casa», che non tocca la tua rete vera;
- lo pubblica su `http://IP-DEL-PC:18800/`, una pagina col codice da aprire sul telefono;
- importa il codice anche su questo PC.

Lascia la finestra aperta; `Ctrl-C` ferma tutto. Se il telefono non si collega, il
firewall del PC blocca le porte: apri TCP 18800 e UDP 4701-4704 verso la rete locale.

## 7. Sicurezza in tre righe
- La chiave **segreta** non esce mai da `~/.nyctalus-rete`: il codice e l'elenco
  contengono solo la pubblica.
- Chi ha il codice può **usare** la rete, non modificarla.
- Rete con tutti i nodi su un solo server: funziona, ma **non è anonima** verso quel
  fornitore. Per l'anonimato servono nodi su server diversi (scenario D).
