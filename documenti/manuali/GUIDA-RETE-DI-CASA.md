<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Guida rapida: chat fra PC e telefono a casa

Serve: un PC Linux con Nyctalus installato (ruolo **Gestore**) e un telefono Android con
l'app Nyctalus. PC e telefono sulla **stessa Wi-Fi**.

## 1. Sul PC: accendi la rete di casa
```sh
nyctalus-rete casa
```
Se il firewall blocca, chiede di aprirlo: rispondi **Invio** e metti la password, solo
la prima volta. Lascia la finestra aperta. Stampa un indirizzo (es. `http://192.168.1.14:18800/`)
e un'**impronta** (es. `92E6-9EB0`).

## 2. Sul telefono: collega l'app
Apri l'app Nyctalus e aspetta **«Nyctalus è pronto»**. Poi, nel browser del telefono, apri
l'indirizzo del punto 1 e tocca **📲 Collega l'app Nyctalus** → **Collegati**.
L'impronta deve essere **uguale** a quella del PC. Nella schermata dell'app la riga **🔗 Rete**
diventa «Rete di casa».

## 3. Apri le due chat
- **PC:** mini menu → **Chat** (o `nyctalus-chat`).
- **Telefono:** nell'app → **💬 Chat**.

## 4. Aggiungetevi
Ognuno copia il proprio **Contact ID**: sul PC in basso a sinistra (**Copia**), sul telefono in
**☰** (**📋 Copia**). Mandatelo all'altro, anche per mail. Poi aggiungete il contatto: **+** sul
PC, **✏️** sul telefono.

## 5. Scrivete
Dopo pochi secondi vi vedete **«in linea»**: la chat bussa da sola. Scrivete e mandate foto.

---

**Se qualcosa non va**

| Cosa vedi | Cosa fare |
|---|---|
| la pagina del punto 2 resta a caricare | firewall del PC: rilancia `nyctalus-rete casa` e rispondi sì alla domanda sul firewall |
| «📲 Collega l'app» non apre l'app | tocca «📋 Copia il codice», poi nell'app **🔗 Rete → Importa codice di rete** |
| l'app dice che Nyctalus non risponde | aspetta: lo riaccende da sola; altrimenti in Termux `nyctalus-mobile --sfondo` |
| «Messaggio non consegnato» | l'altro non è acceso o non è nella stessa rete: controlla la riga 🔗 Rete e che le chat siano aperte |

Più dettagli: `SCELTE-OPERATIVE.md` (scenario B) e `MANUALE-GESTORE-RETE.md`.
