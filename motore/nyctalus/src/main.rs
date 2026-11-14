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

#![forbid(unsafe_code)] // nessun blocco `unsafe` nel motore (piano di sicurezza, punto 7)

mod avvia;
mod avvia_circuito;
mod cella;
mod codifica;
mod chat;
mod conflux;
mod costante;
mod ripresa;
mod cassaforte;
mod raffica;
mod circuito_rete;
mod invia;
mod incontro;
mod metriche;
mod monitor;
mod nodi_online;
mod nodo;
mod percorsi;
mod ponte;
mod prova_circuito;
mod ricevi;
mod tls;
mod tcp;
mod trasporto;
mod travaso;
mod webrtc;
mod uscita;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

pub type Risultato<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

/// Id del flusso dati (mittente → ricevitore) usato dal programma di prova.
pub const ID_FLUSSO_DATI: u64 = 1;

const USO: &str = "\
Usage:
  nyctalus start   [--config FILE] [--directory FILE] [--authority HEX] [--socks 127.0.0.1:1080] [--circuits 2] [--hops 3] [--cover 0] [--bridge LINE]
  nyctalus status  [--config FILE]
  nyctalus nodes   [--config FILE] [--directory FILE] [--authority HEX] [--probe] [--timeout 3] [--obfuscation HEX]
  nyctalus exec    [--socks ADDR] [--config FILE] -- PROGRAM [ARG...]
  nyctalus monitor [--control 127.0.0.1:1090]
  nyctalus node    --listen 0.0.0.0:4601 [--data DIR] [--announce PUBLIC_IP:PORT] [--secret HEX] [--directory FILE --authority HEX] [--obfuscation HEX] [--tor-socks 127.0.0.1:9050] [--cover 0] [--hidden-bridge] [--middle-only | --no-exit] [--max-rate MBIT]
  nyctalus authority
  nyctalus directory --authority-secret HEX --entries 'addr,x,fp;addr,x,fp'
  nyctalus circuit  (--directory FILE --authority HEX | --nodes 'addr,pub;...') [--socks ...] [--circuits 2 (1-8)] [--hops 3 (1-5)] [--cover 0] [--exits 1 (1-4)] [--constant-rate N [--constant-always]]
  nyctalus test-circuit (--directory FILE --authority HEX | --nodes 'addr,pub;...') [--message DEST]
  nyctalus site    (--dir ./folder | --backend IP:PORT) [--directory FILE --authority HEX | --rendezvous IP:PORT --fingerprint HEX] [--identity FILE | --identity-chat 127.0.0.1:1091] [--hops 3] [--pool 4]
  nyctalus address --identity FILE      (a site's key, created if missing, and its .nyct address)
  nyctalus browse  ADDRESS.nyct [--directory FILE --authority HEX | --rendezvous IP:PORT --fingerprint HEX] [--path /] [--hops 3]
  nyctalus chat    [--data DIR] [--identity FILE] [--backend 127.0.0.1:1082] [--control 127.0.0.1:1091] [--socks 127.0.0.1:1080] [--max-file 200] [--control-token SECRET] [--rubrica ADDR.nyct] [--casella ADDR.nyct]
  nyctalus receive --out FILE [--listen 0.0.0.0:4433] [--identity FILE]
  nyctalus send    --to IP:PORT --fingerprint HEX --recipient ADDRESS.nyct [--lanes 4] FILE
  nyctalus single-hop --to IP:PORT --fingerprint HEX [--socks 127.0.0.1:1080]
  nyctalus exit    --listen 0.0.0.0:4600

'start'/'status': SOCKS5 daemon driven by config (~/.config/nyctalus/config) and its status.
'nodes': list the signed directory; with --probe, test which nodes are reachable now (and their RTT).
'node'/'authority'/'directory': nodes and the signed directory.
'circuit': like 'start' but with nodes given explicitly (testing).
'--mimic [--mimic-name HOST]' (all, or env NYCTALUS_MIMETICO=1): HTTP/3 form: plain QUIC with ALPN h3 that looks like
  a web site; the network key (--obfuscation) becomes a tag inside TLS; probers get an HTTP/3 404 page.
'--webrtc' (all, or env NYCTALUS_WEBRTC=1): video-call form: traffic looks like a WebRTC call (STUN + SRTP
  packets); the network key authenticates each packet (probers get nothing). Alternative to --mimic.
'--exits N' (circuit/start, or env NYCTALUS_USCITE, or 'exits = N' in the config): up to 4 different exits; each
  site keeps its own exit, new sites go to the least busy one (each exit sees fewer of your sites; measured: no speed gain).
'--constant-rate N [--constant-always]' (circuit/start, or env NYCTALUS_COSTANTE=N, NYCTALUS_COSTANTE_SEMPRE=1): constant
  cover, N cells/s each way (real or dummy) while in use (or always): the line looks flat. Costs bandwidth, caps speed.
'--no-exit' (node): never an Internet exit (others' browsing never leaves from this address), still a guard, middle and
  rendezvous node, so chat, files and .nyct sites keep working. In the directory: role «noexit» (uscita=no). With no exit
  in the network the tunnel still starts: .nyct sites work, Internet connections get a SOCKS error.
'--hidden-bridge' (node): a private entry NOT in the directory, with its own camouflage key; it prints the
  bridge line. '--bridge LINE[;LINE]' (start/circuit/site/browse, or env NYCTALUS_PONTI, or 'bridge = ' in the config): always enter
  the network through those bridges.
'site'/'browse': host and visit an internal .nyct site through a rendezvous node (a node reached with --rendezvous/--fingerprint).
'chat': metadata-free .nyct chat engine (needs 'start' for the SOCKS and a 'site --backend' pointing at its backend port); a GTK UI talks to it on the control port.
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
    // forma HTTP/3 (trasporto mimetico): da opzione o da ambiente, prima di ogni collegamento
    let mimic_amb = std::env::var("NYCTALUS_MIMETICO").is_ok_and(|v| v == "1" || v == "h3");
    if opzione(&opzioni, "mimic").is_some() || mimic_amb {
        let nome = opzione(&opzioni, "mimic-name").map(String::from)
            .or_else(|| std::env::var("NYCTALUS_NOME_MIMETICO").ok()).unwrap_or_default();
        if !nome.is_empty() && !nome.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-') {
            return Err("--mimic-name: a host name (letters, digits, dots, hyphens)".into());
        }
        trasporto::imposta_mimetico(&nome);
    }
    // forma «videochiamata» WebRTC (trasporto mimetico alternativo): STUN + pacchetti SRTP
    let webrtc_amb = std::env::var("NYCTALUS_WEBRTC").is_ok_and(|v| v == "1");
    if opzione(&opzioni, "webrtc").is_some() || webrtc_amb {
        if trasporto::mimetico() {
            return Err("--webrtc and --mimic are two different forms: pick one".into());
        }
        webrtc::imposta();
    }
    // ponti nascosti per entrare nella rete (start li legge anche dalla configurazione)
    // (meglio la variabile NYCTALUS_PONTI: la riga contiene una chiave e la riga di comando la
    // vedono tutti gli utenti del computer con `ps`, l'ambiente solo il proprietario)
    let da_ambiente = std::env::var("NYCTALUS_PONTI").ok().filter(|p| !p.trim().is_empty());
    if let Some(p) = opzione(&opzioni, "bridge").map(String::from).or(da_ambiente) {
        percorsi::imposta_ponti(leggi_ponti(&p)?);
    }
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
            if let (Some(p), true) = (&cfg.ponti, percorsi::ponti().is_empty()) {
                percorsi::imposta_ponti(leggi_ponti(p)?);
            }
            let nodi = nodi_da_elenco(&cfg.elenco, cfg.autorita)?;
            avvia_circuito::avvia_circuito(nodi, cfg.circuiti, cfg.salti, cfg.socks, cfg.controllo, cfg.offuscamento, cfg.copertura).await
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
        "address" => {
            // la chiave di un sito (creata se manca) e il suo indirizzo, senza andare in rete
            let file = PathBuf::from(obbligatoria(&opzioni, "identity")?);
            let identita = incontro::carica_o_crea_identita(&file)?;
            println!("{}", nyctalus_core::indirizzo::da_chiave(&identita.pubblica()));
            Ok(())
        }
        "site" => {
            let modo = match (opzione(&opzioni, "dir"), opzione(&opzioni, "backend")) {
                (Some(d), None) => incontro::ModoServizio::Cartella(PathBuf::from(d)),
                (None, Some(b)) => incontro::ModoServizio::Backend(b.parse()?),
                (Some(_), Some(_)) => return Err("use either --dir or --backend, not both".into()),
                (None, None) => return Err("missing --dir FOLDER (or --backend IP:PORT)".into()),
            };
            let identita = opzione(&opzioni, "identity").map(PathBuf::from);
            let offusca = da_esadecimale_opt(opzione(&opzioni, "obfuscation"))?;
            let manuale = manuale_rendezvous(&opzioni)?;
            let elenco = if manuale.is_some() { Vec::new() } else { carica_punti_incontro(&opzioni)? };
            let pool: usize = opzione(&opzioni, "pool").map(str::parse).transpose()?.unwrap_or(4);
            // `--identity-chat ADDR`: l'identità la dà il motore della chat (dati cifrati: niente file in chiaro)
            let identita_chat: Option<SocketAddr> = opzione(&opzioni, "identity-chat").map(str::parse).transpose()?;
            incontro::site(modo, identita, identita_chat, elenco, manuale, offusca, salti_incontro(&opzioni)?, pool).await
        }
        "browse" => {
            let indirizzo_nyct = match opzione(&opzioni, "address") {
                Some(a) => a,
                None => posizionali.first().map(String::as_str).ok_or("missing .nyct address")?,
            };
            let percorso = opzione(&opzioni, "path").unwrap_or("/");
            let offusca = da_esadecimale_opt(opzione(&opzioni, "obfuscation"))?;
            let manuale = manuale_rendezvous(&opzioni)?;
            let elenco = if manuale.is_some() { Vec::new() } else { carica_punti_incontro(&opzioni)? };
            incontro::browse(indirizzo_nyct, percorso, elenco, manuale, offusca, salti_incontro(&opzioni)?).await
        }
        "single-hop" => {
            let destinazione: SocketAddr = obbligatoria(&opzioni, "to")?.parse()?;
            let impronta = da_esadecimale(obbligatoria(&opzioni, "fingerprint")?)?;
            let socks: SocketAddr = opzione(&opzioni, "socks").unwrap_or("127.0.0.1:1080").parse()?;
            avvia::avvia(destinazione, impronta, socks).await
        }
        "node" => {
            let ascolta: SocketAddr = obbligatoria(&opzioni, "listen")?.parse()?;
            // --data DIR: segreto e certificato salvati lì la prima volta e riusati
            // dopo, così la riga del nodo nell'elenco firmato non cambia ai riavvii
            let dati = opzione(&opzioni, "data").map(PathBuf::from);
            let segreto = match (opzione(&opzioni, "secret"), &dati) {
                (Some(hex), _) => da_esadecimale(hex)?,
                (None, Some(d)) => match std::fs::read_to_string(d.join("segreto")) {
                    Ok(t) => da_esadecimale(t.trim())?,
                    Err(_) => {
                        let s = nyctalus_core::circuito::genera_segreto();
                        std::fs::create_dir_all(d)?;
                        tls::scrivi_privato(&d.join("segreto"), in_esadecimale(&s).as_bytes())?;
                        s
                    }
                },
                (None, None) => nyctalus_core::circuito::genera_segreto(),
            };
            let contorno = nodo::Contorno {
                dati,
                annuncia: opzione(&opzioni, "announce").map(String::from),
                ponte: opzione(&opzioni, "hidden-bridge").is_some(),
                solo_intermedio: opzione(&opzioni, "middle-only").is_some(),
                senza_uscita: opzione(&opzioni, "no-exit").is_some(),
                tetto_mbit: opzione(&opzioni, "max-rate").map(str::parse::<f64>).transpose()?.unwrap_or(0.0),
            };
            if !(0.0..=100_000.0).contains(&contorno.tetto_mbit) {
                return Err("--max-rate: Mbit/s between 0 (no cap) and 100000".into());
            }
            let auth = match (opzione(&opzioni, "directory"), opzione(&opzioni, "authority")) {
                (Some(e), Some(a)) => {
                    Some(nodo::Autenticazione { elenco: PathBuf::from(e), autorita: da_esadecimale(a)? })
                }
                _ => None,
            };
            let offusca = da_esadecimale_opt(opzione(&opzioni, "obfuscation"))?;
            let copertura: f64 = opzione(&opzioni, "cover").map(str::parse).transpose()?.unwrap_or(0.0);
            if let Some(ts) = opzione(&opzioni, "tor-socks") {
                ponte::imposta_tor_socks(ts.parse()?);
            }
            nodo::nodo(ascolta, segreto, auth, offusca, copertura, contorno).await
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
        "simulate-paths" => {
            // PIANO-ANONIMATO, punto K: con l'elenco vero, quanto spesso un avversario (alcune
            // famiglie) sta sia all'entrata sia all'uscita, con la scelta vecchia e quella attuale
            let nodi = carica_nodi(&opzioni)?;
            let avversari: Vec<String> = obbligatoria(&opzioni, "adversary")?.split(',').map(|s| s.trim().to_string()).collect();
            let utenti: usize = opzione(&opzioni, "users").map(str::parse).transpose()?.unwrap_or(1000);
            let giorni: u64 = opzione(&opzioni, "days").map(str::parse).transpose()?.unwrap_or(90);
            let al_giorno: u64 = opzione(&opzioni, "circuits-per-day").map(str::parse).transpose()?.unwrap_or(20);
            let avv = |n: &avvia_circuito::NodoRete| percorsi::famiglia_di(&n.1).is_some_and(|f| avversari.contains(&f));
            let quanti = nodi.iter().filter(|n| avv(n)).count();
            println!("nodes: {} · adversary families: {} ({} nodes) · {} users × {} days × {} circuits/day",
                     nodi.len(), avversari.join(","), quanti, utenti, giorni, al_giorno);
            println!("{:<14} {:>12} {:>16} {:>18}", "strategy", "linked %", "users linked %", "users IP seen %");
            for (nome, st) in [("old", percorsi::Strategia::Vecchia), ("no-guard", percorsi::Strategia::SenzaGuardia),
                               ("current", percorsi::Strategia::Attuale)] {
                let e = percorsi::simula(&nodi, &avv, st, utenti, giorni, al_giorno)?;
                let pc = |a: f64, b: f64| if b > 0.0 { 100.0 * a / b } else { 0.0 };
                println!("{:<14} {:>11.3}% {:>15.2}% {:>17.2}%", nome,
                         pc(e.compromessi as f64, e.circuiti as f64),
                         pc(e.utenti_colpiti as f64, e.utenti as f64),
                         pc(e.utenti_entrata_vista as f64, e.utenti as f64));
            }
            Ok(())
        }
        "circuit" => {
            let nodi = carica_nodi(&opzioni)?;
            let socks: SocketAddr = opzione(&opzioni, "socks").unwrap_or("127.0.0.1:1080").parse()?;
            let circuiti: usize = opzione(&opzioni, "circuits").map(str::parse).transpose()?.unwrap_or(2);
            let salti: usize = opzione(&opzioni, "hops").map(str::parse).transpose()?.unwrap_or(3);
            let controllo: SocketAddr = opzione(&opzioni, "control").unwrap_or("127.0.0.1:1090").parse()?;
            let offusca = da_esadecimale_opt(opzione(&opzioni, "obfuscation"))?;
            let copertura: f64 = opzione(&opzioni, "cover").map(str::parse).transpose()?.unwrap_or(0.0);
            if let Some(c) = opzione(&opzioni, "constant-rate") {
                let n: u32 = c.parse().map_err(|_| "--constant-rate: cells per second (5-2000)")?;
                costante::imposta(n, opzioni.contains_key("constant-always"));
            }
            if let Some(u) = opzione(&opzioni, "exits") {
                let n: usize = u.parse().map_err(|_| "--exits: a number 1-4")?;
                if !(1..=avvia_circuito::USCITE_MAX).contains(&n) {
                    return Err("--exits: a number 1-4".into());
                }
                avvia_circuito::imposta_uscite(n);
            }
            avvia_circuito::avvia_circuito(nodi, circuiti, salti, socks, controllo, offusca, copertura).await
        }
        "chat" => {
            let base = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
                .join(".nyctalus-chat");
            let dati = opzione(&opzioni, "data").map(PathBuf::from).unwrap_or(base);
            let identita = opzione(&opzioni, "identity")
                .map(PathBuf::from)
                .unwrap_or_else(|| dati.join("identita"));
            let backend: SocketAddr = opzione(&opzioni, "backend").unwrap_or("127.0.0.1:1082").parse()?;
            let controllo: SocketAddr = opzione(&opzioni, "control").unwrap_or("127.0.0.1:1091").parse()?;
            let socks: SocketAddr = opzione(&opzioni, "socks").unwrap_or("127.0.0.1:1080").parse()?;
            let max_mb: u64 = opzione(&opzioni, "max-file").map(str::parse).transpose()?.unwrap_or(200);
            if !(1..=4096).contains(&max_mb) {
                return Err("--max-file must be between 1 and 4096 (MB)".into());
            }
            // segreto della porta di controllo: da opzione o (meglio, non compare in `ps`) da variabile
            let segreto = opzione(&opzioni, "control-token")
                .map(String::from)
                .or_else(|| std::env::var("NYCTALUS_CHAT_TOKEN").ok())
                .filter(|t| !t.is_empty());
            if let Some(t) = &segreto {
                if t.len() < 16 || !t.chars().all(|c| c.is_ascii_alphanumeric()) {
                    return Err("--control-token: at least 16 alphanumeric characters".into());
                }
            }
            // rubrica e casella della rete (dal codice di rete): ritiro e deposito della posta
            let rubrica = opzione(&opzioni, "rubrica").map(String::from);
            let casella = opzione(&opzioni, "casella").map(String::from);
            for a in rubrica.iter().chain(casella.iter()) {
                nyctalus_core::indirizzo::in_chiave(a).map_err(|_| format!("not a valid .nyct address: {a}"))?;
            }
            chat::chat(identita, backend, controllo, socks, dati, max_mb * 1024 * 1024, segreto, rubrica, casella).await
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
const FLAG_BOOLEANI: &[&str] = &["probe", "hidden-bridge", "mimic", "webrtc", "middle-only", "no-exit", "constant-always"];

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
    // un nodo col nome che ora non si risolve (es. casa spenta) si salta: la rete funziona con gli altri
    let mut nodi = Vec::new();
    let mut famiglie = Vec::new();
    let mut bande = Vec::new();
    let mut intermedi = Vec::new();
    let mut guardie = Vec::new();
    let mut senza_uscita = Vec::new();
    for v in voci {
        match circuito_rete::risolvi_voce(&v.indirizzo, &v.x25519) {
            Ok(a) => {
                nodi.push((a, v.x25519, Some(v.impronta_tls)));
                if let Some(f) = v.famiglia {
                    famiglie.push((v.x25519, f));
                }
                if let Some(b) = v.banda {
                    bande.push((v.x25519, b));
                }
                if v.intermedio {
                    intermedi.push(v.x25519);
                }
                if v.senza_uscita {
                    senza_uscita.push(v.x25519);
                }
                if v.guardia {
                    guardie.push(v.x25519);
                }
            }
            Err(e) => eprintln!("nodo saltato: {e}"),
        }
    }
    if nodi.is_empty() {
        return Err("nessun nodo raggiungibile nell'elenco".into());
    }
    // la rete (per ricordare la guardia di ciascuna) e le famiglie dichiarate dal gestore
    percorsi::imposta_rete(&autorita, famiglie);
    percorsi::imposta_bande(bande);
    percorsi::imposta_intermedi(intermedi);
    percorsi::imposta_guardie_promosse(guardie);
    percorsi::imposta_senza_uscita(senza_uscita);
    Ok(nodi)
}

/// Override manuale del punto d'incontro (`--rendezvous` + `--fingerprint`): se
/// dati, si usa quel nodo in modo diretto (un salto) invece della scelta
/// automatica con circuito. La chiave X25519 è ignota (a zero): niente circuito.
fn manuale_rendezvous(opzioni: &Opzioni) -> Risultato<Option<incontro::Punto>> {
    match (opzione(opzioni, "rendezvous"), opzione(opzioni, "fingerprint")) {
        (Some(r), Some(fp)) => Ok(Some((r.parse()?, [0u8; 32], Some(da_esadecimale(fp)?)))),
        (Some(_), None) => Err("--rendezvous requires --fingerprint".into()),
        _ => Ok(None),
    }
}

/// Nodi dell'elenco firmato (indirizzo + chiave X25519 + impronta TLS), presi da
/// `--directory`/`--authority` o dalla configurazione: candidati a punto
/// d'incontro e tappe dei circuiti verso di esso.
fn carica_punti_incontro(opzioni: &Opzioni) -> Risultato<Vec<incontro::Punto>> {
    let file = leggi_file_config(&match opzione(opzioni, "config") {
        Some(p) => PathBuf::from(p),
        None => percorso_config_predefinito(),
    });
    let val = |k: &str| opzione(opzioni, k).map(String::from).or_else(|| file.get(k).cloned());
    let elenco = val("directory").ok_or("missing 'directory' (in config or --directory)")?;
    let autorita = da_esadecimale(&val("authority").ok_or("missing 'authority' (in config or --authority)")?)?;
    // punti d'incontro: mai i nodi «solo intermedi» (PC degli utenti, che si spengono)
    Ok(percorsi::pieni(&nodi_da_elenco(&elenco, autorita)?))
}

/// Numero di salti del circuito verso il punto d'incontro (`--hops`, default 3).
fn salti_incontro(opzioni: &Opzioni) -> Risultato<usize> {
    Ok(opzione(opzioni, "hops").map(str::parse).transpose()?.unwrap_or(3))
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
    copertura: f64,
    ponti: Option<String>,
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
        let copertura: f64 = val("cover").map(|s| s.parse()).transpose()?.unwrap_or(0.0);
        let ponti = val("bridge").filter(|p| !p.trim().is_empty());
        if let Some(u) = val("exits") {
            let n: usize = u.parse().map_err(|_| "exits: a number 1-4")?;
            if !(1..=avvia_circuito::USCITE_MAX).contains(&n) {
                return Err("exits: a number 1-4".into());
            }
            avvia_circuito::imposta_uscite(n);
        }
        Ok(Self { percorso, elenco, autorita, socks, circuiti, salti, controllo, offuscamento, copertura, ponti })
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

/// Ponti nascosti: `indirizzo,x25519hex,improntahex,chiavehex` separati da ';' (la riga che il
/// nodo stampa con `--hidden-bridge`).
fn leggi_ponti(testo: &str) -> Risultato<Vec<(avvia_circuito::NodoRete, [u8; 32])>> {
    testo
        .split(';')
        .filter(|v| !v.trim().is_empty())
        .map(|voce| {
            let campi: Vec<&str> = voce.split(',').map(str::trim).collect();
            let [a, x, f, k] = campi.as_slice() else {
                return Err("bridge line: address,x25519,fingerprint,key (as printed by the bridge)".into());
            };
            let x = da_esadecimale(x)?;
            let addr = circuito_rete::risolvi_voce(a, &x)?;
            Ok(((addr, x, Some(da_esadecimale(f)?)), da_esadecimale(k)?))
        })
        .collect()
}

/// Voci per il comando `elenco`: `indirizzo,x25519hex,improntahex[,famiglia]` separate da ';'.
/// La famiglia (facoltativa) raggruppa i nodi dello stesso gestore o fornitore.
fn leggi_voci_elenco(testo: &str) -> Risultato<Vec<nyctalus_core::elenco::VoceElenco>> {
    testo
        .split(';')
        .filter(|v| !v.trim().is_empty())
        .map(|voce| {
            let campi: Vec<&str> = voce.split(',').map(str::trim).collect();
            let (addr, x, fp, fam, banda, ruolo) = match campi.as_slice() {
                [a, x, f] => (a, x, f, "", "", ""),
                [a, x, f, fam] => (a, x, f, *fam, "", ""),
                [a, x, f, fam, b] => (a, x, f, *fam, *b, ""),
                [a, x, f, fam, b, r] => (a, x, f, *fam, *b, *r),
                _ => return Err("entry in the form address,x25519hex,fingerprinthex[,family[,mbit[,middle|guard|noexit]]]".into()),
            };
            if !matches!(ruolo, "" | "middle" | "intermedio" | "guard" | "guardia" | "noexit" | "senza-uscita") {
                return Err(format!("role: «middle», «guard», «noexit» or empty, not {ruolo}").into());
            }
            let senza_uscita = matches!(ruolo, "noexit" | "senza-uscita");
            let famiglia = if fam.is_empty() {
                None
            } else if nyctalus_core::elenco::famiglia_valida(fam) {
                Some(fam.to_string())
            } else {
                return Err(format!("invalid family name: {fam} (letters, digits, _ . -, max 32)").into());
            };
            let banda = if banda.is_empty() {
                None
            } else {
                Some(banda.parse::<u32>().ok().filter(|b| (1..=1_000_000).contains(b)).ok_or("bandwidth: Mbit/s, 1-1000000")?)
            };
            Ok(nyctalus_core::elenco::VoceElenco {
                indirizzo: (*addr).to_string(),
                x25519: da_esadecimale(x)?,
                impronta_tls: da_esadecimale(fp)?,
                famiglia,
                banda,
                intermedio: !ruolo.is_empty() && !senza_uscita,
                guardia: matches!(ruolo, "guard" | "guardia"),
                senza_uscita,
            })
        })
        .collect()
}

/// Chiave esadecimale opzionale (32 byte / 64 caratteri).
fn da_esadecimale_opt(hex: Option<&str>) -> Risultato<Option<[u8; 32]>> {
    hex.map(da_esadecimale).transpose()
}

/// Diagnosi accendibile con NYCTALUS_DIAGNOSI=1 (spenta di serie): righe su stderr con l'ora.
pub(crate) fn diagnosi_accesa() -> bool {
    static ACCESA: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ACCESA.get_or_init(|| std::env::var("NYCTALUS_DIAGNOSI").is_ok_and(|v| v == "1"))
}

pub(crate) fn diagnosi(testo: impl FnOnce() -> String) {
    if diagnosi_accesa() {
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
        eprintln!("[diagnosi {t:.1}] {}", testo());
    }
}

/// Velocità leggibile (MB/s) per i riepiloghi.
pub fn velocita(byte: u64, secondi: f64) -> String {
    format!("{:.1} MB/s", byte as f64 / 1_000_000.0 / secondi.max(1e-9))
}
