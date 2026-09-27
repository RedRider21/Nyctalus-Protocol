//! `nyctalus`: programma di prova di Nyctalus Protocol.
//!
//! Trasferisce un file tra due computer su QUIC, spezzato in frammenti
//! cifrati end-to-end (livello L2 di `nyctalus-core`) e distribuito su più
//! "corsie" parallele: è la ragnatela in miniatura, tra due soli estremi e
//! ancora senza nodi intermedi.
//!
//! ```text
//! nyctalus ricevi --uscita FILE [--ascolta 0.0.0.0:4433]
//! nyctalus invia  --a IP:PORTA --impronta HEX --segreto HEX [--corsie 4] FILE
//! ```
//!
//! SOLO PROVA: il segreto del flusso viene generato dal ricevitore e passato
//! a mano al mittente; nella rete vera arriverà dalla stretta di mano Noise.

mod invia;
mod ricevi;
mod tls;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

pub type Risultato<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Id del flusso dati (mittente → ricevitore) usato dal programma di prova.
pub const ID_FLUSSO_DATI: u64 = 1;

const USO: &str = "\
Uso:
  nyctalus ricevi --uscita FILE [--ascolta 0.0.0.0:4433]
  nyctalus invia  --a IP:PORTA --impronta HEX --segreto HEX [--corsie 4] FILE

Avvia prima 'ricevi': stampa il comando 'invia' completo da usare sull'altro computer.";

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
            ricevi::ricevi(ascolta, uscita).await
        }
        "invia" => {
            let destinazione: SocketAddr = obbligatoria(&opzioni, "a")?.parse()?;
            let impronta = da_esadecimale(obbligatoria(&opzioni, "impronta")?)?;
            let segreto = da_esadecimale(obbligatoria(&opzioni, "segreto")?)?;
            let corsie: usize = opzione(&opzioni, "corsie").unwrap_or("4").parse()?;
            if !(1..=64).contains(&corsie) {
                return Err("--corsie deve essere tra 1 e 64".into());
            }
            let [file] = posizionali.as_slice() else {
                return Err("indicare un solo file da inviare".into());
            };
            invia::invia(destinazione, impronta, segreto, corsie, PathBuf::from(file)).await
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

fn da_esadecimale(testo: &str) -> Risultato<[u8; 32]> {
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

/// Velocità leggibile (MB/s) per i riepiloghi.
pub fn velocita(byte: u64, secondi: f64) -> String {
    format!("{:.1} MB/s", byte as f64 / 1_000_000.0 / secondi.max(1e-9))
}
