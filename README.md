<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol. -->

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
| **L3 Applicazione** | proxy SOCKS5 locale · siti interni · browser | 🟡 SOCKS5 fatto |
| **L2 Flusso** | etichette segrete + cifratura end-to-end + ricomposizione | ✅ |
| **L1 Cipolla** | strati simmetrici del percorso (data-plane) | 🟡 data-plane fatto |
| **L0 Collegamento** | QUIC tra nodi, camuffato da HTTPS | 🟡 QUIC fatto |

## A che punto siamo

**Fatto e testato** (42 test automatici):
- **L2 — nucleo del protocollo:** frammentazione, etichette segrete rotanti (BLAKE3), cifratura end-to-end (ChaCha20-Poly1305) con pacchetti di lunghezza fissa, ricomposizione con limiti anti-DoS.
- **Stretta di mano Noise NK:** i due estremi si accordano sul segreto senza farlo mai viaggiare; forward secrecy; il mittente resta anonimo.
- **Indirizzi `.nyct`:** la chiave pubblica di un servizio in base32, con controllo anti-errori di battitura.
- **L1 — livello a cipolla (data-plane):** gli strati simmetrici che ogni pacchetto attraversa.
- **CLI:** proxy SOCKS5 funzionante (`nyctalus avvia`) verso un nodo di uscita (`nyctalus uscita`), un salto, provato con `curl`.

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
nyctalus uscita --ascolta 0.0.0.0:4600
```
Stampa il comando `avvia` completo, con l'impronta del certificato. Sul client:
```sh
nyctalus avvia --a IP-USCITA:4600 --impronta <IMPRONTA> --socks 127.0.0.1:1080
curl --socks5-hostname 127.0.0.1:1080 https://example.com
```

> Questo è un **solo salto** (client → uscita): serve a rendere la CLI usabile
> subito. L'uscita vede l'IP del client: **non è ancora anonimo**. I nodi
> intermedi e la cipolla lo trasformeranno in un percorso anonimo.

### Trasferimento di un file (test del livello L2)

Sul ricevitore:
```sh
nyctalus ricevi --uscita ricevuto.bin --identita mia.chiave
```
Stampa il comando `invia` con l'indirizzo `.nyct` del ricevitore. Sul mittente:
```sh
nyctalus invia --a IP:4433 --impronta <IMPRONTA> --destinatario <INDIRIZZO>.nyct file
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
