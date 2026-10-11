<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol. -->

<p align="center"><img src="logo.svg" width="130" alt="Nyctalus Protocol"></p>

# Nyctalus Protocol

**Una rete anonima ad alte prestazioni: l'anonimato di Tor, con la velocità adatta all'uso di ogni giorno.**

> ⚠️ **Stato: prototipo in sviluppo.** Il codice funziona ed è testato, ma il
> protocollo non è ancora completo né verificato da revisori indipendenti.
> **Non usarlo ancora per proteggere dati reali.**

Sito e manuale: **https://redrider21.github.io/Nyctalus-Protocol/**

---

## L'idea in una frase

Ogni utente è anche un nodo, così la rete diventa più veloce man mano che cresce.
Il traffico è camuffato da normale navigazione, e i dati viaggiano spezzati in
tanti frammenti su molte strade in parallelo — una **ragnatela** — per
ricomporsi solo a destinazione.

## Perché, se c'è già Tor

Tor funziona, ma è lento per ragioni strutturali. Nyctalus cambia le scelte di
fondo per guadagnare velocità senza rinunciare all'anonimato:

| | Tor | Nyctalus |
|---|---|---|
| Chi fa da nodo | ~7.000 volontari per milioni di utenti | ogni PC è anche un nodo: più utenti, più capacità |
| Strade per i dati | un solo percorso per connessione | ragnatela: più percorsi in parallelo dopo un ingresso fisso |
| Trasporto | TCP (un pacchetto perso blocca gli altri) | QUIC/UDP (un pacchetto perso non ferma il resto) |
| Camuffamento | opzionale (bridge) | sempre attivo, di serie |
| Siti interni | `.onion` | `.nyct`, con percorsi paralleli |
| Linguaggio | C (Tor classico) | Rust (niente bug di memoria) |

## Come viaggia un dato

```
                       ┌──► nodo ──► nodo ──┐
 Tu ──► [INGRESSO] ────┼──► nodo ──► nodo ──┼──► RICOMPOSIZIONE ──► destinazione
       (fisso per      ├──► nodo ──► nodo ──┤    (uscita verso Internet
        settimane)     └──► nodo ──► nodo ──┘     oppure sito interno .nyct)
```

1. **Camuffamento:** il traffico sembra una normale connessione HTTPS (QUIC su UDP 443).
2. **Ingresso fisso:** il primo nodo resta lo stesso per settimane; è l'unico che vede il tuo IP, ma non sa dove vai.
3. **Ragnatela:** i dati, spezzati in frammenti, viaggiano in parallelo su più strade.
4. **Cipolla:** ogni frammento è chiuso in più strati; ogni nodo ne toglie uno e scopre solo il nodo successivo.
5. **Ricomposizione:** a destinazione i frammenti tornano in ordine.

## Architettura a livelli

| Livello | Cosa fa | Stato |
|---|---|---|
| **L3 Applicazione** | proxy SOCKS5 locale · siti interni · browser | 🟡 SOCKS5 + circuito a 3 salti |
| **L2 Flusso** | etichette segrete + cifratura end-to-end + ricomposizione | ✅ |
| **L1 Cipolla** | apertura circuito (Sphinx) + strati simmetrici | ✅ |
| **L0 Collegamento** | QUIC tra nodi, camuffato da HTTPS | 🟡 QUIC fatto |

## A che punto siamo

**Fatto e testato** (53 test automatici):
- **L2 — nucleo del protocollo:** frammentazione, etichette segrete rotanti (BLAKE3), cifratura end-to-end (ChaCha20-Poly1305) con pacchetti di lunghezza fissa, ricomposizione con limiti anti-DoS.
- **Stretta di mano Noise NK:** i due estremi si accordano sul segreto senza farlo mai viaggiare; forward secrecy; il mittente resta anonimo.
- **Indirizzi `.nyct`:** la chiave pubblica di un servizio in base32, con controllo anti-errori di battitura.
- **L1 — livello a cipolla (data-plane):** gli strati simmetrici che ogni pacchetto attraversa.
- **CLI (comandi in inglese):** demone `nyctalus start` (SOCKS5 da configurazione, ragnatela conflux), `nyctalus status` e `nyctalus nodes` (elenco dei nodi, con `--probe` per vedere quali sono raggiungibili ora e il loro tempo di risposta); comandi di test `circuit`, `node`, `directory`, `authority`.
- **Elenco firmato dei nodi** (directory, autorità Ed25519) e **autenticazione dei collegamenti**: il client pinna il guard, ogni relè pinna il nodo a valle con l'impronta presa dall'elenco firmato.
- **Circuito condiviso**: un solo circuito trasporta più connessioni, multiplexate con celle dentro gli strati a cipolla (i relè restano ciechi ai singoli flussi).
- **Ponte verso i `.onion` di Tor**: l'uscita riconosce gli indirizzi `.onion` e li raggiunge attraverso un Tor già presente sul sistema (proxy SOCKS, `--tor-socks`, default `127.0.0.1:9050`); i siti normali vanno in TCP diretto.
- **Camuffamento del trasporto**: con una chiave di rete, ogni datagramma UDP è offuscato (ChaCha20): sul filo appare rumore casuale, senza la firma di QUIC riconoscibile da un DPI. Opzione `--obfuscation` / `obfuscation` nel config.
- **Ragnatela (conflux)**: un singolo scaricamento viene **spezzato su più circuiti** (guard e uscita condivisi, intermedi diversi) e **ricomposto a destinazione** grazie a una sequenza end-to-end dentro l'onion — così sfrutta la banda di più strade.
- **Anti-correlazione**: il numero di sequenza non viaggia in chiaro (nonce da contatore locale per direzione).

**In arrivo** (vedi [STATO.md](STATO.md)): apertura del circuito con Sphinx, ID di circuito per tratta, nodi intermedi, ragnatela completa, simulazione contro Tor con Shadow, camuffamento, browser dedicato.

## Compilare

Serve [Rust](https://rustup.rs) (profilo minimal sufficiente):

```sh
cargo test              # 42 test
cargo build --release   # produce target/release/nyctalus
```

`cargo clean` svuota la cartella `target/` se serve spazio.

## Uso da terminale

Nyctalus è, come Tor, un demone con interfaccia a riga di comando; il browser
è solo uno dei client. Comandi previsti (`avvia`, `stato`, `sito`, `monitor`,
`esec`); oggi funzionano questi:

### Proxy SOCKS5 (un salto — non ancora anonimo)

Sul nodo di uscita:
```sh
nyctalus exit --listen 0.0.0.0:4600
```
Stampa il comando `avvia` completo, con l'impronta del certificato. Sul client:
```sh
nyctalus single-hop --to IP-USCITA:4600 --fingerprint <FINGERPRINT> --socks 127.0.0.1:1080
curl --socks5-hostname 127.0.0.1:1080 https://example.com
```

> Questo è un **solo salto** (client → uscita): serve a rendere la CLI usabile
> subito. L'uscita vede l'IP del client: **non è ancora anonimo**. I nodi
> intermedi e la cipolla lo trasformeranno in un percorso anonimo.

### Circuito anonimo a 3 salti (autenticato)

```sh
# 1) crea un'autorità per firmare l'elenco dei nodi
nyctalus authority                      # stampa segreto + pubblica

# 2) avvia i nodi (stampano la loro riga per l'elenco)
nyctalus node --listen 0.0.0.0:4601 --directory directory.txt --authority <PUB>
nyctalus node --listen 0.0.0.0:4602 --directory directory.txt --authority <PUB>
nyctalus node --listen 0.0.0.0:4603 --directory directory.txt --authority <PUB>

# 3) firma l'elenco con le righe stampate dai nodi
nyctalus directory --authority-secret <SECRET> \
  --entries 'addr,x,fp;addr,x,fp;addr,x,fp' > directory.txt

# 4) proxy SOCKS5 attraverso il circuito a 3 salti
nyctalus circuit --directory directory.txt --authority <PUB> --socks 127.0.0.1:1080 \
  --circuits 2 --hops 3
curl --socks5-hostname 127.0.0.1:1080 http://example.com
```

Con l'elenco firmato ogni collegamento è autenticato: un elenco manomesso o
firmato da un'autorità diversa viene rifiutato.

### Demone da configurazione

Invece di passare tutto a riga di comando, si può usare un file
`~/.config/nyctalus/config`:

```ini
directory = /percorso/directory.txt
authority = <PUB>
socks     = 127.0.0.1:1080
circuits  = 2
hops      = 3
```

```sh
nyctalus status     # mostra la configurazione e valida l'elenco
nyctalus start      # avvia il proxy SOCKS5 attraverso la ragnatela conflux
```

### Elenco dei nodi e scelta del target

```sh
nyctalus nodes                      # elenco firmato dei nodi (indirizzo, chiave, impronta)
nyctalus nodes --probe              # + verifica quali sono raggiungibili ora, con il tempo di risposta
nyctalus nodes --probe --timeout 2  # tempo massimo di attesa per nodo (secondi)
```

`nodes` legge `directory`/`authority` dal config (o da `--directory`/`--authority`).
Senza `--probe` non contatta nessuno: mostra solo l'elenco. Con `--probe` apre
verso ogni nodo la stessa connessione QUIC autenticata di un circuito e segna
`online`/`offline`. I valori `address` e `fingerprint` stampati si incollano
direttamente in `single-hop`, `send` o `circuit` per **scegliere un target**.

### Programmi e monitoraggio

```sh
# lancia un programma instradandone il traffico nella rete (stile torsocks)
nyctalus exec -- curl https://example.com
nyctalus exec --socks 127.0.0.1:1080 -- git clone https://…

# cruscotto testuale con le metriche del demone (stile nyx), aggiornato ogni secondo
nyctalus monitor --control 127.0.0.1:1090
```

`exec` funziona con i programmi che rispettano le variabili `ALL_PROXY`/`http_proxy` (curl, git, wget…); il demone pubblica le metriche sulla porta di controllo (`--control`, default `127.0.0.1:1090`).

**Impostazioni del percorso:**
- `--circuits N` (o `circuits` nel config) — strade parallele usate contemporaneamente. **Minimo 1, massimo 8**, default **2**.
- `--hops N` (o `hops`) — nodi per percorso (guard + intermedi + uscita). **Minimo 1, massimo 5** (limite di Sphinx), default **3**. Sotto i 3 salti non c'è vero anonimato.

**Affidabilità:** il trasporto è QUIC (su UDP), che **recupera da solo i pacchetti persi** e li riconsegna in ordine; un pacchetto perso non blocca gli altri flussi (niente head-of-line blocking fra flussi).

### Trasferimento di un file (test del livello L2)

Sul ricevitore:
```sh
nyctalus receive --out received.bin --identity my.key
```
Stampa il comando `invia` con l'indirizzo `.nyct` del ricevitore. Sul mittente:
```sh
nyctalus send --to IP:4433 --fingerprint <FINGERPRINT> --recipient <ADDRESS>.nyct file
```

## Cosa promette e cosa no

**Protegge da:** provider Internet e reti Wi-Fi, tracciamento e profilazione,
censura e blocchi, siti che vogliono il tuo IP, singoli nodi malevoli.

**Non promette:** protezione totale contro chi sorveglia contemporaneamente
gran parte di Internet — contro un avversario simile nessuna rete veloce può
farlo, e su questo la protezione sarà pari a Tor, non superiore. E non protegge
chi si identifica da solo (login, dati personali).

## Documenti

- [VISIONE.md](VISIONE.md) — l'idea, le scelte, il percorso verso il protocollo di riferimento
- [SPECIFICA.md](SPECIFICA.md) — modello di minaccia, livelli, formati dei pacchetti
- [STATO.md](STATO.md) — punto dei lavori e cose da fare

## Il nome

Dal genere *Nyctalus*, le nottole: pipistrelli che volano veloci di notte e si
orientano al buio. Dal greco *nyx*, notte.

## Licenza

[AGPL-3.0-or-later](LICENSE). © 2026 Daniele Deplano (RedRider21).
