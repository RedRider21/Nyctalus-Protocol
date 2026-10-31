<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Manuale del server (nodi della rete)

Un **nodo** inoltra il traffico cifrato della rete. Non vede cosa passa né chi parla con
chi. Su un server ne puoi far girare da 1 a 8. Quadro generale: `SCELTE-OPERATIVE.md`.

## 1. Cosa serve
- Un server **Linux** sempre acceso e raggiungibile da Internet: un VPS (1 CPU, 512 MB-1 GB
  di RAM, Debian o Ubuntu) oppure un computer di casa, anche un **Raspberry Pi** (§6).
- Accesso **root** (`sudo`). Senza root c'è `--utente`, vedi §7.
- Le porte **UDP** dei nodi aperte: per esempio 4601-4604.
- Il pacchetto giusto (dalla release o da `installazioni/`), solo motore, senza grafica:

  | Server | Pacchetto |
  |---|---|
  | PC, VPS (x86_64) | `Nyctalus-Server-Linux.zip` |
  | Raspberry Pi 3/4/5 con sistema a 64 bit, altri ARM | `Nyctalus-Server-Linux-arm64.zip` |

## 2. Portare il pacchetto sul server
Dal tuo PC:
```sh
scp Nyctalus-Server-Linux.zip utente@IP-DEL-SERVER:
ssh utente@IP-DEL-SERVER
unzip Nyctalus-Server-Linux.zip      # se manca: sudo apt install unzip
cd Nyctalus-Server
```

## 3. Installare
```sh
sudo ./installa.sh
```
L'installatore chiede:

| Domanda | Cosa rispondere |
|---|---|
| IP pubblico | quello del **pannello del fornitore** (l'installatore propone quello che vede). Con l'IP che cambia: un **nome** (§6) |
| Quanti nodi | **4** se è l'unico server della rete; **1-2** se ci sono altri server |
| Prima porta | `4601`, va bene la proposta (le altre seguono: 4602, 4603…) |

Senza domande:
```sh
sudo ./installa.sh --ip 203.0.113.5 --nodi 4 --porta 4601 --si
```

Cosa fa:
- motore in `/opt/nyctalus`;
- un servizio `nyctalus-nodo@PORTA` per nodo, che riparte da solo dopo un riavvio, gira
  con un utente dedicato `nyctalus` e privilegi ridotti;
- identità dei nodi in `/var/lib/nyctalus`;
- apre le porte su `ufw`/`firewalld`, se attivi.

**Alla fine stampa le RIGHE D'ELENCO**, una per nodo:
```
203.0.113.5:4601,1f59a3…,2ecd48…
```
**Mandale al gestore della rete.** Non sono segrete.

> **Firewall del fornitore:** molti VPS hanno anche un firewall nel **pannello web**.
> Apri lì le stesse porte **UDP**, altrimenti i nodi risultano irraggiungibili.

## 4. Gestire
```sh
nyctalus-server stato       # servizi attivi?
nyctalus-server righe       # ristampa le righe per il gestore
nyctalus-server riavvia     # (anche: avvia, ferma)
nyctalus-server log         # ultimi messaggi (log 4601 = solo quel nodo)
```
Le righe **non cambiano** tra un riavvio e l'altro: ogni nodo conserva la sua identità.

## 5. Opzioni in più
- `--camuffamento HEX`: il traffico sembra rumore a chi lo osserva. La chiave di 64
  caratteri esadecimali deve essere **uguale su tutti i nodi e sui client**: la decide
  il gestore.
- `--rumore HZ`: celle finte di copertura verso i client (es. `5`). Consuma banda.
- Per cambiare IP o opzioni: modifica `/etc/nyctalus/server.conf`, poi
  `nyctalus-server riavvia`. Se cambia l'IP, rimanda le righe al gestore (con un nome, §6, non serve).

## 6. A casa, con IP dinamico (No-IP, DuckDNS…)
1. Un **nome** che segue il tuo IP (es. `miacasa.ddns.net`), aggiornato dal router o dal
   programma del servizio.
2. Sul **router**: inoltra le porte **UDP** dei nodi (es. 4601-4604) al server/Raspberry.
3. Installa con il nome al posto dell'IP: `sudo ./installa.sh --ip miacasa.ddns.net --nodi 2`.
4. Manda le righe (`miacasa.ddns.net:4601,…`) al gestore.

Quando l'IP cambia non serve fare nulla: client e nodi risolvono il nome a ogni collegamento.
Nota: alcuni router non permettono ai dispositivi di casa di raggiungere il proprio nome
pubblico; in quel caso in casa si usano gli altri nodi della rete.

## 7. Senza root
```sh
./installa.sh --utente
sudo loginctl enable-linger $USER     # una volta: i nodi restano accesi senza login
```
Tutto finisce in `~/.local/share/nyctalus-server`, con servizi `systemctl --user`.

## 8. Aggiornare e disinstallare
- **Aggiornare:** scompatta il pacchetto nuovo e rilancia `sudo ./installa.sh` con le
  stesse risposte. Le identità restano: le righe non cambiano.
- **Disinstallare:** `sudo /opt/nyctalus/disinstalla.sh`. Toglie servizi e programma e
  **conserva** le identità. Con `--anche-identita` cancella anche quelle.
  Poi chiedi al gestore di togliere i nodi: `nyctalus-rete togli IP:PORTA`.

## 9. Problemi
| Sintomo | Causa probabile |
|---|---|
| il gestore vede i nodi «offline» con `nyctalus-rete verifica` | porte UDP chiuse sul **firewall del fornitore**, o IP sbagliato nelle righe |
| `il motore non parte su questo server` | pacchetto sbagliato: su ARM (Raspberry Pi) serve `Nyctalus-Server-Linux-arm64.zip` |
| nodi col nome «offline» | il nome non punta al tuo IP attuale (servizio dinamico non aggiornato) o porte non inoltrate dal router |
| `nyctalus-server stato` → `failed` | `nyctalus-server log`: spesso è la porta già usata da un altro programma |
