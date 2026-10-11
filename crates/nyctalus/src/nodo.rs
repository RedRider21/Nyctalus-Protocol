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

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use nyctalus_core::circuito::{self, PassoNodo};
use nyctalus_core::cipolla;

use crate::circuito_rete::{BIT_RISPOSTA, leggi_quadro, scrivi_quadro};
use crate::{Risultato, in_esadecimale};

/// Come il nodo autentica il collegamento verso il nodo a valle.
#[derive(Clone)]
pub struct Autenticazione {
    pub elenco: PathBuf,
    pub autorita: [u8; 32],
}

pub async fn nodo(ascolta: SocketAddr, segreto: [u8; 32], auth: Option<Autenticazione>) -> Risultato<()> {
    let pubblica = circuito::chiave_pubblica(&segreto);
    let identita = crate::tls::identita_server()?;
    println!("Nodo in ascolto su {ascolta}");
    println!("  chiave pubblica X25519: {}", in_esadecimale(&pubblica));
    println!("  impronta TLS:           {}", in_esadecimale(&identita.impronta));
    println!("  riga per l'elenco:  {ascolta},{},{}", in_esadecimale(&pubblica), in_esadecimale(&identita.impronta));
    if auth.is_some() {
        println!("  autenticazione dei collegamenti a valle: ATTIVA (via elenco firmato)");
    } else {
        println!("  autenticazione dei collegamenti a valle: assente (prototipo, link non verificati)");
    }

    let endpoint = quinn::Endpoint::server(identita.config, ascolta)?;
    let auth = Arc::new(auth);
    while let Some(in_arrivo) = endpoint.accept().await {
        let auth = auth.clone();
        tokio::spawn(async move {
            let Ok(connessione) = in_arrivo.await else { return };
            while let Ok((invio, ricezione)) = connessione.accept_bi().await {
                let auth = auth.clone();
                tokio::spawn(async move {
                    if let Err(e) = gestisci_circuito(invio, ricezione, segreto, &auth).await {
                        eprintln!("circuito chiuso: {e}");
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
        .ok_or_else(|| format!("nodo {indirizzo} non presente nell'elenco firmato").into())
}

async fn gestisci_circuito(
    mut invio_monte: quinn::SendStream,
    mut ricezione_monte: quinn::RecvStream,
    segreto: [u8; 32],
    auth: &Option<Autenticazione>,
) -> Risultato<()> {
    // Primo quadro dello stream = pacchetto di apertura Sphinx.
    let Some(pacchetto) = leggi_quadro(&mut ricezione_monte).await? else {
        return Ok(());
    };
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
            let (_endpoint, mut invio_valle, mut ricezione_valle) = collega_valle(prossimo, pacchetto, impronta).await?;

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
            uscita_circuito(invio_monte, ricezione_monte, chiave).await?;
        }
    }
    Ok(())
}

/// Uscita del circuito condiviso (punto D): smista le **celle** multiplexate.
/// Ogni cella `APRI` apre una connessione TCP verso Internet; le celle `DATI`
/// vi scorrono; `CHIUDI` la termina. I pacchetti in andata vengono spogliati
/// del proprio strato, quelli di ritorno rivestiti. Un solo task scrittore
/// serializza il ritorno con un contatore nonce unico.
async fn uscita_circuito(
    mut invio_monte: quinn::SendStream,
    mut ricezione_monte: quinn::RecvStream,
    chiave: [u8; 32],
) -> Risultato<()> {
    use crate::cella::{cmd, Cella};
    use std::collections::HashMap;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::mpsc;

    // Scrittore unico del ritorno.
    let (tx_ritorno, mut rx_ritorno) = mpsc::channel::<Vec<u8>>(256);
    let scrittore = tokio::spawn(async move {
        let mut n: u64 = 0;
        while let Some(mut payload) = rx_ritorno.recv().await {
            cipolla::sbuccia(BIT_RISPOSTA | n, &chiave, &mut payload);
            n += 1;
            if scrivi_quadro(&mut invio_monte, &payload).await.is_err() {
                break;
            }
        }
        let _ = invio_monte.finish();
    });

    // Per ogni flusso aperto: canale verso il task che scrive sul suo TCP.
    let mut verso_tcp: HashMap<u16, mpsc::Sender<Vec<u8>>> = HashMap::new();
    let mut n: u64 = 0;
    while let Some(mut payload) = leggi_quadro(&mut ricezione_monte).await? {
        cipolla::sbuccia(n, &chiave, &mut payload);
        n += 1;
        let Some(cella) = Cella::da_byte(&payload) else { continue };
        match cella.comando {
            cmd::APRI => {
                let flusso = cella.flusso;
                let destinazione = String::from_utf8_lossy(&cella.dati).to_string();
                match tokio::net::TcpStream::connect(destinazione.trim()).await {
                    Ok(tcp) => {
                        let (mut tcp_lettura, mut tcp_scrittura) = tcp.into_split();
                        let (tx_dati, mut rx_dati) = mpsc::channel::<Vec<u8>>(64);
                        verso_tcp.insert(flusso, tx_dati);
                        // Prima l'esito, così arriva sempre prima dei dati.
                        let _ = tx_ritorno.send(Cella::controllo(flusso, cmd::APERTO).in_byte()).await;
                        // TCP → client (celle DATI), poi CHIUDI a fine connessione.
                        let tx_ritorno2 = tx_ritorno.clone();
                        tokio::spawn(async move {
                            let mut buffer = vec![0u8; 16 * 1024];
                            loop {
                                match tcp_lettura.read(&mut buffer).await {
                                    Ok(0) | Err(_) => break,
                                    Ok(k) => {
                                        let cella = Cella::nuova(flusso, cmd::DATI, buffer[..k].to_vec());
                                        if tx_ritorno2.send(cella.in_byte()).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                            let _ = tx_ritorno2.send(Cella::controllo(flusso, cmd::CHIUDI).in_byte()).await;
                        });
                        // client → TCP.
                        tokio::spawn(async move {
                            while let Some(dati) = rx_dati.recv().await {
                                if tcp_scrittura.write_all(&dati).await.is_err() {
                                    break;
                                }
                            }
                            let _ = tcp_scrittura.shutdown().await;
                        });
                    }
                    Err(_) => {
                        let _ = tx_ritorno.send(Cella::controllo(flusso, cmd::RIFIUTATO).in_byte()).await;
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
    drop(verso_tcp);
    drop(tx_ritorno);
    let _ = scrittore.await;
    Ok(())
}

async fn collega_valle(
    prossimo: SocketAddr,
    pacchetto_apertura: Vec<u8>,
    impronta: Option<[u8; 32]>,
) -> Risultato<(quinn::Endpoint, quinn::SendStream, quinn::RecvStream)> {
    let locale: SocketAddr = if prossimo.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse()?;
    let mut endpoint = quinn::Endpoint::client(locale)?;
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
