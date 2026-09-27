// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! `nyctalus`: programma di prova di Nyctalus Protocol.
//!
//! Trasferisce un file tra due computer su QUIC, spezzato in frammenti
//! cifrati end-to-end (livello L2 di `nyctalus-core`) e distribuito su più
//! "corsie" parallele: è la ragnatela in miniatura, tra due soli estremi e
//! ancora senza nodi intermedi.
//!
//! ```text
//! nyctalus ricevi --uscita FILE [--ascolta 0.0.0.0:4433] [--identita FILE]
//! nyctalus invia  --a IP:PORTA --impronta HEX --destinatario INDIRIZZO.nyct [--corsie 4] FILE
//! ```
//!
//! Il segreto del flusso nasce dalla stretta di mano Noise NK con la chiave
//! pubblica del destinatario: non viaggia mai e non va copiato a mano.
//!
//! Inoltre offre il primo abbozzo dell'uso "da rete" (VISIONE §8.2):
//!
//! ```text
//! nyctalus uscita --ascolta 0.0.0.0:4600
//! nyctalus avvia  --a IP:PORTA --impronta HEX [--socks 127.0.0.1:1080]
//! nyctalus nodo   --ascolta 0.0.0.0:4601
//! nyctalus prova-circuito --nodi 'a,pub;b,pub;c,pub' --messaggio "ciao"
//! ```
//!
//! `avvia` apre un proxy SOCKS5 locale che inoltra il traffico a un nodo di
//! `uscita`, il quale lo porta su Internet. ATTENZIONE: è ancora un solo
//! salto (client → uscita), quindi **non è anonimo**: l'uscita vede l'IP del
//! client. Serve a rendere la CLI usabile fin da subito; i nodi intermedi e
//! la cipolla (tappe 1b/1c) trasformeranno questo in un percorso anonimo.

mod avvia;
mod avvia_circuito;
mod circuito_rete;
mod invia;
mod nodo;
mod prova_circuito;
mod ricevi;
mod tls;
mod travaso;
mod uscita;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

pub type Risultato<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Id del flusso dati (mittente → ricevitore) usato dal programma di prova.
pub const ID_FLUSSO_DATI: u64 = 1;

const USO: &str = "\
Uso:
  nyctalus ricevi --uscita FILE [--ascolta 0.0.0.0:4433] [--identita FILE]
  nyctalus invia  --a IP:PORTA --impronta HEX --destinatario INDIRIZZO.nyct [--corsie 4] FILE
  nyctalus uscita --ascolta 0.0.0.0:4600
  nyctalus avvia  --a IP:PORTA --impronta HEX [--socks 127.0.0.1:1080]
  nyctalus nodo   --ascolta 0.0.0.0:4601 [--segreto HEX]
  nyctalus prova-circuito --nodi 'addr,pubhex;addr,pubhex;addr,pubhex' [--messaggio TESTO]
  nyctalus avvia-circuito  --nodi 'addr,pubhex;addr,pubhex;addr,pubhex' [--socks 127.0.0.1:1080]

'ricevi'/'invia': trasferimento di un file (test del livello L2).
'uscita'/'avvia': proxy SOCKS5 verso Internet (un solo salto, non ancora anonimo).
'nodo'/'prova-circuito': circuito onion a più nodi (tappa 2).
Avvia prima i nodi (stampano il pezzo per --nodi), poi 'prova-circuito'.";

#[tokio::main]
async fn main() {
    if let Err(errore) = esegui().await {
        eprintln!("errore: {errore}");
        std::process::exit(1);
    }
}

async fn esegui() -> Risultato<()> {
    let argomenti: Vec<String> = std::env::args().skip(1).collect();
    let Some((comando, resto)) = argomenti.split_first() else {
        println!("{USO}");
        return Ok(());
    };
    let (opzioni, posizionali) = leggi_opzioni(resto)?;
    match comando.as_str() {
        "ricevi" => {
            let ascolta: SocketAddr = opzione(&opzioni, "ascolta").unwrap_or("0.0.0.0:4433").parse()?;
            let uscita = PathBuf::from(obbligatoria(&opzioni, "uscita")?);
            let identita = opzione(&opzioni, "identita").map(PathBuf::from);
            ricevi::ricevi(ascolta, uscita, identita).await
        }
        "invia" => {
            let destinazione: SocketAddr = obbligatoria(&opzioni, "a")?.parse()?;
            let impronta = da_esadecimale(obbligatoria(&opzioni, "impronta")?)?;
            let destinatario = nyctalus_core::indirizzo::in_chiave(obbligatoria(&opzioni, "destinatario")?)?;
            let corsie: usize = opzione(&opzioni, "corsie").unwrap_or("4").parse()?;
            if !(1..=64).contains(&corsie) {
                return Err("--corsie deve essere tra 1 e 64".into());
            }
            let [file] = posizionali.as_slice() else {
                return Err("indicare un solo file da inviare".into());
            };
            invia::invia(destinazione, impronta, destinatario, corsie, PathBuf::from(file)).await
        }
        "uscita" => {
            let ascolta: SocketAddr = opzione(&opzioni, "ascolta").unwrap_or("0.0.0.0:4600").parse()?;
            uscita::uscita(ascolta).await
        }
        "avvia" => {
            let destinazione: SocketAddr = obbligatoria(&opzioni, "a")?.parse()?;
            let impronta = da_esadecimale(obbligatoria(&opzioni, "impronta")?)?;
            let socks: SocketAddr = opzione(&opzioni, "socks").unwrap_or("127.0.0.1:1080").parse()?;
            avvia::avvia(destinazione, impronta, socks).await
        }
        "nodo" => {
            let ascolta: SocketAddr = obbligatoria(&opzioni, "ascolta")?.parse()?;
            let segreto = match opzione(&opzioni, "segreto") {
                Some(hex) => da_esadecimale(hex)?,
                None => nyctalus_core::circuito::genera_segreto(),
            };
            nodo::nodo(ascolta, segreto).await
        }
        "prova-circuito" => {
            let nodi = leggi_nodi(obbligatoria(&opzioni, "nodi")?)?;
            let messaggio = opzione(&opzioni, "messaggio").unwrap_or("PING da Nyctalus").to_string();
            prova_circuito::prova_circuito(nodi, messaggio).await
        }
        "avvia-circuito" => {
            let nodi = leggi_nodi(obbligatoria(&opzioni, "nodi")?)?;
            let socks: SocketAddr = opzione(&opzioni, "socks").unwrap_or("127.0.0.1:1080").parse()?;
            avvia_circuito::avvia_circuito(nodi, socks).await
        }
        "aiuto" | "--help" | "-h" => {
            println!("{USO}");
            Ok(())
        }
        altro => Err(format!("comando sconosciuto: {altro}\n\n{USO}").into()),
    }
}

type Opzioni = HashMap<String, String>;

fn leggi_opzioni(argomenti: &[String]) -> Risultato<(Opzioni, Vec<String>)> {
    let mut opzioni = HashMap::new();
    let mut posizionali = Vec::new();
    let mut iter = argomenti.iter();
    while let Some(argomento) = iter.next() {
        if let Some(nome) = argomento.strip_prefix("--") {
            let valore = iter.next().ok_or_else(|| format!("manca il valore di --{nome}"))?;
            opzioni.insert(nome.to_string(), valore.clone());
        } else {
            posizionali.push(argomento.clone());
        }
    }
    Ok((opzioni, posizionali))
}

fn opzione<'a>(opzioni: &'a Opzioni, nome: &str) -> Option<&'a str> {
    opzioni.get(nome).map(String::as_str)
}

fn obbligatoria<'a>(opzioni: &'a Opzioni, nome: &str) -> Risultato<&'a str> {
    opzione(opzioni, nome).ok_or_else(|| format!("manca l'opzione obbligatoria --{nome}").into())
}

pub fn in_esadecimale(byte: &[u8]) -> String {
    byte.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn da_esadecimale(testo: &str) -> Risultato<[u8; 32]> {
    if testo.len() != 64 || !testo.is_ascii() {
        return Err("atteso un valore esadecimale di 64 caratteri".into());
    }
    let mut risultato = [0u8; 32];
    for (i, coppia) in testo.as_bytes().chunks(2).enumerate() {
        let coppia = std::str::from_utf8(coppia)?;
        risultato[i] = u8::from_str_radix(coppia, 16)
            .map_err(|_| format!("carattere non esadecimale in '{coppia}'"))?;
    }
    Ok(risultato)
}

/// Interpreta la lista dei nodi per `prova-circuito`: voci separate da ';',
/// ciascuna `indirizzo,chiavepubblica_esadecimale`, nell'ordine guard→uscita.
fn leggi_nodi(testo: &str) -> Risultato<Vec<(SocketAddr, [u8; 32])>> {
    testo
        .split(';')
        .filter(|v| !v.trim().is_empty())
        .map(|voce| {
            let (addr, hex) = voce.split_once(',').ok_or("nodo nel formato indirizzo,chiavehex")?;
            Ok((addr.trim().parse()?, da_esadecimale(hex.trim())?))
        })
        .collect()
}

/// Velocità leggibile (MB/s) per i riepiloghi.
pub fn velocita(byte: u64, secondi: f64) -> String {
    format!("{:.1} MB/s", byte as f64 / 1_000_000.0 / secondi.max(1e-9))
}
