<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Scelte operative: cosa installare e dove

## I tre ruoli

| Ruolo | Che cosa fa | Dove | Pacchetto |
|---|---|---|---|
| **Client** | usa la rete: chat, browser | PC, telefono | `PC-Linux/` (scegli 1) · `Android/` · `Windows/` (anteprima) |
| **Nodo** | tiene in piedi la rete: inoltra il traffico cifrato | **server sempre acceso** raggiungibile da Internet (VPS, o a casa con IP dinamico) | `Server-Linux/` (anche arm64 per Raspberry Pi) |
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
**Cosa:** il PC fa da nodi e da gestore sulla Wi-Fi di casa; il telefono si collega. È il modo
per provare **due identità diverse** tra loro. Installa sul PC il **gestore** (`./installa.sh` → 2),
poi segui **`GUIDA-RETE-DI-CASA.md`** (5 passi).

Limiti: solo in casa e solo finché la finestra di `nyctalus-rete casa` resta aperta.

## Scenario C · Rete vera con un server (VPS)
**Cosa:** un server in affitto con 4 nodi; chiunque abbia il codice può usare la rete.

| Dove | Cosa fai | Manuale |
|---|---|---|
| **Server** | `sudo ./installa.sh --nodi 4` dal pacchetto `Server-Linux` → stampa 4 righe | `MANUALE-SERVER.md` |
| **Tuo PC (gestore)** | `nyctalus-rete crea` → `aggiungi` le 4 righe → `firma` | `MANUALE-GESTORE-RETE.md` |
| **Dove pubblichi** | metti `directory.txt` a un indirizzo web (anche sullo stesso server) | `MANUALE-GESTORE-RETE.md` §4 |
| **Server** (facoltativo) | rubrica e casella: `./gestore-rete/nyctalus-rete rubrica --codice … --servizio` | `MANUALE-SERVER.md` §6-bis |
| **Tuo PC (gestore)** | `nyctalus-rete codice --url URL` (con `--rubrica … --casella …`) → il codice `NYCT1:…` | `MANUALE-GESTORE-RETE.md` |
| **Ogni client** | PC: `nyctalus-rete importa CODICE` · telefono: **🔗 Rete** → Importa codice | `MANUALE-PC.md` §3, `MANUALE-TELEFONO.md` §1 |

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
