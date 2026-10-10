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

/// Uscita del circuito: il primo quadro dati porta la destinazione
/// (`host:porta`), poi si fa da tramite fra il circuito e Internet.
/// Ogni pacchetto in andata viene spogliato del proprio strato, ogni pacchetto
/// di ritorno ne viene rivestito.
async fn uscita_circuito(
    mut invio_monte: quinn::SendStream,
    mut ricezione_monte: quinn::RecvStream,
    chiave: [u8; 32],
) -> Risultato<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    // 1) Primo quadro dati = richiesta di connessione (andata #0).
    let Some(mut payload) = leggi_quadro(&mut ricezione_monte).await? else {
        return Ok(());
    };
    cipolla::sbuccia(0, &chiave, &mut payload);
    let destinazione = String::from_utf8_lossy(&payload).to_string();

    let tcp = tokio::net::TcpStream::connect(destinazione.trim()).await;
    // 2) Esito verso il client (ritorno #0): "OK" o "NO".
    let mut tcp = match tcp {
        Ok(t) => {
            invia_ritorno(&mut invio_monte, BIT_RISPOSTA, &chiave, b"OK").await?;
            t
        }
        Err(e) => {
            invia_ritorno(&mut invio_monte, BIT_RISPOSTA, &chiave, b"NO").await?;
            let _ = invio_monte.finish();
            return Err(format!("uscita: connessione a {destinazione} fallita: {e}").into());
        }
    };
    let (mut tcp_lettura, mut tcp_scrittura) = tcp.split();

    // 3) Tramite bidirezionale, con contatori locali (andata da 1, ritorno da 1).
    let andata = async {
        let mut n: u64 = 1; // 0 era la richiesta di connessione
        while let Some(mut payload) = leggi_quadro(&mut ricezione_monte).await? {
            cipolla::sbuccia(n, &chiave, &mut payload);
            tcp_scrittura.write_all(&payload).await?;
            n += 1;
        }
        let _ = tcp_scrittura.shutdown().await;
        Risultato::Ok(())
    };
    let ritorno = async {
        let mut buffer = vec![0u8; 16 * 1024];
        let mut n: u64 = 1; // 0 era l'esito della connessione
        loop {
            let letti = tcp_lettura.read(&mut buffer).await?;
            if letti == 0 {
                break;
            }
            invia_ritorno(&mut invio_monte, BIT_RISPOSTA | n, &chiave, &buffer[..letti]).await?;
            n += 1;
        }
        let _ = invio_monte.finish();
        Risultato::Ok(())
    };
    let (a, b) = tokio::join!(andata, ritorno);
    a?;
    b?;
    Ok(())
}

async fn invia_ritorno(
    invio: &mut quinn::SendStream,
    nonce: u64,
    chiave: &[u8; 32],
    dati: &[u8],
) -> Risultato<()> {
    let mut payload = dati.to_vec();
    cipolla::sbuccia(nonce, chiave, &mut payload);
    scrivi_quadro(invio, &payload).await
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
