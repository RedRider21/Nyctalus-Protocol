// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Nodo del percorso (tappa 2). Riceve un pacchetto di apertura Sphinx, ricava
//! la propria chiave di salto e l'indirizzo del nodo successivo, poi fa da
//! relè: sui pacchetti dati toglie il proprio strato a cipolla e li inoltra;
//! sui pacchetti di ritorno aggiunge il proprio strato e li rimanda indietro.
//! Se è l'ultimo nodo (uscita del circuito) consegna il messaggio e risponde.
//!
//! Ogni nodo conosce solo il vicino a monte e quello a valle: mai l'intero
//! percorso. In questo prototipo i collegamenti fra nodi non sono autenticati
//! (vedi `tls::config_client_insicuro`) e l'uscita si limita a fare da eco.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use nyctalus_core::circuito::{self, PassoNodo};
use nyctalus_core::cipolla;
use tokio::sync::mpsc;

use crate::cella::{cmd, Cella};
use crate::circuito_rete::{BIT_RISPOSTA, leggi_quadro, scrivi_quadro};
use crate::conflux::{self, Riordina};
use crate::{Risultato, in_esadecimale};

/// Come il nodo autentica il collegamento verso il nodo a valle.
#[derive(Clone)]
pub struct Autenticazione {
    pub elenco: PathBuf,
    pub autorita: [u8; 32],
}

pub async fn nodo(
    ascolta: SocketAddr,
    segreto: [u8; 32],
    auth: Option<Autenticazione>,
    offusca: Option<[u8; 32]>,
    copertura: f64,
) -> Risultato<()> {
    let _ = COPERTURA_NODO.set(copertura);
    let pubblica = circuito::chiave_pubblica(&segreto);
    let identita = crate::tls::identita_server()?;
    println!("Node listening on {ascolta}");
    println!("  X25519 public key: {}", in_esadecimale(&pubblica));
    println!("  TLS fingerprint:   {}", in_esadecimale(&identita.impronta));
    println!("  directory entry:   {ascolta},{},{}", in_esadecimale(&pubblica), in_esadecimale(&identita.impronta));
    if auth.is_some() {
        println!("  downstream link authentication: ON (via signed directory)");
    } else {
        println!("  downstream link authentication: off (prototype, unverified links)");
    }
    if offusca.is_some() {
        println!("  transport camouflage: ON");
    }
    if copertura > 0.0 {
        println!("  cover traffic (downstream): ~{copertura:.1} dummy cells/s");
    }

    let endpoint = crate::trasporto::endpoint_server(identita.config, ascolta, offusca)?;
    let auth = Arc::new(auth);
    while let Some(in_arrivo) = endpoint.accept().await {
        let auth = auth.clone();
        tokio::spawn(async move {
            let Ok(connessione) = in_arrivo.await else { return };
            while let Ok((invio, ricezione)) = connessione.accept_bi().await {
                let auth = auth.clone();
                let connessione = connessione.clone();
                tokio::spawn(async move {
                    if let Err(e) = gestisci_circuito(connessione, invio, ricezione, segreto, &auth, offusca).await {
                        eprintln!("circuit closed: {e}");
                    }
                });
            }
        });
    }
    Ok(())
}

/// Cerca nell'elenco firmato l'impronta TLS del nodo a quell'indirizzo.
fn impronta_attesa(auth: &Autenticazione, indirizzo: SocketAddr) -> Risultato<[u8; 32]> {
    let testo = std::fs::read_to_string(&auth.elenco)?;
    let voci = nyctalus_core::elenco::verifica_elenco(&testo, &auth.autorita)?;
    let cercato = indirizzo.to_string();
    voci
        .into_iter()
        .find(|v| v.indirizzo == cercato)
        .map(|v| v.impronta_tls)
        .ok_or_else(|| format!("node {indirizzo} not in the signed directory").into())
}

async fn gestisci_circuito(
    connessione: quinn::Connection,
    mut invio_monte: quinn::SendStream,
    mut ricezione_monte: quinn::RecvStream,
    segreto: [u8; 32],
    auth: &Option<Autenticazione>,
    offusca: Option<[u8; 32]>,
) -> Risultato<()> {
    // Primo quadro dello stream: controllo del punto d'incontro (`.nyct`) se
    // inizia con la marca, altrimenti pacchetto di apertura Sphinx del circuito.
    let Some(pacchetto) = leggi_quadro(&mut ricezione_monte).await? else {
        return Ok(());
    };
    if pacchetto.starts_with(crate::incontro::MAGIA) {
        return crate::incontro::gestisci(connessione, invio_monte, ricezione_monte, pacchetto).await;
    }
    let elaborazione = circuito::elabora_nodo(&pacchetto, &segreto)?;
    let chiave = elaborazione.chiave_salto;

    match elaborazione.passo {
        PassoNodo::Inoltra { prossimo_indirizzo, pacchetto } => {
            let prossimo = crate::circuito_rete::decodifica_indirizzo(&prossimo_indirizzo)?;
            // Autentica il nodo a valle se abbiamo un elenco firmato.
            let impronta = match auth {
                Some(a) => Some(impronta_attesa(a, prossimo)?),
                None => None,
            };
            // `_endpoint` va tenuto in vita per tutta la durata del circuito.
            let (_endpoint, mut invio_valle, mut ricezione_valle) = collega_valle(prossimo, pacchetto, impronta, offusca).await?;

            // Andata: togli il tuo strato e inoltra a valle. Il nonce è un
            // contatore locale (non viaggia sul filo): stessa posizione, stesso
            // nonce su ogni nodo, grazie all'ordine garantito dallo stream.
            let andata = async {
                let mut n: u64 = 0;
                while let Some(mut payload) = leggi_quadro(&mut ricezione_monte).await? {
                    cipolla::sbuccia(n, &chiave, &mut payload);
                    scrivi_quadro(&mut invio_valle, &payload).await?;
                    n += 1;
                }
                let _ = invio_valle.finish();
                Risultato::Ok(())
            };
            // Ritorno: aggiungi il tuo strato e rimanda a monte.
            let ritorno = async {
                let mut n: u64 = 0;
                while let Some(mut payload) = leggi_quadro(&mut ricezione_valle).await? {
                    cipolla::sbuccia(BIT_RISPOSTA | n, &chiave, &mut payload);
                    scrivi_quadro(&mut invio_monte, &payload).await?;
                    n += 1;
                }
                let _ = invio_monte.finish();
                Risultato::Ok(())
            };
            let (a, b) = tokio::join!(andata, ritorno);
            a?;
            b?;
        }
        PassoNodo::Finale => {
            // 1º quadro onion dell'uscita: tolto il nostro strato, è il controllo
            // del punto d'incontro (marca MAGIA) oppure il LINK del gruppo conflux.
            let Some(mut primo) = leggi_quadro(&mut ricezione_monte).await? else {
                return Ok(());
            };
            cipolla::sbuccia(0, &chiave, &mut primo);
            // Rendezvous se, tolto l'incapsulamento del tunnel, il 1º quadro
            // inizia con MAGIA; altrimenti è il LINK del gruppo conflux.
            let e_rendezvous = crate::incontro::spacchetta_tunnel(&primo)
                .map(|p| p.starts_with(crate::incontro::MAGIA))
                .unwrap_or(false);
            if e_rendezvous {
                crate::incontro::uscita_rendezvous(invio_monte, ricezione_monte, chiave, primo).await?;
            } else {
                uscita_conflux(invio_monte, ricezione_monte, chiave, primo).await?;
            }
        }
    }
    Ok(())
}

/// Stato condiviso, nel processo, di un gruppo conflux all'uscita: raccoglie
/// le unità in andata dai vari circuiti e tiene le caselle di ritorno di ognuno.
struct ConfluxUscita {
    tx_collect: mpsc::Sender<(u64, Vec<u8>)>,
    circuiti_ritorno: Arc<Mutex<Vec<(u64, mpsc::Sender<Vec<u8>>)>>>,
    prossimo_id: AtomicU64,
}

/// Ritmo (celle/s) del rumore di fondo in **discesa** (uscita→client), impostato
/// all'avvio del nodo con `--cover`. 0 = spento.
static COPERTURA_NODO: OnceLock<f64> = OnceLock::new();
fn copertura_nodo() -> f64 {
    *COPERTURA_NODO.get().unwrap_or(&0.0)
}

fn registro() -> &'static Mutex<HashMap<[u8; 16], Arc<ConfluxUscita>>> {
    static R: OnceLock<Mutex<HashMap<[u8; 16], Arc<ConfluxUscita>>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Trova o crea lo stato del gruppo conflux con quel set_id, avviando (alla
/// creazione) il collettore che ricompone l'andata e lo schedulatore del ritorno.
fn ottieni_set(set_id: [u8; 16]) -> Arc<ConfluxUscita> {
    let mut reg = registro().lock().unwrap();
    if let Some(s) = reg.get(&set_id) {
        return s.clone();
    }
    let (tx_collect, rx_collect) = mpsc::channel::<(u64, Vec<u8>)>(256);
    let (tx_return, rx_return) = mpsc::channel::<Vec<u8>>(256);
    let circuiti_ritorno = Arc::new(Mutex::new(Vec::new()));
    let set = Arc::new(ConfluxUscita {
        tx_collect,
        circuiti_ritorno: circuiti_ritorno.clone(),
        prossimo_id: AtomicU64::new(0),
    });
    reg.insert(set_id, set.clone());
    drop(reg);
    tokio::spawn(collettore_uscita(rx_collect, tx_return.clone()));
    tokio::spawn(schedulatore_ritorno(set_id, rx_return, circuiti_ritorno.clone()));

    // Rumore di fondo in discesa: celle fittizie a ritmo di Poisson verso il
    // client, scartate dal suo lato. Si ferma quando il gruppo non ha più
    // circuiti attivi (evita di girare a vuoto su circuiti morti).
    let copertura = copertura_nodo();
    if copertura > 0.0 {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(crate::avvia_circuito::ritardo_poisson(copertura)).await;
                if circuiti_ritorno.lock().unwrap().is_empty() {
                    break;
                }
                if tx_return.send(Cella::riempimento().in_byte()).await.is_err() {
                    break;
                }
            }
        });
    }
    set
}

/// Uscita del gruppo conflux (punto E2): i circuiti che condividono questa
/// uscita consegnano le celle spezzate con un numero di sequenza end-to-end;
/// il collettore del gruppo le **ricompone** e le smista alle connessioni TCP.
/// I circuiti si riconoscono tramite il LINK (set_id) nel primo quadro.
async fn uscita_conflux(
    mut invio_monte: quinn::SendStream,
    mut ricezione_monte: quinn::RecvStream,
    chiave: [u8; 32],
    link: Vec<u8>,
) -> Risultato<()> {
    // `link` = 1º quadro onion già sbucciato (LINK con il set_id del gruppo).
    if link.len() < 16 {
        return Err("missing conflux group LINK".into());
    }
    let mut set_id = [0u8; 16];
    set_id.copy_from_slice(&link[..16]);
    let set = ottieni_set(set_id);

    // Scrittore di ritorno di QUESTO circuito. Si registra con un id e si toglie
    // dalla lista del gruppo quando il circuito cade (così lo schedulatore e il
    // rumore di fondo non girano a vuoto su circuiti morti).
    let (tx_unit, mut rx_unit) = mpsc::channel::<Vec<u8>>(256);
    let id = set.prossimo_id.fetch_add(1, Ordering::Relaxed);
    set.circuiti_ritorno.lock().unwrap().push((id, tx_unit));
    let circuiti = set.circuiti_ritorno.clone();
    tokio::spawn(async move {
        let mut n: u64 = 0;
        while let Some(mut unita) = rx_unit.recv().await {
            cipolla::sbuccia(BIT_RISPOSTA | n, &chiave, &mut unita);
            n += 1;
            if scrivi_quadro(&mut invio_monte, &unita).await.is_err() {
                break;
            }
        }
        let _ = invio_monte.finish();
        circuiti.lock().unwrap().retain(|(i, _)| *i != id);
    });

    // Unità in andata → collettore del gruppo (0 era il LINK).
    let mut n: u64 = 1;
    while let Some(mut payload) = leggi_quadro(&mut ricezione_monte).await? {
        cipolla::sbuccia(n, &chiave, &mut payload);
        n += 1;
        if let Some((seq, cella)) = conflux::spacchetta(&payload)
            && set.tx_collect.send((seq, cella)).await.is_err()
        {
            break;
        }
    }
    Ok(())
}

/// Collettore del gruppo: ricompone le unità in andata e, in ordine, apre le
/// connessioni TCP (`APRI`), vi scrive (`DATI`), le chiude (`CHIUDI`).
async fn collettore_uscita(mut rx: mpsc::Receiver<(u64, Vec<u8>)>, tx_return: mpsc::Sender<Vec<u8>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut riordina = Riordina::nuovo();
    let mut verso_tcp: HashMap<u16, mpsc::Sender<Vec<u8>>> = HashMap::new();
    while let Some((seq, cella_bytes)) = rx.recv().await {
        for cb in riordina.inserisci(seq, cella_bytes) {
            let Some(cella) = Cella::da_byte(&cb) else { continue };
            match cella.comando {
                cmd::APRI => {
                    let flusso = cella.flusso;
                    let dest = String::from_utf8_lossy(&cella.dati).to_string();
                    match crate::ponte::connetti(dest.trim()).await {
                        Ok(flusso_uscita) => {
                            let (mut lettura, mut scrittura) = tokio::io::split(flusso_uscita);
                            let (tx_w, mut rx_w) = mpsc::channel::<Vec<u8>>(64);
                            verso_tcp.insert(flusso, tx_w);
                            let _ = tx_return.send(Cella::controllo(flusso, cmd::APERTO).in_byte()).await;
                            let tx_ret2 = tx_return.clone();
                            tokio::spawn(async move {
                                let mut buffer = vec![0u8; crate::cella::CAPACITA];
                                loop {
                                    match lettura.read(&mut buffer).await {
                                        Ok(0) | Err(_) => break,
                                        Ok(k) => {
                                            let c = Cella::nuova(flusso, cmd::DATI, buffer[..k].to_vec());
                                            if tx_ret2.send(c.in_byte()).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                }
                                let _ = tx_ret2.send(Cella::controllo(flusso, cmd::CHIUDI).in_byte()).await;
                            });
                            tokio::spawn(async move {
                                while let Some(d) = rx_w.recv().await {
                                    if scrittura.write_all(&d).await.is_err() {
                                        break;
                                    }
                                }
                                let _ = scrittura.shutdown().await;
                            });
                        }
                        Err(_) => {
                            let _ = tx_return.send(Cella::controllo(flusso, cmd::RIFIUTATO).in_byte()).await;
                        }
                    }
                }
                cmd::DATI => {
                    if let Some(tx) = verso_tcp.get(&cella.flusso) {
                        let _ = tx.send(cella.dati).await;
                    }
                }
                cmd::CHIUDI => {
                    verso_tcp.remove(&cella.flusso);
                }
                _ => {}
            }
        }
    }
}

/// Schedulatore del ritorno: numera le celle con la sequenza end-to-end e le
/// spezza sui circuiti del gruppo a rotazione.
async fn schedulatore_ritorno(
    set_id: [u8; 16],
    mut rx: mpsc::Receiver<Vec<u8>>,
    circuiti: Arc<Mutex<Vec<(u64, mpsc::Sender<Vec<u8>>)>>>,
) {
    let mut seq: u64 = 0;
    while let Some(cella_bytes) = rx.recv().await {
        let unita = conflux::impacchetta(seq, &cella_bytes);
        let destinatario = {
            let c = circuiti.lock().unwrap();
            if c.is_empty() {
                None
            } else {
                Some(c[(seq as usize) % c.len()].1.clone())
            }
        };
        match destinatario {
            Some(tx) => {
                let _ = tx.send(unita).await;
                seq += 1;
            }
            None => break,
        }
    }
    // Gruppo esaurito: togli lo stato dal registro.
    registro().lock().unwrap().remove(&set_id);
}

async fn collega_valle(
    prossimo: SocketAddr,
    pacchetto_apertura: Vec<u8>,
    impronta: Option<[u8; 32]>,
    offusca: Option<[u8; 32]>,
) -> Risultato<(quinn::Endpoint, quinn::SendStream, quinn::RecvStream)> {
    let locale: SocketAddr = if prossimo.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse()?;
    let mut endpoint = crate::trasporto::endpoint_client(locale, offusca)?;
    let config = match impronta {
        Some(fp) => crate::tls::config_client(fp)?,
        None => crate::tls::config_client_insicuro()?,
    };
    endpoint.set_default_client_config(config);
    let connessione = endpoint.connect(prossimo, "nyctalus.invalid")?.await?;
    let (mut invio, ricezione) = connessione.open_bi().await?;
    scrivi_quadro(&mut invio, &pacchetto_apertura).await?;
    Ok((endpoint, invio, ricezione))
}
