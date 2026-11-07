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

   - `verifica_paradigmi.py CARTELLA [--camuffamento] [--rumore 0.2]`: **le scelte di fondo
     controllate sui pacchetti** (UDP/QUIC, entrata unica, ragnatela, cipolla, camuffamento, nome
     nel primo pacchetto decifrato come farebbe un osservatore, rumore di fondo, dimensioni).
     Risultati in `documenti/SICUREZZA.md` §1.1-ter.

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
