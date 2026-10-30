<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Scelte operative: cosa installare e dove

## I tre ruoli

| Ruolo | Che cosa fa | Dove | Pacchetto |
|---|---|---|---|
| **Client** | usa la rete: chat, browser | PC, telefono | `PC-Linux/` (scegli 1) · `Android/` |
| **Nodo** | tiene in piedi la rete: inoltra il traffico cifrato | **server sempre acceso** con IP pubblico (VPS) | `Server-Linux/` |
| **Gestore** | firma l'elenco dei nodi e dà il **codice di rete** ai client | il tuo PC (uno solo) | `PC-Linux/` (scegli 2) |

- I **telefoni sono solo client**. Non possono fare da nodo: cambiano indirizzo, non
  si possono raggiungere da Internet e Android li mette a dormire.
- Ogni percorso usa **3 nodi diversi**, quindi una rete ne ha almeno 3 (meglio 4).
- Il **codice di rete** (`NYCT1:…`) è l'unica cosa che un client deve ricevere: contiene
  il nome della rete, l'indirizzo dell'elenco e la chiave **pubblica** del gestore.
  Non contiene segreti.

---

## Scenario A · Provare da solo (nessun server)
**Cosa:** chat e browser su un dispositivo, con la rete di prova che si crea da sola.

1. Installa il **client** (PC: `./installa.sh` → 1 · telefono: `MANUALE-INSTALLAZIONE.md`).
2. Apri la chat: la striscia dice «Rete di prova LOCALE».
3. Aggiungi come contatto il **tuo stesso Contact ID**: messaggi e file fanno il giro
   completo della rete.

Limite: non puoi parlare con nessun altro dispositivo.

## Scenario B · Rete di casa: telefono ↔ PC (nessun server)
**Cosa:** il PC fa da nodi e da gestore sulla Wi-Fi di casa; il telefono si collega.
È il modo per provare **due identità diverse** tra loro.

1. Sul PC installa il **gestore** (`./installa.sh` → 2).
2. Sul PC: `nyctalus-rete casa` e lascia la finestra aperta. Stampa un codice `NYCT1:…`
   e un indirizzo `http://IP-DEL-PC:18800/`.
3. Sul telefono, sulla **stessa Wi-Fi**: apri quell'indirizzo nel browser e copia il
   codice. Poi nell'app vai in ⚙ Impostazioni → **Rete condivisa** → **Importa codice**.
4. Sul PC la chat usa già la rete di casa: `casa` importa il codice anche sul PC.
   Chiudi e riapri la chat.
5. Controlla che l'**impronta della rete** mostrata dal PC sia uguale a quella in ⚙ → Rete
   condivisa sul telefono.
6. Scambiatevi i Contact ID e scrivetevi.

Se il telefono resta a caricare la pagina, il firewall del PC blocca le porte: `casa` stampa
i due comandi `sudo ufw allow …` da dare una volta sola (vedi `MANUALE-GESTORE-RETE.md` §6).

Limiti: funziona solo in casa e solo finché la finestra di `casa` resta aperta.
`Ctrl-C` la ferma. Per tornare alla rete di prova: PC `nyctalus-rete importa --prova`,
telefono «↩ Rete di prova».

## Scenario C · Rete vera con un server (VPS)
**Cosa:** un server in affitto con 4 nodi; chiunque abbia il codice può usare la rete.

| Dove | Cosa fai | Manuale |
|---|---|---|
| **Server** | `sudo ./installa.sh --nodi 4` dal pacchetto `Server-Linux` → stampa 4 righe | `MANUALE-SERVER.md` |
| **Tuo PC (gestore)** | `nyctalus-rete crea` → `aggiungi` le 4 righe → `firma` | `MANUALE-GESTORE-RETE.md` |
| **Dove pubblichi** | metti `directory.txt` a un indirizzo web (anche sullo stesso server) | `MANUALE-GESTORE-RETE.md` §4 |
| **Tuo PC (gestore)** | `nyctalus-rete codice --url URL` → il codice `NYCT1:…` | `MANUALE-GESTORE-RETE.md` |
| **Ogni client** | PC: `nyctalus-rete importa CODICE` · telefono: ⚙ → Importa codice | questo file, sopra |

Limite: tutti i nodi stanno da un solo fornitore. La rete funziona davvero, ma quel
fornitore vedrebbe sia l'entrata sia l'uscita: va bene per provare, non per l'anonimato.

## Scenario D · Rete distribuita (più server, anche di amici)
**Cosa:** come C, ma con i nodi su **server e fornitori diversi**. È così che la rete
diventa anonima.

1. Su ogni server: pacchetto `Server-Linux`, `sudo ./installa.sh --nodi 1` (o 2).
2. Ognuno ti manda le sue righe (`nyctalus-server righe`): non sono segrete.
3. Tu le aggiungi, firmi e ripubblichi `directory.txt`. **Il codice non cambia**, perché
   restano uguali l'URL e la chiave: i client vedono i nodi nuovi al prossimo avvio.

---

## Domande rapide
- **Mi serve un server per provare?** No: scenari A e B.
- **Che server prendere?** Un VPS Linux x86_64 qualsiasi, anche il più piccolo (1 CPU,
  512 MB-1 GB di RAM), con IP pubblico e possibilità di aprire porte **UDP**.
- **Posso usare il PC di casa come nodo per gli altri?** Sì, se resta acceso e apri sul
  router le porte UDP verso il PC. Alcuni operatori però non danno un IP pubblico.
- **Ho cambiato o aggiunto nodi: devo ridare il codice?** No. Si ridà solo se cambi URL o
  ricrei la rete con `crea --sostituisci`.
- **Chi ha il codice può alterare la rete?** No: per firmare un elenco serve la chiave
  segreta, che resta in `~/.nyctalus-rete` sul PC del gestore. Fanne una copia di sicurezza.
