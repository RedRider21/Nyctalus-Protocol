<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Manuale d'uso — PC

Sul PC Nyctalus ha tre programmi grafici: il **launcher** (il pannello da cui parte
tutto), il **browser** e la **chat**. Sotto c'è sempre lo stesso **motore** in Rust
(`bin/nyctalus`), che fa crittografia e rete; i programmi grafici non vedono mai le
chiavi.

Installazione: `MANUALE-INSTALLAZIONE.md`. La chat ha il suo manuale: `MANUALE-CHAT.md`.

---

## 1. Il launcher (pannello da desktop)

Si avvia con `./avvia-launcher.sh`. È un piccolo pannello **senza bordi**, che puoi
**trascinare** ovunque sullo schermo con il tasto sinistro.

| Bottone | Cosa fa |
|---|---|
| 🦇 logo | apre la guida online (sito del progetto) nel browser |
| Browser | apre il browser Nyctalus |
| Chat | apre la chat (motore + servizio `.nyct` + finestra) |
| File | gestore dei file: in arrivo |
| ☰ | impostazioni (anche con il **tasto destro** sul pannello) |

**Impostazioni (☰ o tasto destro):**
- **Disposizione verticale / orizzontale** dei bottoni;
- **Più grande / Più piccola / Normale**: dimensione dei bottoni;
- **Sempre in primo piano**;
- **Tasti rapidi…**: scegli tu le combinazioni per Browser, Chat, File e
  Mostra/Nascondi il pannello. Premi il bottone accanto all'azione, poi la combinazione
  (Esc annulla, ✕ la toglie). **Sono vuote all'inizio**, per non scontrarsi con le
  scorciatoie del sistema: scegline di libere. Con il pacchetto `gir1.2-keybinder-3.0`
  funzionano **ovunque**; senza, solo quando il pannello è in primo piano.
- **Esci**.

Posizione, orientamento, dimensione e tasti rapidi vengono ricordati
(`~/.nyctalus-launcher/config.json`).

---

## 2. Il browser

Si avvia dal launcher o con `./avvia-browser.sh`. È un browser completo (schede,
preferiti, cronologia, download, ricerca nella pagina, traduzione, 5 lingue).

### Normale e Stealth
- Parte **normale**: naviga come un browser qualunque.
- Il bottone **Stealth** accende la rete Nyctalus: **avvia da solo i servizi** (rete
  pubblica se configurata, altrimenti una rete di prova locale) e da quel momento le
  nuove schede passano dalla rete anonima, senza cookie né cronologia su disco, con le
  protezioni contro le fughe di dati (WebRTC spento, User-Agent generico, niente
  geolocalizzazione).
- Le scritte del bottone: **Stealth OFF**, **Stealth (avvio…)**, **Stealth ON**,
  **Stealth ON · prova locale**, **Stealth (non connesso)**.

### Siti `.nyct`
Gli indirizzi `.nyct` sono siti interni della rete, come gli `.onion` di Tor. Se ne apri
uno, lo Stealth si accende da solo. Senza rete Nyctalus attiva **il browser li blocca**
(mostra «Rete Nyctalus non attiva») invece di farli uscire in chiaro.

---

## 3. La chat

Si avvia dal launcher (bottone Chat) o con `./avvia-chat.sh`. La finestra si apre
subito, mentre la rete finisce di avviarsi in sottofondo. Tutto il resto (contatti,
messaggi, allegati, fotocamera, menu del tasto destro, temi) è in **`MANUALE-CHAT.md`**.

---

### Collegarsi a una rete condivisa
Senza configurazione chat e browser usano una rete di prova solo sul PC. Per una rete
condivisa serve il **codice di rete** dato da chi la gestisce:
```sh
nyctalus-rete importa NYCT1:…     # poi chiudi e riapri chat e browser
nyctalus-rete stato               # a quale rete sei collegato
nyctalus-rete importa --prova     # torna alla rete di prova
nyctalus-rete casa                # rete di casa su questo PC (prova telefono ↔ PC)
```
Gestire una rete propria: `MANUALE-GESTORE-RETE.md`.

## 4. Dove stanno i dati

| Cartella | Contenuto |
|---|---|
| `~/.nyctalus-chat/` | identità (Contact ID), contatti, cronologia, allegati, aspetto della chat |
| `~/.nyctalus-browser/` | impostazioni, preferiti e cronologia del browser (solo in modalità normale) |
| `~/.nyctalus-launcher/` | posizione e impostazioni del pannello |
| `~/.nyctalus-rete/` | solo per chi **gestisce** una rete: chiavi dell'autorità (copia di sicurezza!), nodi, elenco firmato, rete di casa |
| `~/.cache/nyctalus-chat/` | miniature, foto e video della fotocamera prima dell'invio |

## 5. Verificare che tutto funzioni

Chi ha i sorgenti può lanciare la batteria di prove: vedi **`MANUALE-PROVE.md`**.
