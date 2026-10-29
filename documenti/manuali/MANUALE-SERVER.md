<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Manuale del server (nodi della rete)

Un **nodo** inoltra il traffico cifrato della rete. Non vede cosa passa né chi parla con
chi. Su un server ne puoi far girare da 1 a 8. Quadro generale: `SCELTE-OPERATIVE.md`.

## 1. Cosa serve
- Un server **Linux x86_64** sempre acceso, con **IP pubblico**: un VPS va benissimo
  (1 CPU, 512 MB-1 GB di RAM, Debian o Ubuntu).
- Accesso **root** (`sudo`). Senza root c'è `--utente`, vedi §6.
- Le porte **UDP** dei nodi aperte: per esempio 4601-4604.
- Il pacchetto `installazioni/Server-Linux/Nyctalus-Server-Linux.zip`. Contiene solo il
  motore, senza grafica.

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
| IP pubblico | quello del **pannello del fornitore**. L'installatore propone quello che vede, ma dietro NAT può essere sbagliato |
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
  `nyctalus-server riavvia`. Se cambia l'IP, rimanda le righe al gestore.

## 6. Senza root
```sh
./installa.sh --utente
sudo loginctl enable-linger $USER     # una volta: i nodi restano accesi senza login
```
Tutto finisce in `~/.local/share/nyctalus-server`, con servizi `systemctl --user`.

## 7. Aggiornare e disinstallare
- **Aggiornare:** scompatta il pacchetto nuovo e rilancia `sudo ./installa.sh` con le
  stesse risposte. Le identità restano: le righe non cambiano.
- **Disinstallare:** `sudo /opt/nyctalus/disinstalla.sh`. Toglie servizi e programma e
  **conserva** le identità. Con `--anche-identita` cancella anche quelle.
  Poi chiedi al gestore di togliere i nodi: `nyctalus-rete togli IP:PORTA`.

## 8. Problemi
| Sintomo | Causa probabile |
|---|---|
| il gestore vede i nodi «offline» con `nyctalus-rete verifica` | porte UDP chiuse sul **firewall del fornitore**, o IP sbagliato nelle righe |
| `il motore non parte su questo server` | server non x86_64 (ARM): per ora serve x86_64 |
| `nyctalus-server stato` → `failed` | `nyctalus-server log`: spesso è la porta già usata da un altro programma |
