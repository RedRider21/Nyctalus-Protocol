<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Simulazioni (PIANO-ANONIMATO, punto K)

Misure dell'anonimato con il **codice vero** di Nyctalus. Sono due tipi di misura:

1. **Scelta dei percorsi**, senza Shadow: quanto spesso un avversario che controlla alcuni nodi sta
   sia all'entrata sia all'uscita. Comando del motore:
   ```sh
   nyctalus simulate-paths --directory ELENCO --authority CHIAVE --adversary FAMIGLIA[,…] \
       [--users 1000] [--days 90] [--circuits-per-day 20]
   ```
   Confronta tre strategie: `old` (prima della Tappa 2), `no-guard` (entrata a caso a ogni
   circuito), `current` (guardia persistente e regola di parentela).
2. **Nyctalus dentro Shadow**: una Internet simulata (ritardi, banda, orologio simulato) in cui
   girano i programmi veri.
   - `shadow-base.sh CARTELLA [RITARDO_MS]`: 4 nodi, un utente, un sito; tempi e velocità di uno
     scaricamento attraverso 3 salti.
   - `correlazione.py CARTELLA [--utenti 6] [--giri 3] [--rumore HZ] [--seme N]`: **attacco di
     correlazione dei tempi** di un avversario che vede le linee degli utenti e del sito.
   - `riservata.py CARTELLA (--normale | --riservata) [--messaggi 15] [--seme N]`: **chi scrive a
     chi?** 6 utenti chattano a coppie con rubrica e casella; un avversario che vede le linee di
     tutti prova a scoprire le coppie (quattro attacchi, vale il migliore). Con `DIAGNOSI=1` motori
     e rubrica scrivono un registro dettagliato (`NYCTALUS_DIAGNOSI`, `rubrica_diagnosi.py`).

   - `tor_correlazione.py CARTELLA [--seme N]`: lo **stesso attacco con Tor 0.4.9** (rete di prova di
     Shadow: 1 autorità, 4 relè, 2 uscite), stessa Internet simulata e stessi utenti: confronto alla
     pari. Serve Tor compilato in `~/.local/share/nyctalus-sim/installato` (sotto).

   - `verifica_paradigmi.py CARTELLA [--camuffamento | --http3 SITO] [--rumore 0.2] [--porta 443]`: **le scelte di fondo
     controllate sui pacchetti** (UDP/QUIC, entrata unica, ragnatela, cipolla, camuffamento, nome
     nel primo pacchetto decifrato come farebbe un osservatore, rumore di fondo, dimensioni).
     Risultati in `documenti/SICUREZZA.md` §1.1-ter.

   - `velocita.py CARTELLA --sistema tor|nyctalus [--perdita 0.01] [--banda-nodi 10 | --bande A,B,…]
     [--utenti 4] [--mb 5] [--circuiti 2] [--congestione bbr|cubic|newreno] [--finestra KB]
     [--codifica K,R] [--senza-pesi] [--seme N]`: **confronto di
     velocità** in condizioni realistiche (pacchetti persi, nodi carichi, più utenti insieme). Tor:
     rete di prova di Shadow (7 relè); Nyctalus: 7 nodi. Ogni corsa dura pochi secondi.

   - `impronta_tls.py PORTA [--volte N]`: registra il ClientHello TLS di chi si collega e ne calcola
     l'impronta **JA4** (confronto Nyctalus / Chrome / curl: `documenti/SICUREZZA.md` §1.1-quater).

## Installare Shadow (una volta)

Servono compilatori, CMake, Rust e GLib (su Mint/Ubuntu già presenti con gli strumenti di
sviluppo). Niente da installare nel sistema, niente password:

```sh
mkdir -p ~/.local/share/nyctalus-sim && cd ~/.local/share/nyctalus-sim
git clone https://github.com/shadow/shadow.git && cd shadow
git checkout "$(cat …/simulazione/shadow-versione.txt)"
git apply …/simulazione/shadow-patch-nyctalus.patch
./setup build -j 8 --prefix ~/.local/share/nyctalus-sim/installato && ./setup install
rm -rf build      # libera circa 4 GB
```

Per il confronto con Tor: sorgenti di **libevent 2.1.13** e **Tor 0.4.9.16** (impronta SHA-256
verificata con quella pubblicata da Tor Project), compilati con
`./configure --prefix=~/.local/share/nyctalus-sim/installato` (Tor con `--with-libevent-dir` sulla
stessa cartella, `--disable-asciidoc --disable-unittests --disable-zstd`).

La **patch** (`shadow-patch-nyctalus.patch`) aggiunge a Shadow due cose che la libreria QUIC del
motore usa e che Shadow 3.3 non simulava: la chiamata di sistema `recvmmsg` (ricevuta come un
`recvmsg` alla volta) e l'opzione `IP_PKTINFO` (accettata senza effetto). Le altre opzioni non
simulate restano rifiutate apposta: accettarle senza simularle falserebbe i risultati.

## Risultati (07/11/2026)

**Scelta dei percorsi** (11 nodi di 5 gestori; 1000 utenti × 90 giorni × 20 circuiti al giorno).
«Collegati» = circuiti con l'avversario sia all'entrata sia all'uscita.

| Avversario | old | no-guard | **current** |
|---|---|---|---|
| un gestore (2 nodi su 11) | 0% o 100% (dipende dall'ordine dell'elenco) | 1,8% circuiti, 100% utenti | **0% circuiti, 0% utenti** |
| due gestori d'accordo (4 su 11) | idem | 10,9% · 100% | 8,0% · 65% |
| 4 nodi infiltrati non dichiarati (su 15) | idem | 5,7% · 100% | 5,3% · 51% |

Un singolo gestore disonesto non può più collegare chi con chi; contro accordi e infiltrati
servono **molti gestori indipendenti** (Tappa 3), guardie più lunghe e un controllo su chi entra.

**Nyctalus in Shadow** (30 ms fra due computer qualsiasi, 50/20 Mbit): 2 MB attraverso 3 salti in
circa 2,1 s (≈1 MB/s), primo byte dopo 0,48 s, come previsto dai ritardi.

**Attacco di correlazione** (6 utenti, 18 scaricamenti da 500 KB sovrapposti; a caso: 17%), media
di 3 ripetizioni:

| Rumore di fondo | L'avversario globale attribuisce giusti |
|---|---|
| nessuno | 78% |
| 0,2/s (di serie) | 83% |
| 5/s | 83% |
| 20/s | 74% |

Il rumore di fondo a celle sparse **non protegge gli scaricamenti** da chi vede entrambi i capi: è
il limite noto di tutte le reti in tempo reale, Tor compreso. Servono copertura costante pari al
traffico, oppure ritardi e rimescolamento (mixnet), praticabili per chat e messaggi.

**Confronto con Tor 0.4.9** (stesso attacco, stessa rete simulata, 3 ripetizioni ciascuno):

| | Tor | Nyctalus |
|---|---|---|
| l'avversario globale attribuisce giusti gli scaricamenti | 89-94% (media 91%) | 72-89% (media 80%) |
| 500 KB attraverso 3 salti | 1,10 s | 1,16 s |
| primo byte | 0,49 s | 0,50 s |

Sugli scaricamenti in tempo reale siamo **alla pari con Tor** (la piccola differenza non è
significativa con 18 scaricamenti per prova), con la stessa velocità.

**Chi scrive a chi** (6 utenti a coppie, 90 messaggi tutti consegnati; a caso 20%), 3 ripetizioni:

| Chat | L'avversario scopre le coppie |
|---|---|
| normale | **100%** (6 su 6, in tutte e tre) |
| modalità riservata | **17-33%**: al livello del caso |

La modalità riservata (Tappa 2-bis) regge contro un avversario che vede **tutte** le linee.

**Velocità in condizioni realistiche** (`velocita.py`, file da 5 MB, 3 ripetizioni, Mbit/s medi):

| Condizioni | Tor 0.4.9 | Nyctalus, Cubic (fino alla 0.3.7) | **Nyctalus, BBR (dalla 0.3.8)** |
|---|---|---|---|
| linea pulita | 6,4 | 10,2 | **9,7-10,5** |
| 0,5% di pacchetti persi | 3,5 | 1,7 | **3,5** |
| 1% | 2,3 | 1,3 | **2,2-2,4** |
| 2% | 0,47 | 0,77 | **0,98** |
| nodi da 10 Mbit, 4 utenti insieme | **2,9** | 2,2 | 2,0-2,5 |
| idem + 1% di perdite | 1,4 | 1,0 | **1,8** |

La prova da 500 KB del confronto precedente era dominata dai ritardi (alla pari). Con file più
grandi, su linea pulita siamo più veloci; con **Cubic** (il controllo di congestione di serie di
QUIC) ogni pacchetto perso dimezzava la velocità e Tor ci superava di ~1,8 volte: da qui il
passaggio a **BBR** (scelta locale, compatibile). Restano due punti da capire: con nodi carichi
Tor è avanti, e **più circuiti in parallelo non aiutano** in nessun caso (1, 2 e 4 vanno uguali).
Limite: Shadow simula il TCP di Tor con un suo modello, non quello di Linux; servono anche misure
su rete vera.

**Velocità 0.3.10** (distributore verso la strada più libera, finestra 256 KB, scelta pesata per
banda; `--bande 50,30,20,10,5,30,10` = nodi di velocità diverse; 3-5 ripetizioni, Mbit/s):

| Scenario | Tor | Nyctalus 0.3.10 |
|---|---|---|
| linea pulita | 6,4 | **10,9** |
| nodi diversi, 1 utente | 6,2 | **9,9** (a caso, senza bande: 3,9-4,2) |
| nodi diversi, 4 utenti | 2,6 | **4,8** |
| 1% di perdite | 2,0-2,3 | 2,2 |
| 2% di perdite | 0,47 | **1,1** |
| nodi tutti da 10 Mbit, 4 utenti | **2,9** | 2,4 |
| idem + 1% di perdite | 1,4 | **1,8** |

**Velocità 0.3.12** (celle piene: una cella parte quando è piena o dopo 2 ms; simulazioni
ripetibili per seme). Nodi saturi, 4 utenti, 20 semi: **Nyctalus 3,32** contro Tor 2,66 (prima
2,57); le catture mostrano 79 MB spediti dai nodi contro 94 di Tor per gli stessi 21 MB di dati
(prima 107: celle quasi vuote). Linea pulita 12,6; 1% di perdite 2,4; 2% 1,24; nodi diversi 10,0
(1 utente) e 5,7 (4 utenti); 8 utenti saturi 2,08 (Tor 1,47).

**Ragnatela codificata** (`--codifica 8,2`, `8,4`): non accelera (-5..-25%): ogni tratta QUIC recupera
già i pacchetti persi e le strade condividono entrata e uscita; spenta di serie.

**Difetti veri trovati dalla simulazione (corretti):**
1. un servizio `.nyct` serviva al massimo **4 client insieme** (il «posto» restava occupato finché il
   client non se ne andava): la rubrica non reggeva il 5° utente, un utente il 5° contatto collegato.
   Ora il posto si ri-registra subito (tetto di 256 client). Prova `31-servizio-molti-client`
   (con il motore vecchio: 4 su 8);
2. `rubrica.py` accettava solo nomi di busta con l'ora a 10 cifre e, se una conferma non
   corrispondeva, rispediva **all'infinito** la stessa busta. Ora 1-12 cifre e nessun giro infinito.

**Limite di Shadow (non del programma):** una riga molto lunga (~67 KB, un lotto di 4 buste) sul
collegamento **locale** fra `rubrica.py` e il suo motore blocca quel collegamento simulato; sul PC
vero funziona (prova `26`). In simulazione la rubrica consegna una busta per messaggio
(`NYCTALUS_LOTTO_POSTA`, solo per le prove).

## Risultati (11/11/2026): uscite diverse e guardie promosse

Opzioni nuove di `velocita.py`: `--siti S` (S siti uguali su indirizzi diversi), `--paralleli P`
(P scaricamenti insieme per utente, ognuno da un sito diverso), `--uscite N` (Nyctalus, uscite
diverse), `--guardie` (con `--aiuto`: i PC che aiutano sono promossi a guardia). La riga
`RISULTATO` ha in più `completamento` = tempo medio per finire TUTTI gli scaricamenti di un utente.
Dati grezzi: `misure-uscite-2026-11-11.txt`, `misure-guardie-2026-11-11.txt` (5 semi ciascuno,
nessuno scaricamento fallito).

**Uscite diverse** (`--banda-nodi 10 --mb 5 --siti 4 --paralleli 4`), completamento in secondi:

| | 1 uscita | 2 uscite | 4 uscite | Tor |
|---|---|---|---|---|
| 1 utente | 21,9 | 21,5 | 23,3 | 28,2 |
| 4 utenti | 55,8 | 56,5 | 60,1 | 71,2 |

Non accelera: la strettoia è la guardia (una sola per utente, comune a tutti i fasci).

**Guardie promosse** (`--banda-nodi 10 --mb 5 --aiuto 10`, con e senza `--guardie`):

| | solo intermedi | promossi a guardia |
|---|---|---|
| 4 utenti | 13,6 s | 14,7 s (+8%) |
| 8 utenti | 22,3 s | 18,3 s (−18%) |

Con più utenti le entrate diventano la strettoia e i PC promossi la allargano.

## Risultati (14/11/2026): le misure della tabella con la 0.3.21

Stessi scenari e stessi semi delle misure pubblicate, con il motore della release 0.3.21
(congelato in una copia, `NYCTALUS_BIN=…` per `velocita.py`). Tor non cambia (stesso Tor 0.4.9,
Shadow deterministico): i suoi valori restano quelli di prima. 48 corse, **nessuno scaricamento
fallito**. Dati grezzi: `misure-0.3.21-2026-11-14.txt` (`SCENARIO=…` davanti a ogni riga).

| Scenario | Tor | fino alla 0.3.19 | **0.3.21** |
|---|---|---|---|
| linea pulita (Mbit/s) | 6,4 | 12,6 | **13,6** |
| nodi diversi, 1 utente | 6,2 | 10,0 | **11,2** |
| nodi diversi, 4 utenti | 2,6 | 5,7 | **5,1** |
| 1% di perdite | 2,0-2,3 | 2,4 | **2,4** |
| 2% di perdite | 0,47 | 1,24 | **1,11** |
| nodi saturi, 4 utenti (20 semi) | 2,66 | 3,32 | **3,24** |
| nodi saturi, 8 utenti | 1,47 | 2,08 | **2,02** |
| 4 scaricamenti da 4 siti, 1 utente (s) | 28,2 | 21,9 | **21,5** |
| … 4 utenti (s, media / mediana) | 71,2 / 63,5 | 55,8 / 61,3 | **57,0 / 52,8** |
| primo byte (s) | 0,49 | 0,50 | **0,48** |

Le differenze fra le versioni stanno nel rumore della misura: le novità di 0.3.20 e 0.3.21
(cassaforte, collaudo, guardie di riserva, integrità) non toccano il trasporto dei dati.

