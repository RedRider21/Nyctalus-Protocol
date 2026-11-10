// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! `nyctalus avvia-circuito`: proxy SOCKS5 con **ragnatela conflux** (punto E2).
//!
//! Il demone apre più circuiti che condividono **guard e uscita** ma passano
//! per nodi intermedi diversi. Le celle di **tutte** le connessioni vengono
//! numerate con una sequenza end-to-end e **spezzate** sui vari circuiti; la
//! destinazione le **ricompone** nell'ordine originale ([`crate::conflux`]).
//! Il numero di sequenza viaggia dentro gli strati a cipolla: i relè non lo
//! vedono. Così un singolo scaricamento sfrutta la banda di più strade.
//!
//! Guardia persistente e uscita a caso con la regola di parentela ([`crate::percorsi`]);
//! guard e uscita restano gli stessi per la sessione. Prototipo: nessuna gestione della
//! morte di un circuito (un circuito perso blocca la ricomposizione).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use nyctalus_core::circuito::{apri_circuito, NodoPercorso};
use nyctalus_core::cipolla;
use nyctalus_core::socks5::{self, Destinazione, Esito, Richiesta, Saluto};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::cella::{cmd, Cella};
use crate::circuito_rete::{BIT_RISPOSTA, codifica_nodo, leggi_quadro, scrivi_quadro};
use crate::conflux::{self, Riordina};
use crate::Risultato;

/// Un nodo del percorso: indirizzo, chiave X25519 e, se noto, l'impronta TLS.
pub type NodoRete = (SocketAddr, [u8; 32], Option<[u8; 32]>);

type Chiavi = Arc<Vec<[u8; 32]>>;
type MappaFlussi = Arc<Mutex<HashMap<u16, mpsc::Sender<Cella>>>>;

pub const SALTI_MIN: usize = 1;
pub const SALTI_MAX: usize = nyctalus_core::circuito::MAX_NODI; // 5
pub const CIRCUITI_MIN: usize = 1;
pub const CIRCUITI_MAX: usize = 8;

pub async fn avvia_circuito(
    nodi: Vec<NodoRete>,
    circuiti: usize,
    salti: usize,
    socks: SocketAddr,
    controllo: SocketAddr,
    offusca: Option<[u8; 32]>,
    copertura: f64,
) -> Risultato<()> {
    if !(SALTI_MIN..=SALTI_MAX).contains(&salti) {
        return Err(format!("--hops must be between {SALTI_MIN} and {SALTI_MAX}").into());
    }
    if !(CIRCUITI_MIN..=CIRCUITI_MAX).contains(&circuiti) {
        return Err(format!("--circuits must be between {CIRCUITI_MIN} and {CIRCUITI_MAX}").into());
    }
    if nodi.len() < salti {
        return Err(format!("need at least {salti} nodes in the directory").into());
    }

    // Percorsi: una guardia per tutti; uno o più fasci, ognuno con la sua uscita.
    let gruppi = seleziona_fasci(&nodi, uscite_volute(), circuiti, salti)?;
    if !crate::percorsi::ponti().is_empty() {
        println!("  entry: hidden bridge (not in the public directory)");
    }
    for (f, percorsi) in gruppi.iter().enumerate() {
        for (i, p) in percorsi.iter().enumerate() {
            let strada: Vec<String> = p.iter().map(|n| n.0.to_string()).collect();
            if gruppi.len() > 1 {
                println!("  exit {f}, circuit {i}: {}", strada.join(" → "));
            } else {
                println!("  circuit {i}: {}", strada.join(" → "));
            }
        }
    }

    // Un solo collegamento QUIC verso la guardia condivisa (da tutti i fasci).
    let (guardiano, _, impronta_guard) = gruppi[0][0][0];
    // all'entrata con la chiave del ponte, se è un ponte nascosto
    let chiave_entrata = crate::percorsi::chiave_entrata(&gruppi[0][0][0], offusca);
    let config = match impronta_guard {
        Some(fp) => crate::tls::config_client(fp)?,
        None => crate::tls::config_client_insicuro()?,
    };
    // UDP, oppure (se UDP è bloccato) TCP alla stessa porta
    let (_endpoint, connessione) = crate::trasporto::collega_entrata(guardiano, chiave_entrata, config, impronta_guard).await?;

    // Ogni fascio: i suoi circuiti, uno schedulatore in andata e un collettore del ritorno.
    let mut fasci: Vec<Arc<Fascio>> = Vec::with_capacity(gruppi.len());
    for percorsi in &gruppi {
        // Identificativo del gruppo conflux (l'uscita ricompone per gruppo)
        let mut set_id = [0u8; 16];
        getrandom::fill(&mut set_id).expect("generatore casuale di sistema");
        let flussi: MappaFlussi = Arc::new(Mutex::new(HashMap::new()));
        let (tx_out, rx_out) = mpsc::channel::<Vec<u8>>(256); // celle in uscita (senza seq)
        let (tx_ret, rx_ret) = mpsc::channel::<(u64, Vec<u8>)>(256); // unità di ritorno
        let mut inbox_circuiti = Vec::with_capacity(percorsi.len());
        for percorso in percorsi {
            let nodi_sphinx: Vec<NodoPercorso> = percorso
                .iter()
                .map(|(a, p, _)| Ok(NodoPercorso { indirizzo: codifica_nodo(*a, p)?, pubblica: *p }))
                .collect::<Risultato<_>>()?;
            let circuito = apri_circuito(&nodi_sphinx, b"nyctalus")?;
            let chiavi: Chiavi = Arc::new(circuito.chiavi_salto);
            let (mut invio, ricezione) = crate::trasporto::apri_flusso(&connessione, chiave_entrata).await?;
            scrivi_quadro(&mut invio, &circuito.pacchetto).await?;

            let (tx_unit, rx_unit) = mpsc::channel::<Vec<u8>>(256);
            tokio::spawn(scrittore_circuito(invio, chiavi.clone(), set_id, rx_unit));
            tokio::spawn(lettore_circuito(ricezione, chiavi, tx_ret.clone()));
            inbox_circuiti.push(tx_unit);
        }
        let attivi = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        match crate::costante::scelto() {
            // copertura costante: fra i flussi e i circuiti un regolatore a ritmo fisso; all'uscita
            // la stessa richiesta per il ritorno (prima cella del gruppo)
            Some((r, sempre)) => {
                let (tx_regolato, rx_regolato) = mpsc::channel::<Vec<u8>>(256);
                let ritmo: crate::costante::Ritmo = Arc::new(Mutex::new(Some((r, sempre))));
                tokio::spawn(crate::costante::regola(rx_out, tx_regolato, ritmo, attivi.clone(), || Cella::riempimento().in_byte()));
                tokio::spawn(schedulatore(rx_regolato, inbox_circuiti));
                let _ = tx_out.send(Cella::nuova(0, cmd::COSTANTE, crate::costante::richiesta(r, sempre)).in_byte()).await;
            }
            None => {
                tokio::spawn(schedulatore(rx_out, inbox_circuiti));
            }
        }
        tokio::spawn(collettore_ritorno(rx_ret, flussi.clone()));
        fasci.push(Arc::new(Fascio { tx_out, flussi, attivi }));
    }
    let fasci: Arc<Vec<Arc<Fascio>>> = Arc::new(fasci);
    let scelte: Arc<Mutex<HashMap<String, usize>>> = Arc::new(Mutex::new(HashMap::new()));

    // Rumore di fondo (H2): se richiesto, inietta celle fittizie a ritmo di
    // Poisson nel flusso in uscita. Hanno la stessa lunghezza delle vere e
    // l'uscita le scarta: mascherano QUANDO l'utente invia davvero. (Nel primo fascio:
    // passa dalla stessa guardia di tutti, ed è lì che si vede il ritmo dell'utente.)
    if copertura > 0.0 {
        let tx = fasci[0].tx_out.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(ritardo_poisson(copertura)).await;
                if tx.send(Cella::riempimento().in_byte()).await.is_err() {
                    break;
                }
            }
        });
        println!("Cover traffic: ON (~{copertura:.1} dummy cells/s, uplink)");
    }

    // Metriche + porta di controllo per `monitor`.
    let metriche = Arc::new(crate::metriche::Metriche::nuovo());
    let tot_circuiti: usize = gruppi.iter().map(Vec::len).sum();
    metriche.circuiti.store(tot_circuiti as u64, std::sync::atomic::Ordering::Relaxed);
    avvia_controllo(controllo, metriche.clone());

    let ascoltatore = TcpListener::bind(socks).await?;
    if fasci.len() > 1 {
        println!(
            "SOCKS5 proxy on {socks} — {} exits (each site keeps its own), conflux over {circuiti} circuits each (shared guard, {salti} hops)",
            fasci.len()
        );
    } else {
        println!(
            "SOCKS5 proxy on {socks} — conflux over {tot_circuiti} circuits (shared guard and exit, {salti} hops)"
        );
    }
    if let Some((r, sempre)) = crate::costante::scelto() {
        println!("Constant cover: {r} cells/s each way, {}", if sempre { "always" } else { "while in use" });
    }
    println!("Monitor:  nyctalus monitor --control {controllo}");
    println!("Try:  curl --socks5-hostname {socks} http://example.com\n");

    // Nodi utilizzabili come punto d'incontro (e come tappe del circuito verso
    // di esso) per i siti `.nyct`: lo stesso elenco firmato che alimenta i
    // circuiti d'uscita.
    let punti_incontro: Arc<Vec<crate::incontro::Punto>> = Arc::new(crate::percorsi::pieni(&nodi));

    let mut prossimo_flusso: u16 = 0;
    loop {
        let (cliente, _) = ascoltatore.accept().await?;
        let flusso = prossimo_flusso;
        prossimo_flusso = prossimo_flusso.wrapping_add(1);
        let fasci = fasci.clone();
        let scelte = scelte.clone();
        let metriche = metriche.clone();
        let punti_incontro = punti_incontro.clone();
        tokio::spawn(async move {
            if let Err(e) = gestisci_flusso(cliente, flusso, &fasci, &scelte, &metriche, &punti_incontro, offusca, salti).await {
                eprintln!("flow {flusso} closed: {e}");
            }
        });
    }
}

/// Porta di controllo locale: a ogni connessione invia un'istantanea delle
/// metriche e chiude. Se non riesce a mettersi in ascolto avvisa e prosegue.
fn avvia_controllo(indirizzo: SocketAddr, metriche: Arc<crate::metriche::Metriche>) {
    tokio::spawn(async move {
        let ascoltatore = match TcpListener::bind(indirizzo).await {
            Ok(a) => a,
            Err(e) => {
                eprintln!("control port {indirizzo} unavailable: {e} (monitor disabled)");
                return;
            }
        };
        while let Ok((mut cliente, _)) = ascoltatore.accept().await {
            let snapshot = metriche.snapshot();
            let _ = cliente.write_all(snapshot.as_bytes()).await;
            let _ = cliente.shutdown().await;
        }
    });
}

/// Scrittore di un circuito: prima il LINK (set_id) poi le unità conflux, ognuna
/// avvolta negli strati a cipolla con nonce = contatore locale.
async fn scrittore_circuito(mut invio: quinn::SendStream, chiavi: Chiavi, set_id: [u8; 16], mut rx: mpsc::Receiver<Vec<u8>>) {
    let mut n: u64 = 0;
    // LINK riempito alla stessa dimensione di un'unità dati, così il 1º quadro
    // non si distingue dagli altri per lunghezza (il destinatario legge i primi
    // 16 byte come set_id e ignora il riempimento).
    let link = cipolla::avvolgi(n, &crate::conflux::quadro_link(&set_id), &chiavi);
    if scrivi_quadro(&mut invio, &link).await.is_err() {
        return;
    }
    n += 1;
    while let Some(unita) = rx.recv().await {
        let avvolto = cipolla::avvolgi(n, &unita, &chiavi);
        if scrivi_quadro(&mut invio, &avvolto).await.is_err() {
            break;
        }
        n += 1;
    }
}

/// Lettore di un circuito: toglie gli strati e passa (seq, cella) al collettore.
async fn lettore_circuito(mut ricezione: quinn::RecvStream, chiavi: Chiavi, tx_ret: mpsc::Sender<(u64, Vec<u8>)>) {
    let mut n: u64 = 0;
    loop {
        let letto = leggi_quadro(&mut ricezione).await;
        let Ok(Some(mut payload)) = letto else {
            crate::diagnosi(|| format!("client: circuito di ritorno finito dopo {n} unità: {:?}", letto.map(|o| o.is_some())));
            break;
        };
        svolgi(BIT_RISPOSTA | n, &chiavi, &mut payload);
        n += 1;
        if let Some((seq, cella)) = conflux::spacchetta(&payload)
            && tx_ret.send((seq, cella)).await.is_err()
        {
            break;
        }
    }
}

/// Assegna la sequenza end-to-end e spezza le celle sui circuiti a rotazione.
async fn schedulatore(mut rx_out: mpsc::Receiver<Vec<u8>>, inbox: Vec<mpsc::Sender<Vec<u8>>>) {
    let mut seq: u64 = 0;
    while let Some(cella) = rx_out.recv().await {
        let unita = conflux::impacchetta(seq, &cella);
        let liberi: Vec<usize> = inbox.iter().map(|t| t.capacity()).collect();
        let i = conflux::scegli_strada(&liberi, seq as usize);
        if inbox[i].send(unita).await.is_err() {
            break;
        }
        seq += 1;
    }
}

/// Ricompone le unità di ritorno e consegna le celle al flusso giusto.
async fn collettore_ritorno(mut rx_ret: mpsc::Receiver<(u64, Vec<u8>)>, flussi: MappaFlussi) {
    let mut riordina = Riordina::nuovo();
    // ragnatela codificata: le unità con il bit di codifica vanno al decodificatore
    let mut decodifica = crate::codifica::richiesta_configurata()
        .map(|(k, _)| crate::codifica::Decodificatore::nuovo(k));
    while let Some((seq, cella_bytes)) = rx_ret.recv().await {
        if crate::diagnosi_accesa() {
            static ULTIMA: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
            let mut u = ULTIMA.lock().unwrap();
            if u.is_none_or(|t| t.elapsed().as_secs_f64() > 2.0) {
                *u = Some(std::time::Instant::now());
                let (p, n, primo) = riordina.stato();
                crate::diagnosi(|| format!("client: riordino atteso {p}, in attesa {n} (prima {primo:?}), arrivata {seq}"));
            }
        }
        let pronte = match decodifica.as_mut() {
            Some(d) if seq & crate::codifica::CODIFICATA != 0 => d.inserisci(seq, cella_bytes),
            _ => riordina.inserisci(seq, cella_bytes),
        };
        for cb in pronte {
            if let Some(cella) = Cella::da_byte(&cb) {
                if cella.comando == cmd::RIEMPIMENTO {
                    continue; // cella fittizia (cover traffic): scartala
                }
                let inbox = flussi.lock().unwrap().get(&cella.flusso).cloned();
                if let Some(inbox) = inbox {
                    let _ = inbox.send(cella).await;
                }
            }
        }
    }
}

/// Ritardo casuale a legge esponenziale (processo di Poisson a `hz` eventi/s),
/// limitato a 60s. A ogni intervallo si invia una cella fittizia.
pub(crate) fn ritardo_poisson(hz: f64) -> std::time::Duration {
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).expect("generatore casuale di sistema");
    let u = (u64::from_le_bytes(b) as f64 / u64::MAX as f64).clamp(1e-12, 1.0);
    let secondi = (-u.ln() / hz).min(60.0);
    std::time::Duration::from_secs_f64(secondi)
}

/// Quante uscite diverse al massimo (fasci conflux paralleli).
pub const USCITE_MAX: usize = 4;
static USCITE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

/// Fissa il numero di uscite (da `--exits`); altrimenti vale `NYCTALUS_USCITE` (di serie 1).
pub fn imposta_uscite(n: usize) {
    let _ = USCITE.set(n.clamp(1, USCITE_MAX));
}

fn uscite_volute() -> usize {
    USCITE
        .get()
        .copied()
        .or_else(|| std::env::var("NYCTALUS_USCITE").ok().and_then(|v| v.parse().ok()))
        .unwrap_or(1)
        .clamp(1, USCITE_MAX)
}

/// Sceglie i percorsi di `fasci` gruppi conflux (PIANO-ANONIMATO, Tappa 2; capacità punto 3b):
/// tutti entrano dalla **stessa guardia** persistente; ogni fascio ha la **sua uscita** (diversa
/// dalle altre, mai parente della guardia, mai un nodo «solo intermedio») e `circuiti` percorsi
/// con intermedi diversi. Con una sola uscita (di serie) è il comportamento di sempre. Se non ci
/// sono abbastanza uscite i fasci sono meno di quelli chiesti.
fn seleziona_fasci(nodi: &[NodoRete], fasci: usize, circuiti: usize, salti: usize) -> Risultato<Vec<Vec<Vec<NodoRete>>>> {
    let guard = crate::percorsi::guardia(nodi);
    if salti == 1 {
        // Guard = uscita: nessun anonimato, ma valido. Una guardia «promossa» (intermedio stabile)
        // non fa mai da uscita: in quel caso si esce da un nodo pieno.
        let pieni = crate::percorsi::pieni(nodi);
        let unico = if pieni.iter().any(|n| n.1 == guard.1) { guard } else { *pieni.first().unwrap_or(&guard) };
        return Ok(vec![vec![vec![unico]; circuiti]]);
    }
    // uscite: mai un nodo «solo intermedio», mai la guardia, tutte diverse fra loro
    let mut uscite: Vec<NodoRete> = Vec::new();
    for _ in 0..fasci {
        let candidati: Vec<NodoRete> = crate::percorsi::pieni(nodi)
            .into_iter()
            .filter(|n| n.1 != guard.1 && uscite.iter().all(|u| u.1 != n.1))
            .collect();
        if candidati.is_empty() {
            break;
        }
        let (scelta, _) = crate::percorsi::scegli(&candidati, &[guard], 1)?;
        match scelta.first() {
            Some(&u) => uscite.push(u),
            None => break,
        }
    }
    if uscite.is_empty() {
        return Err("need at least 2 nodes in the directory".into());
    }
    let interni = salti - 2;
    if interni > 0 && nodi.len() < 3 {
        return Err("need middle nodes besides guard and exit".into());
    }
    let mut tutti = Vec::with_capacity(uscite.len());
    for uscita in uscite {
        let mut percorsi = Vec::with_capacity(circuiti);
        let mut usati: Vec<NodoRete> = Vec::new();
        for _ in 0..circuiti {
            // prima si prova con intermedi non ancora usati, poi con tutti
            let liberi: Vec<NodoRete> = nodi.iter().copied().filter(|n| usati.iter().all(|u| u.1 != n.1)).collect();
            let intermedi = match crate::percorsi::scegli_non_parenti(&liberi, &[guard, uscita], interni) {
                Some(v) => v,
                None => crate::percorsi::scegli(nodi, &[guard, uscita], interni)?.0,
            };
            if intermedi.len() < interni {
                return Err("need middle nodes besides guard and exit".into());
            }
            usati.extend(intermedi.iter().copied());
            let mut percorso = vec![guard];
            percorso.extend(intermedi);
            percorso.push(uscita);
            percorsi.push(percorso);
        }
        tutti.push(percorsi);
    }
    Ok(tutti)
}

/// Un fascio conflux: i suoi circuiti (stessa uscita), la coda delle celle in andata, i flussi
/// che ci passano e quanti ce ne sono aperti (per mandare i siti nuovi al fascio più libero).
struct Fascio {
    tx_out: mpsc::Sender<Vec<u8>>,
    flussi: MappaFlussi,
    attivi: Arc<std::sync::atomic::AtomicUsize>,
}

/// Il fascio per una destinazione: lo stesso sito usa SEMPRE la stessa uscita (il sito non vede
/// l'utente arrivare da più indirizzi insieme); un sito nuovo va al fascio con meno flussi aperti.
fn fascio_per(fasci: &[Arc<Fascio>], scelte: &Mutex<HashMap<String, usize>>, host: &str) -> usize {
    if fasci.len() == 1 {
        return 0;
    }
    let mut m = scelte.lock().unwrap();
    if let Some(&i) = m.get(host) {
        return i;
    }
    let carico = |i: usize| fasci[i].attivi.load(std::sync::atomic::Ordering::Relaxed);
    let minimo = (0..fasci.len()).map(carico).min().unwrap_or(0);
    let pari: Vec<usize> = (0..fasci.len()).filter(|&i| carico(i) == minimo).collect();
    let mut b = [0u8; 2];
    getrandom::fill(&mut b).expect("generatore casuale di sistema");
    let i = pari[u16::from_le_bytes(b) as usize % pari.len()];
    if m.len() > 4096 {
        m.clear(); // memoria limitata: si ricomincia (i siti già aperti tengono il loro fascio)
    }
    m.insert(host.to_string(), i);
    i
}

#[allow(clippy::too_many_arguments)]
async fn gestisci_flusso(
    mut cliente: TcpStream,
    flusso: u16,
    fasci: &[Arc<Fascio>],
    scelte: &Mutex<HashMap<String, usize>>,
    metriche: &Arc<crate::metriche::Metriche>,
    punti_incontro: &Arc<Vec<crate::incontro::Punto>>,
    offusca: Option<[u8; 32]>,
    salti: usize,
) -> Risultato<()> {
    use std::sync::atomic::Ordering::Relaxed;
    let saluto = leggi_socks(&mut cliente, Saluto::analizza).await?;
    // Nome utente/password (RFC 1929) non autentica: sceglie il trattamento del flusso.
    // «mix» = chat da mixnet (buste della modalità riservata, ritardate nei nodi).
    let mut mix = false;
    if saluto.offre_utente_password() {
        cliente.write_all(&socks5::risposta_saluto(socks5::METODO_UTENTE_PASSWORD)).await?;
        let (utente, _) = leggi_socks(&mut cliente, socks5::analizza_credenziali).await?;
        mix = utente == "mix";
        cliente.write_all(&[1, 0]).await?; // sotto-negoziazione riuscita
    } else if saluto.offre_senza_autenticazione() {
        cliente.write_all(&socks5::risposta_saluto(socks5::METODO_SENZA_AUTENTICAZIONE)).await?;
    } else {
        cliente.write_all(&socks5::risposta_saluto(socks5::METODO_NESSUNO_ACCETTABILE)).await?;
        return Err("the SOCKS client offers no usable authentication method".into());
    }
    let richiesta = leggi_socks(&mut cliente, Richiesta::analizza).await?;

    // Siti interni `.nyct`: non passano dall'uscita del circuito, ma dal punto
    // d'incontro con sessione cifrata end-to-end verso il servizio.
    if richiesta.destinazione.e_interna() {
        if let Destinazione::Nome(nome) = &richiesta.destinazione {
            metriche.flussi.fetch_add(1, Relaxed);
            let _guardia = GuardiaFlusso(metriche);
            return crate::incontro::tunnel_socks(cliente, nome, (**punti_incontro).clone(), offusca, salti, mix).await;
        }
    }

    let host = match &richiesta.destinazione {
        Destinazione::Nome(n) => n.clone(),
        Destinazione::Ipv4(b) => std::net::Ipv4Addr::from(*b).to_string(),
        Destinazione::Ipv6(b) => std::net::Ipv6Addr::from(*b).to_string(),
    };
    let destinazione = format!("{host}:{}", richiesta.porta);

    // il fascio (cioè l'uscita) di questo sito; il flusso si registra lì per il ritorno
    let i = fascio_per(fasci, scelte, &host);
    let fascio = &fasci[i];
    if fasci.len() > 1 {
        crate::diagnosi(|| format!("client: flusso {flusso} → uscita {i}"));
    }
    let tx_out = &fascio.tx_out;
    let (tx_in, mut rx_in) = mpsc::channel::<Cella>(64);
    fascio.flussi.lock().unwrap().insert(flusso, tx_in);
    fascio.attivi.fetch_add(1, Relaxed);
    struct Esce<'a>(&'a Fascio, u16);
    impl Drop for Esce<'_> {
        fn drop(&mut self) {
            self.0.flussi.lock().unwrap().remove(&self.1);
            self.0.attivi.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let _esce = Esce(fascio, flusso);

    tx_out.send(Cella::nuova(flusso, cmd::APRI, destinazione.into_bytes()).in_byte()).await?;
    let aperto = matches!(rx_in.recv().await, Some(c) if c.comando == cmd::APERTO);
    let codice = if aperto { Esito::Riuscito } else { Esito::ConnessioneRifiutata };
    cliente.write_all(&socks5::risposta_richiesta(codice)).await?;
    if !aperto {
        return Ok(());
    }

    metriche.flussi.fetch_add(1, Relaxed);
    let _guardia = GuardiaFlusso(metriche);

    let (mut app_lettura, mut app_scrittura) = cliente.into_split();
    let andata = async {
        let mut buffer = vec![0u8; crate::cella::CAPACITA];
        loop {
            let letti = crate::cella::leggi_per_cella(&mut app_lettura, &mut buffer).await?;
            if letti == 0 {
                break;
            }
            metriche.su.fetch_add(letti as u64, Relaxed);
            tx_out
                .send(Cella::nuova(flusso, cmd::DATI, buffer[..letti].to_vec()).in_byte())
                .await
                .map_err(|_| "circuit closed")?;
        }
        let _ = tx_out.send(Cella::controllo(flusso, cmd::CHIUDI).in_byte()).await;
        Risultato::Ok(())
    };
    let ritorno = async {
        while let Some(cella) = rx_in.recv().await {
            match cella.comando {
                cmd::DATI => {
                    metriche.giu.fetch_add(cella.dati.len() as u64, Relaxed);
                    app_scrittura.write_all(&cella.dati).await?;
                }
                cmd::CHIUDI => break,
                _ => {}
            }
        }
        let _ = app_scrittura.shutdown().await;
        Risultato::Ok(())
    };
    let (a, b) = tokio::join!(andata, ritorno);
    a?;
    b?;
    Ok(())
}

/// Decrementa il contatore dei flussi aperti quando la connessione termina.
struct GuardiaFlusso<'a>(&'a Arc<crate::metriche::Metriche>);
impl Drop for GuardiaFlusso<'_> {
    fn drop(&mut self) {
        self.0.flussi.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Toglie tutti gli strati dal pacchetto (lato client).
fn svolgi(nonce: u64, chiavi: &[[u8; 32]], payload: &mut [u8]) {
    for k in chiavi {
        cipolla::sbuccia(nonce, k, payload);
    }
}

async fn leggi_socks<T>(
    cliente: &mut TcpStream,
    analizza: impl Fn(&[u8]) -> Result<T, socks5::ErroreSocks>,
) -> Risultato<T> {
    let mut buffer = Vec::with_capacity(262);
    loop {
        match analizza(&buffer) {
            Ok(v) => return Ok(v),
            Err(socks5::ErroreSocks::Incompleto) => {}
            Err(altro) => return Err(altro.into()),
        }
        let mut pezzo = [0u8; 262];
        let n = cliente.read(&mut pezzo).await?;
        if n == 0 {
            return Err("the SOCKS client closed during negotiation".into());
        }
        buffer.extend_from_slice(&pezzo[..n]);
    }
}

#[cfg(test)]
mod test_fasci {
    use super::*;
    use std::sync::atomic::Ordering::Relaxed;

    fn fascio() -> Arc<Fascio> {
        let (tx_out, _rx) = mpsc::channel::<Vec<u8>>(1);
        Arc::new(Fascio { tx_out, flussi: Arc::new(Mutex::new(HashMap::new())), attivi: Arc::new(std::sync::atomic::AtomicUsize::new(0)) })
    }

    #[test]
    fn stesso_sito_stessa_uscita_siti_nuovi_al_fascio_piu_libero() {
        let fasci = vec![fascio(), fascio(), fascio()];
        let scelte = Mutex::new(HashMap::new());
        // un sito tiene sempre la sua uscita
        let a = fascio_per(&fasci, &scelte, "a.example");
        for _ in 0..20 {
            assert_eq!(fascio_per(&fasci, &scelte, "a.example"), a);
        }
        // con un fascio carico, i siti nuovi vanno agli altri
        fasci[a].attivi.store(5, Relaxed);
        // (8 siti: gli altri due arrivano a 4 flussi, sempre meno dei 5 del fascio carico)
        for k in 0..8 {
            let i = fascio_per(&fasci, &scelte, &format!("s{k}.example"));
            assert_ne!(i, a, "il fascio carico non riceve siti nuovi");
            fasci[i].attivi.fetch_add(1, Relaxed);
        }
        let carichi: Vec<usize> = fasci.iter().map(|f| f.attivi.load(Relaxed)).collect();
        assert_eq!(carichi.iter().sum::<usize>(), 13);
        assert!(carichi.iter().filter(|&&c| c == 4).count() == 2, "siti nuovi divisi in parti uguali: {carichi:?}");
        // un fascio solo: sempre 0, senza memoria
        let uno = vec![fascio()];
        let s1 = Mutex::new(HashMap::new());
        assert_eq!(fascio_per(&uno, &s1, "x"), 0);
        assert!(s1.lock().unwrap().is_empty());
    }
}
