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

Installato, è nel menu delle applicazioni come **«Nyctalus»** (o col comando `nyctalus-launcher`); dalla cartella scompattata `./avvia-launcher.sh`; dai sorgenti `strumenti/avvia-launcher.sh` (dopo il riordino non sta più in `launcher/` ma in `pc/launcher/`). È un piccolo pannello **senza bordi**, che puoi
**trascinare** ovunque sullo schermo con il tasto sinistro.

| Bottone | Cosa fa |
|---|---|
| 🦇 logo | apre la guida online (sito del progetto) nel browser |
| Browser | apre il browser Nyctalus |
| Chat | apre la chat (motore + servizio `.nyct` + finestra) |
| Rubrica | apre la sola **rubrica della rete**, in una finestra a sé (la finestra della chat compare solo quando scegli qualcuno a cui scrivere; chiusa la rubrica, se la chat non era aperta si chiude tutto): iscriversi, chi è in linea, aggiungere e scrivere (`MANUALE-CHAT.md` §3-bis) |
| File | il programma **File**: manda file grandi ai contatti, con pausa e ripresa (`MANUALE-FILE.md`) |
| Terminale | il terminale del sistema pronto per **nyctalus-cli**: chat e file scrivendo comandi; se il motore è spento lo accende e lo rispegne quando chiudi il terminale (`MANUALE-TERMINALE.md`) |
| ☰ | impostazioni (anche con il **tasto destro** sul pannello) |

**Impostazioni (☰ o tasto destro):**
- **Disposizione verticale / orizzontale** dei bottoni;
- **Più grande / Più piccola / Normale**: dimensione dei bottoni;
- **Sempre in primo piano**;
- **Tema** (Notte, Ardesia, Viola, Chiaro, Menta, Cielo, **Personalizzato…** con selettore di
  colore e chiaro/scuro): **uno solo per tutto Nyctalus** sul PC — mini
  menu, chat, finestre delle chiamate e browser (che passa a chiaro/scuro con l'accento del
  tema). Cambia **al volo** nei programmi già aperti; sceglierlo nella chat (⚙) è lo stesso.
  Sta in `~/.config/nyctalus/tema`.
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

> **In più: Nyctalus Firefox** (menu delle applicazioni, oppure ⚙ del mini menu → 🦊). Non
> sostituisce questo browser: serve quando sul web normale non vuoi essere riconosciuto dalla
> sua «impronta» (il motore di questo browser è usato da pochi; Firefox con le protezioni di Tor
> Browser ti fa somigliare a milioni di altri). Tutto attraverso la rete; a rete spenta non esce niente. Il tuo Firefox
> di tutti i giorni non viene toccato. Profilo in `~/.nyctalus-firefox/` (`GUIDA-COMPLETA.md` §8).

Si avvia dal launcher o con `./avvia-browser.sh`. È un browser completo (schede,
preferiti, cronologia, download, ricerca nella pagina, traduzione, 5 lingue).

### Normale e Stealth
- Parte **normale**: naviga come un browser qualunque.
- Il bottone **Stealth** accende la rete Nyctalus: **avvia da solo i servizi** (rete
  pubblica se configurata, altrimenti una rete di prova locale) e da quel momento le
  nuove schede passano dalla rete anonima, senza cookie né cronologia su disco, con le
  protezioni contro le fughe di dati (WebRTC spento, niente geolocalizzazione).
- **Impronta uniforme**: nelle schede Stealth il browser si presenta a tutti i siti
  **uguale per tutti gli utenti di Nyctalus**, qualunque sia il PC: identità Safari su macOS
  (coerente con il motore), lingua inglese, ora UTC, schermo arrotondato, 2 processori,
  niente WebGL né Web Audio, immagini disegnate (canvas) non leggibili; anche nelle cornici
  interne. Due PC con fuso e lingua diversi danno la stessa impronta (prova `30`).
  Limite onesto: il motore WebKitGTK è usato da pochi e un sito molto determinato può accorgersi
  dei valori coperti; per i casi delicati sul web normale c'è in più **Nyctalus Firefox**.
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
messaggi, allegati, fotocamera, menu del tasto destro, temi, **chiamate 📞 e videochiamate 📹**) è in **`MANUALE-CHAT.md`**.

---

### Accendere la propria rete (🏠 La mia rete)
Dal **mini menu → ⚙ → 🏠 La mia rete…** (dalla 0.3.23) si accende la **propria** rete senza
terminale: «Solo in casa» oppure «Anche da Internet» con il nome o l'IP pubblico; spunte **Senza
uscita verso Internet** (consigliata) e **Sempre accesa** (servizio che riparte da solo dopo un
riavvio); in «Avanzate» numero di nodi e porte. In fondo la finestra mostra il **comando
equivalente** (`nyctalus-rete casa …`, utilizzabile anche nel terminale), poi **▶ Accendi** lo
esegue e mostra codice di rete e passi; **■ Spegni** ferma la rete (in entrambi i modi);
**🔓 Apri il firewall** apre le porte con `ufw` (chiede la password). Le scelte si ricordano.
Senza «Sempre accesa» la rete resta accesa anche chiudendo il mini menu, fino a «Spegni» o allo
spegnimento del PC.

### Collegarsi a una rete condivisa
Senza configurazione chat e browser usano una rete di prova solo sul PC. Per una rete
condivisa serve il **codice di rete** dato da chi la gestisce. Dal **mini menu → ⚙ → 🔗
Collegati a una rete…**: incolli il codice (se l'hai copiato è già proposto), tocchi **Collegati**
e ti mostra l'**impronta** da confrontare con chi gestisce la rete; c'è anche «Torna alla rete di
prova». Oppure da terminale:
```sh
nyctalus-rete importa NYCT1:…     # poi chiudi e riapri chat e browser
nyctalus-rete stato               # a quale rete sei collegato
nyctalus-rete importa --prova     # torna alla rete di prova
nyctalus-rete reti                # le reti salvate (● quella in uso)
nyctalus-rete usa N               # passa alla rete salvata N
nyctalus-rete ponte aggiungi 'RIGA'   # PONTE NASCOSTO: si entra sempre da lì (GUIDA-COMPLETA §13)
nyctalus-rete aiuta si --indirizzo NOME --tetto 10   # 🦇 AIUTA LA RETE: il PC fa da nodo intermedio (anche dal launcher ⚙)
nyctalus-rete ponte elenco | togli IP:PORTA | togli tutti
nyctalus-rete casa                # rete di casa su questo PC (prova telefono ↔ PC)
```
Gestire una rete propria: `MANUALE-GESTORE-RETE.md`.

## 3-bis. Pubblicare un sito `.nyct`
`nyctalus-sito` (guida passo passo) crea un sito, con le pagine in una cartella oppure dietro il tuo Apache, e lo mette online sulla rete di questo PC; vedi
`MANUALE-SITI.md`.

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
