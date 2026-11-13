<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Manuale del collaudo (nyctalus-collaudo)

Le prove automatiche del progetto (`MANUALE-PROVE.md`) girano tutte **sul tuo PC**, con una
rete finta. **nyctalus-collaudo** fa l'altra metà del lavoro: controlla la **tua rete vera**,
su Internet, dal dispositivo da cui lo lanci, e alla fine ti dice con ✓ e ✗ che cosa funziona
e che cosa no.

Non cambia nulla: legge la configurazione, usa il tunnel acceso (se è spento ne accende uno
temporaneo e lo spegne alla fine) e stampa un rapporto.

---

## 1. Lanciarlo

```
nyctalus-collaudo                 # il collaudo automatico (2-3 minuti)
nyctalus-collaudo --guidato       # …più i passi da fare a mano (browser, chiamata, file)
nyctalus-collaudo --rapporto collaudo.txt     # salva anche il rapporto in un file
```

| Dove | Come |
|---|---|
| PC Linux (installato) | `nyctalus-collaudo` in un terminale |
| Dalla cartella scaricata, senza installare | `cli/nyctalus-collaudo` |
| Telefono | nel **⌨ Terminale** dell'app o in Termux: `nyctalus-collaudo` |
| Server / nodo via SSH | `nyctalus-collaudo --solo-rete` (vedi §3) |

Conviene lanciarlo **da ogni dispositivo** che usa la rete (PC di casa, telefono sulla rete
mobile, PC dell'amico): ognuno vede Internet da un punto diverso, e un nodo raggiungibile da
casa può non esserlo dal telefono.

---

## 2. Che cosa controlla

| # | Controllo | Cosa vuol dire ✓ | Se è ✗ |
|---|---|---|---|
| 1 | **Configurazione** | c'è il motore, la rete è configurata (codice `NYCT1:`), si vede la forma del traffico, rubrica e casella | incolla il codice di rete nelle Impostazioni |
| 2 | **Nodi** | l'elenco è firmato dall'autorità della rete; i nodi rispondono (con i tempi) | nodi spenti, porte chiuse sul router (UDP **e** TCP), nome No-IP non aggiornato |
| 2 | **Sonde senza chiave** | a chi **non** ha la chiave della rete i nodi non rispondono affatto | quel nodo gira senza la protezione della rete: va riavviato con il codice aggiornato |
| 3 | **Tunnel** | il circuito a 3 salti si costruisce | meno di 3 nodi vivi, o chiave/forma diverse fra te e i nodi |
| 4 | **Indirizzo IP** | dal tunnel Internet si raggiunge, e i siti vedono l'indirizzo dell'**uscita**, non il tuo | l'uscita è sulla tua stessa linea (rete tutta in casa): serve un'uscita altrove |
| 5 | **Nomi dei siti (DNS)** | mentre navighi dal tunnel, dal tuo computer non esce nessuna richiesta DNS per quel sito | il programma risolve i nomi da sé: va usato il proxy «con i nomi remoti» |
| 6 | **Velocità** | tempo per aprire una pagina e MB/s di uno scaricamento di prova | un nodo del circuito ha poca banda (`nyctalus-rete banda`) |
| 7 | **Chat e servizi** | il motore della chat risponde, i dati sono protetti o no, rubrica e casella si raggiungono dentro la rete | servizi del gestore spenti (`nyctalus-rete stato`) |
| 8 | **Passi a mano** (`--guidato`) | browser, messaggio, posta per chi è spento, chiamata, videochiamata, file, nodo spento a metà, cassaforte | rispondi «n» e scrivi cosa è successo: finisce nel rapporto |

I segni: **✓** superato · **✗** fallito · **!** funziona ma c'è qualcosa da guardare ·
**–** saltato (con il motivo).

### Note sui singoli controlli

- **IP (4).** Per sapere quale indirizzo vedono i siti, lo strumento lo chiede a due servizi
  pubblici («che IP vedi?»), una volta dal tunnel e una volta in diretta, e confronta. La
  domanda in diretta rivela a quei servizi soltanto che il tuo indirizzo ha chiesto il suo
  indirizzo; se non la vuoi fare: `--senza-diretto` (resta il controllo che dal tunnel si
  esce, senza il confronto). Nel rapporto il tuo indirizzo vero compare **mascherato**.
- **Rete tutta in casa.** Se i nodi stanno tutti sulla tua linea, l'uscita sei tu: il
  controllo 4 fallisce **giustamente**. Non è un guasto: è il limite di una rete di un solo
  soggetto (`PIANO-ANONIMATO.md`).
- **DNS (5).** Serve poter ascoltare la rete del proprio computer: `dumpcap` (Wireshark) o
  `tcpdump` con il permesso di cattura. Senza, il controllo risulta «saltato» e lo strumento
  dice come abilitarlo. Lo strumento fa anche una controprova: se l'ascolto non vede nemmeno
  una richiesta DNS fatta apposta, lo dichiara invece di dare un ✓ che non vale nulla.
  Sul telefono (Termux senza root) questo controllo è sempre saltato.
- **Sonde (2).** Con la forma **HTTP/3** i nodi rispondono apposta a chiunque come un sito
  qualsiasi: lì il controllo è saltato, non è un difetto.
- **Velocità (6).** `--mb 20` per uno scaricamento più lungo (di serie 3 MB);
  `--senza-velocita` per saltarla (rete a consumo).

---

## 3. Opzioni

| Opzione | A cosa serve |
|---|---|
| `--guidato` | aggiunge le domande sui passi da fare a mano |
| `--rapporto FILE` | salva il rapporto (testo) da conservare o mandare a chi ti aiuta |
| `--solo-rete` | solo nodi, sonde, tunnel e servizi interni: **niente** verso Internet (per una rete di casa senza uscita pubblica, o da un server) |
| `--senza-diretto` | non chiede in diretta il tuo indirizzo vero |
| `--senza-velocita` · `--mb N` | salta la prova di velocità · megabyte da scaricare |
| `--contatto NOME` | manda anche un messaggio di prova a quel contatto (serve la chat accesa) |
| `--socks 127.0.0.1:PORTA` | usa proprio quel tunnel |
| `--config FILE` | un file di configurazione diverso da quello trovato |
| `--json` | in fondo, gli esiti anche in JSON (per gli script) |

Il comando esce con **0** se non c'è nessun ✗, con **1** altrimenti: si può usare in uno
script o in un controllo periodico (`nyctalus-collaudo --solo-rete --senza-diretto || avvisami`).

---

## 4. Leggere il rapporto

```
── 2 · Nodi della rete
  ✓ elenco firmato dall'autorità della rete: 6 nodi
  ! 5 nodi su 6 rispondono (ne bastano 3 per un circuito)
      spenti o irraggiungibili: casa.example.net:4603
  ✓ a chi non ha la chiave della rete i nodi non rispondono (6 su 6 muti)
…
════════════════════════════════════════════════════════════
✓ COLLAUDO SUPERATO, con cose da guardare   14 superate · 0 fallite · 1 da guardare · 2 saltate
```

Ordine consigliato per sistemare: prima i ✗ del punto 2 (senza nodi non funziona niente),
poi il 3, poi il resto. Il rapporto salvato non contiene chiavi, segreti né il tuo indirizzo
intero: lo puoi mandare a chi gestisce la rete.

---

## 5. Cosa NON dimostra

Un collaudo superato dice che la rete **funziona** e che gli errori grossolani non ci sono
(IP in chiaro, DNS fuori dal tunnel, nodi che rispondono a chiunque). **Non** dimostra
l'anonimato contro chi osserva molti punti di Internet, né sostituisce una revisione
indipendente: per questo vedi `SICUREZZA.md` §2 (verifiche manuali su rete reale) e
`PIANO-ANONIMATO.md`.
