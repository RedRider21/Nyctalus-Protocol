# Nyctalus Protocol

Rete anonima ad alte prestazioni: ogni utente è anche un nodo, il traffico è camuffato da normale navigazione, e i dati viaggiano spezzati in frammenti cifrati su più strade in parallelo, per ricomporsi solo a destinazione.

- **Idea e scelte:** [VISIONE.md](VISIONE.md)
- **Dettagli tecnici:** [SPECIFICA.md](SPECIFICA.md)

> Stato: **prototipo iniziale**. Da non usare per proteggere dati reali.

## Struttura

| Cartella | Contenuto |
|---|---|
| `crates/nyctalus-core` | Nucleo del protocollo: etichette segrete, cifratura dei frammenti, ricomposizione |
| `crates/nyctalus` | Programma di prova: trasferimento di un file su QUIC |

## Compilare

Serve Rust (profilo minimal sufficiente):

```sh
cargo test                 # esegue i test
cargo build --release      # produce target/release/nyctalus
```

La cartella `target/` può diventare grande: `cargo clean` la svuota.

## Provare un trasferimento

Sul computer che riceve:

```sh
nyctalus ricevi --uscita ricevuto.bin --identita mia.chiave
```

Il comando stampa l'istruzione `nyctalus invia ...` completa da eseguire sull'altro computer. Contiene l'impronta del certificato e l'**indirizzo `.nyct`** del ricevitore (`--destinatario`), cioè la sua chiave pubblica: non è segreto: basta sostituire l'IP e il nome del file.

- I due programmi si accordano da soli sul segreto con la stretta di mano **Noise NK**. Il segreto non viaggia mai.
- `--identita FILE` salva la chiave del ricevitore (permessi 600): così il suo indirizzo resta lo stesso tra un avvio e l'altro. Senza questa opzione, a ogni avvio ne viene generato uno nuovo.
- `--corsie N` sceglie quante corsie parallele usare (predefinito 4).

## Licenza

AGPL-3.0-or-later. © 2026 Daniele Deplano (RedRider21).
