// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! `nyctalus`: eseguibile di Nyctalus Protocol.
//!
//! I **nomi di comandi e opzioni sono in inglese** (interfaccia pubblica,
//! pronta per un pubblico internazionale); i commenti del codice restano in
//! italiano. Esempio d'uso "da rete":
//!
//! ```text
//! nyctalus authority                         # crea l'autorità dell'elenco
//! nyctalus node --listen 0.0.0.0:4601 --directory dir.txt --authority HEX
//! nyctalus directory --authority-secret HEX --entries 'addr,x,fp;...' > dir.txt
//! nyctalus start --directory dir.txt --authority HEX --socks 127.0.0.1:1080
//! ```
//!
//! `start` legge la configurazione (`~/.config/nyctalus/config` o `--config`)
//! e apre un proxy SOCKS5 che instrada il traffico attraverso la ragnatela
//! conflux (più circuiti, guard e uscita condivisi). `status` ne mostra la
//! configurazione effettiva.

mod avvia;
mod avvia_circuito;
mod cella;
mod conflux;
mod circuito_rete;
mod invia;
mod metriche;
mod monitor;
mod nodi_online;
mod nodo;
mod ponte;
mod prova_circuito;
mod ricevi;
mod tls;
mod trasporto;
mod travaso;
mod uscita;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

pub type Risultato<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Id del flusso dati (mittente → ricevitore) usato dal programma di prova.
pub const ID_FLUSSO_DATI: u64 = 1;

const USO: &str = "\
Usage:
  nyctalus start   [--config FILE] [--directory FILE] [--authority HEX] [--socks 127.0.0.1:1080] [--circuits 2] [--hops 3]
  nyctalus status  [--config FILE]
  nyctalus nodes   [--config FILE] [--directory FILE] [--authority HEX] [--probe] [--timeout 3] [--obfuscation HEX]
  nyctalus exec    [--socks ADDR] [--config FILE] -- PROGRAM [ARG...]
  nyctalus monitor [--control 127.0.0.1:1090]
  nyctalus node    --listen 0.0.0.0:4601 [--secret HEX] [--directory FILE --authority HEX] [--obfuscation HEX] [--tor-socks 127.0.0.1:9050]
  nyctalus authority
  nyctalus directory --authority-secret HEX --entries 'addr,x,fp;addr,x,fp'
  nyctalus circuit  (--directory FILE --authority HEX | --nodes 'addr,pub;...') [--socks ...] [--circuits 2 (1-8)] [--hops 3 (1-5)]
  nyctalus test-circuit (--directory FILE --authority HEX | --nodes 'addr,pub;...') [--message DEST]
  nyctalus receive --out FILE [--listen 0.0.0.0:4433] [--identity FILE]
  nyctalus send    --to IP:PORT --fingerprint HEX --recipient ADDRESS.nyct [--lanes 4] FILE
  nyctalus single-hop --to IP:PORT --fingerprint HEX [--socks 127.0.0.1:1080]
  nyctalus exit    --listen 0.0.0.0:4600

'start'/'status': SOCKS5 daemon driven by config (~/.config/nyctalus/config) and its status.
'nodes': list the signed directory; with --probe, test which nodes are reachable now (and their RTT).
'node'/'authority'/'directory': nodes and the signed directory.
'circuit': like 'start' but with nodes given explicitly (testing).
'receive'/'send': file transfer (L2 test). 'single-hop'/'exit': 1-hop proxy (test).";

#[tokio::main]
async fn main() {
    if let Err(errore) = esegui().await {
        eprintln!("error: {errore}");
        std::process::exit(1);
    }
}

async fn esegui() -> Risultato<()> {
    let argomenti: Vec<String> = std::env::args().skip(1).collect();
    let Some((comando, resto)) = argomenti.split_first() else {
        println!("{USO}");
        return Ok(());
    };
    // `exec` prende gli argomenti grezzi (le opzioni del programma lanciato non
    // devono essere interpretate da noi): gestito prima dell'analisi generica.
    if comando == "exec" {
        return esegui_exec(resto);
    }
    let (opzioni, posizionali) = leggi_opzioni(resto)?;
    match comando.as_str() {
        "receive" => {
            let ascolta: SocketAddr = opzione(&opzioni, "listen").unwrap_or("0.0.0.0:4433").parse()?;
            let uscita = PathBuf::from(obbligatoria(&opzioni, "out")?);
            let identita = opzione(&opzioni, "identity").map(PathBuf::from);
            ricevi::ricevi(ascolta, uscita, identita).await
        }
        "send" => {
            let destinazione: SocketAddr = obbligatoria(&opzioni, "to")?.parse()?;
            let impronta = da_esadecimale(obbligatoria(&opzioni, "fingerprint")?)?;
            let destinatario = nyctalus_core::indirizzo::in_chiave(obbligatoria(&opzioni, "recipient")?)?;
            let corsie: usize = opzione(&opzioni, "lanes").unwrap_or("4").parse()?;
            if !(1..=64).contains(&corsie) {
                return Err("--lanes must be between 1 and 64".into());
            }
            let [file] = posizionali.as_slice() else {
                return Err("specify a single file to send".into());
            };
            invia::invia(destinazione, impronta, destinatario, corsie, PathBuf::from(file)).await
        }
        "exit" => {
            let ascolta: SocketAddr = opzione(&opzioni, "listen").unwrap_or("0.0.0.0:4600").parse()?;
            uscita::uscita(ascolta).await
        }
        "start" => {
            let cfg = Config::carica(&opzioni)?;
            println!("Configuration: {}", cfg.percorso.display());
            let nodi = nodi_da_elenco(&cfg.elenco, cfg.autorita)?;
            avvia_circuito::avvia_circuito(nodi, cfg.circuiti, cfg.salti, cfg.socks, cfg.controllo, cfg.offuscamento).await
        }
        "status" => {
            let cfg = Config::carica(&opzioni)?;
            cfg.stampa();
            Ok(())
        }
        "nodes" => {
            // Elenco preso dalla configurazione se non passato a riga di comando.
            let file = leggi_file_config(&match opzione(&opzioni, "config") {
                Some(p) => PathBuf::from(p),
                None => percorso_config_predefinito(),
            });
            let val = |k: &str| opzione(&opzioni, k).map(String::from).or_else(|| file.get(k).cloned());
            let elenco = val("directory").ok_or("missing 'directory' (in config or --directory)")?;
            let autorita = da_esadecimale(&val("authority").ok_or("missing 'authority' (in config or --authority)")?)?;
            let offusca = da_esadecimale_opt(val("obfuscation").as_deref())?;
            let sonda = opzioni.contains_key("probe");
            let scadenza = std::time::Duration::from_secs_f32(
                opzione(&opzioni, "timeout").map(str::parse).transpose()?.unwrap_or(3.0),
            );
            nodi_online::elenca(&elenco, autorita, offusca, sonda, scadenza).await
        }
        "monitor" => {
            let controllo: SocketAddr = opzione(&opzioni, "control").unwrap_or("127.0.0.1:1090").parse()?;
            monitor::monitor(controllo).await
        }
        "single-hop" => {
            let destinazione: SocketAddr = obbligatoria(&opzioni, "to")?.parse()?;
            let impronta = da_esadecimale(obbligatoria(&opzioni, "fingerprint")?)?;
            let socks: SocketAddr = opzione(&opzioni, "socks").unwrap_or("127.0.0.1:1080").parse()?;
            avvia::avvia(destinazione, impronta, socks).await
        }
        "node" => {
            let ascolta: SocketAddr = obbligatoria(&opzioni, "listen")?.parse()?;
            let segreto = match opzione(&opzioni, "secret") {
                Some(hex) => da_esadecimale(hex)?,
                None => nyctalus_core::circuito::genera_segreto(),
            };
            let auth = match (opzione(&opzioni, "directory"), opzione(&opzioni, "authority")) {
                (Some(e), Some(a)) => {
                    Some(nodo::Autenticazione { elenco: PathBuf::from(e), autorita: da_esadecimale(a)? })
                }
                _ => None,
            };
            let offusca = da_esadecimale_opt(opzione(&opzioni, "obfuscation"))?;
            if let Some(ts) = opzione(&opzioni, "tor-socks") {
                ponte::imposta_tor_socks(ts.parse()?);
            }
            nodo::nodo(ascolta, segreto, auth, offusca).await
        }
        "authority" => {
            let (segreto, pubblica) = nyctalus_core::elenco::genera_autorita();
            println!("Directory authority (keep the secret safe):");
            println!("  secret:   {}", in_esadecimale(&segreto));
            println!("  public:   {}", in_esadecimale(&pubblica));
            Ok(())
        }
        "directory" => {
            let segreto = da_esadecimale(obbligatoria(&opzioni, "authority-secret")?)?;
            let voci = leggi_voci_elenco(obbligatoria(&opzioni, "entries")?)?;
            print!("{}", nyctalus_core::elenco::firma_elenco(&voci, &segreto));
            Ok(())
        }
        "test-circuit" => {
            let nodi = carica_nodi(&opzioni)?;
            let messaggio = opzione(&opzioni, "message").unwrap_or("example.com:80").to_string();
            prova_circuito::prova_circuito(nodi, messaggio).await
        }
        "circuit" => {
            let nodi = carica_nodi(&opzioni)?;
            let socks: SocketAddr = opzione(&opzioni, "socks").unwrap_or("127.0.0.1:1080").parse()?;
            let circuiti: usize = opzione(&opzioni, "circuits").map(str::parse).transpose()?.unwrap_or(2);
            let salti: usize = opzione(&opzioni, "hops").map(str::parse).transpose()?.unwrap_or(3);
            let controllo: SocketAddr = opzione(&opzioni, "control").unwrap_or("127.0.0.1:1090").parse()?;
            let offusca = da_esadecimale_opt(opzione(&opzioni, "obfuscation"))?;
            avvia_circuito::avvia_circuito(nodi, circuiti, salti, socks, controllo, offusca).await
        }
        "help" | "--help" | "-h" => {
            println!("{USO}");
            Ok(())
        }
        altro => Err(format!("unknown command: {altro}\n\n{USO}").into()),
    }
}

type Opzioni = HashMap<String, String>;

/// Opzioni senza valore (flag sì/no): se presenti valgono "true".
const FLAG_BOOLEANI: &[&str] = &["probe"];

fn leggi_opzioni(argomenti: &[String]) -> Risultato<(Opzioni, Vec<String>)> {
    let mut opzioni = HashMap::new();
    let mut posizionali = Vec::new();
    let mut iter = argomenti.iter();
    while let Some(argomento) = iter.next() {
        if let Some(nome) = argomento.strip_prefix("--") {
            if FLAG_BOOLEANI.contains(&nome) {
                opzioni.insert(nome.to_string(), "true".to_string());
                continue;
            }
            let valore = iter.next().ok_or_else(|| format!("missing value for --{nome}"))?;
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
    opzione(opzioni, nome).ok_or_else(|| format!("missing required option --{nome}").into())
}

pub fn in_esadecimale(byte: &[u8]) -> String {
    byte.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn da_esadecimale(testo: &str) -> Risultato<[u8; 32]> {
    if testo.len() != 64 || !testo.is_ascii() {
        return Err("expected a 64-character hex value".into());
    }
    let mut risultato = [0u8; 32];
    for (i, coppia) in testo.as_bytes().chunks(2).enumerate() {
        let coppia = std::str::from_utf8(coppia)?;
        risultato[i] = u8::from_str_radix(coppia, 16)
            .map_err(|_| format!("non-hex character in '{coppia}'"))?;
    }
    Ok(risultato)
}

/// Carica i nodi da un elenco firmato (file + chiave pubblica dell'autorità).
fn nodi_da_elenco(percorso: &str, autorita: [u8; 32]) -> Risultato<Vec<avvia_circuito::NodoRete>> {
    let testo = std::fs::read_to_string(percorso)?;
    let voci = nyctalus_core::elenco::verifica_elenco(&testo, &autorita)?;
    voci.into_iter().map(|v| Ok((v.indirizzo.parse()?, v.x25519, Some(v.impronta_tls)))).collect()
}

/// Configurazione del demone: valori da riga di comando, poi dal file
/// `~/.config/nyctalus/config` (chiave = valore), infine i default.
struct Config {
    percorso: PathBuf,
    elenco: String,
    autorita: [u8; 32],
    socks: SocketAddr,
    circuiti: usize,
    salti: usize,
    controllo: SocketAddr,
    offuscamento: Option<[u8; 32]>,
}

impl Config {
    fn carica(opzioni: &Opzioni) -> Risultato<Self> {
        let percorso = match opzione(opzioni, "config") {
            Some(p) => PathBuf::from(p),
            None => percorso_config_predefinito(),
        };
        let file = leggi_file_config(&percorso);
        let val = |k: &str| opzione(opzioni, k).map(String::from).or_else(|| file.get(k).cloned());
        let elenco = val("directory").ok_or("missing 'directory' (in config or --directory)")?;
        let autorita = da_esadecimale(&val("authority").ok_or("missing 'authority' (in config or --authority)")?)?;
        let socks: SocketAddr = val("socks").unwrap_or_else(|| "127.0.0.1:1080".into()).parse()?;
        let circuiti: usize = val("circuits").map(|s| s.parse()).transpose()?.unwrap_or(2);
        let salti: usize = val("hops").map(|s| s.parse()).transpose()?.unwrap_or(3);
        let controllo: SocketAddr = val("control").unwrap_or_else(|| "127.0.0.1:1090".into()).parse()?;
        let offuscamento = da_esadecimale_opt(val("obfuscation").as_deref())?;
        Ok(Self { percorso, elenco, autorita, socks, circuiti, salti, controllo, offuscamento })
    }

    fn stampa(&self) {
        println!("Configuration status ({}):", self.percorso.display());
        println!("  directory: {}", self.elenco);
        println!("  authority: {}", in_esadecimale(&self.autorita));
        println!("  socks:     {}", self.socks);
        println!("  circuits:  {}", self.circuiti);
        println!("  hops:      {}", self.salti);
        match nodi_da_elenco(&self.elenco, self.autorita) {
            Ok(nodi) => println!("  directory valid: {} nodes", nodi.len()),
            Err(e) => println!("  directory INVALID: {e}"),
        }
    }
}

/// Percorso predefinito del file di configurazione.
fn percorso_config_predefinito() -> PathBuf {
    let base = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join(".config/nyctalus/config")
}

/// `exec` (stile torsocks): esegue un programma instradandone il traffico sul
/// proxy SOCKS tramite le variabili d'ambiente proxy. Funziona con i programmi
/// che rispettano `ALL_PROXY`/`http_proxy` (curl, git, wget…); l'intercettazione
/// totale via LD_PRELOAD è un possibile sviluppo futuro.
/// Uso: `nyctalus exec [--socks ADDR] [--config FILE] -- PROGRAMMA [ARG…]`.
fn esegui_exec(resto: &[String]) -> Risultato<()> {
    let mut socks: Option<String> = None;
    let mut config: Option<String> = None;
    let mut i = 0;
    while i < resto.len() {
        match resto[i].as_str() {
            "--socks" => {
                socks = resto.get(i + 1).cloned();
                i += 2;
            }
            "--config" => {
                config = resto.get(i + 1).cloned();
                i += 2;
            }
            "--" => {
                i += 1;
                break;
            }
            s if s.starts_with("--") => {
                return Err(format!("unknown option for exec: {s}; put -- before the program").into());
            }
            _ => break,
        }
    }
    let comando = &resto[i..];
    let [programma, argomenti @ ..] = comando else {
        return Err("exec: specify the program to run (e.g. exec -- curl https://example.com)".into());
    };

    let socks = socks.unwrap_or_else(|| {
        let percorso = config.map(PathBuf::from).unwrap_or_else(percorso_config_predefinito);
        leggi_file_config(&percorso).get("socks").cloned().unwrap_or_else(|| "127.0.0.1:1080".into())
    });
    let proxy = format!("socks5h://{socks}");

    let stato = std::process::Command::new(programma)
        .args(argomenti)
        .envs([
            ("ALL_PROXY", proxy.as_str()),
            ("all_proxy", proxy.as_str()),
            ("http_proxy", proxy.as_str()),
            ("https_proxy", proxy.as_str()),
            ("HTTP_PROXY", proxy.as_str()),
            ("HTTPS_PROXY", proxy.as_str()),
        ])
        .status()
        .map_err(|e| format!("exec: cannot run {programma}: {e}"))?;
    std::process::exit(stato.code().unwrap_or(1));
}

/// Legge un file `chiave = valore` (righe vuote e `#` ignorate). Vuoto se assente.
fn leggi_file_config(percorso: &std::path::Path) -> HashMap<String, String> {
    let mut mappa = HashMap::new();
    if let Ok(testo) = std::fs::read_to_string(percorso) {
        for riga in testo.lines() {
            let riga = riga.split('#').next().unwrap_or("").trim();
            if let Some((k, v)) = riga.split_once('=') {
                mappa.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    mappa
}

/// Carica i nodi del percorso: da elenco firmato (`--elenco` + `--autorita`,
/// con impronte TLS per autenticare) oppure da `--nodi` (prototipo, senza
/// autenticazione). Ordine: guard → … → uscita.
fn carica_nodi(opzioni: &Opzioni) -> Risultato<Vec<avvia_circuito::NodoRete>> {
    if let Some(percorso) = opzione(opzioni, "directory") {
        let autorita = da_esadecimale(obbligatoria(opzioni, "authority")?)?;
        return nodi_da_elenco(percorso, autorita);
    }
    let nodi = obbligatoria(opzioni, "nodes")?;
    nodi
        .split(';')
        .filter(|v| !v.trim().is_empty())
        .map(|voce| {
            let (addr, hex) = voce.split_once(',').ok_or("node in the form address,pubkeyhex")?;
            Ok((addr.trim().parse()?, da_esadecimale(hex.trim())?, None))
        })
        .collect()
}

/// Voci per il comando `elenco`: `indirizzo,x25519hex,improntahex` separate da ';'.
fn leggi_voci_elenco(testo: &str) -> Risultato<Vec<nyctalus_core::elenco::VoceElenco>> {
    testo
        .split(';')
        .filter(|v| !v.trim().is_empty())
        .map(|voce| {
            let campi: Vec<&str> = voce.split(',').map(str::trim).collect();
            let [addr, x, fp] = campi.as_slice() else {
                return Err("entry in the form address,x25519hex,fingerprinthex".into());
            };
            Ok(nyctalus_core::elenco::VoceElenco {
                indirizzo: (*addr).to_string(),
                x25519: da_esadecimale(x)?,
                impronta_tls: da_esadecimale(fp)?,
            })
        })
        .collect()
}

/// Chiave esadecimale opzionale (32 byte / 64 caratteri).
fn da_esadecimale_opt(hex: Option<&str>) -> Risultato<Option<[u8; 32]>> {
    hex.map(da_esadecimale).transpose()
}

/// Velocità leggibile (MB/s) per i riepiloghi.
pub fn velocita(byte: u64, secondi: f64) -> String {
    format!("{:.1} MB/s", byte as f64 / 1_000_000.0 / secondi.max(1e-9))
}
