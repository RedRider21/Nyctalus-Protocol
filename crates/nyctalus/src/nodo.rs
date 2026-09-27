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

use nyctalus_core::circuito::{self, PassoNodo};
use nyctalus_core::cipolla;

use crate::circuito_rete::{BIT_RISPOSTA, SEQ_APERTURA, leggi_quadro, scrivi_quadro};
use crate::{Risultato, in_esadecimale};

pub async fn nodo(ascolta: SocketAddr, segreto: [u8; 32]) -> Risultato<()> {
    let pubblica = circuito::chiave_pubblica(&segreto);
    let identita = crate::tls::identita_server()?;
    let endpoint = quinn::Endpoint::server(identita.config, ascolta)?;
    println!("Nodo in ascolto su {ascolta}");
    println!("  chiave pubblica: {}", in_esadecimale(&pubblica));
    println!("  nel percorso del client:  {ascolta},{}", in_esadecimale(&pubblica));

    while let Some(in_arrivo) = endpoint.accept().await {
        tokio::spawn(async move {
            let Ok(connessione) = in_arrivo.await else { return };
            while let Ok((invio, ricezione)) = connessione.accept_bi().await {
                tokio::spawn(async move {
                    if let Err(e) = gestisci_circuito(invio, ricezione, segreto).await {
                        eprintln!("circuito chiuso: {e}");
                    }
                });
            }
        });
    }
    Ok(())
}

async fn gestisci_circuito(
    mut invio_monte: quinn::SendStream,
    mut ricezione_monte: quinn::RecvStream,
    segreto: [u8; 32],
) -> Risultato<()> {
    // Primo quadro: apertura Sphinx.
    let Some((seq, pacchetto)) = leggi_quadro(&mut ricezione_monte).await? else {
        return Ok(());
    };
    if seq != SEQ_APERTURA {
        return Err("atteso il pacchetto di apertura del circuito".into());
    }
    let elaborazione = circuito::elabora_nodo(&pacchetto, &segreto)?;
    let chiave = elaborazione.chiave_salto;

    match elaborazione.passo {
        PassoNodo::Inoltra { prossimo_indirizzo, pacchetto } => {
            let prossimo = crate::circuito_rete::decodifica_indirizzo(&prossimo_indirizzo)?;
            // `_endpoint` va tenuto in vita per tutta la durata del circuito.
            let (_endpoint, mut invio_valle, mut ricezione_valle) = collega_valle(prossimo, pacchetto).await?;

            // Andata: togli il tuo strato e inoltra a valle.
            let andata = async {
                while let Some((seq, mut payload)) = leggi_quadro(&mut ricezione_monte).await? {
                    cipolla::sbuccia(seq, &chiave, &mut payload);
                    scrivi_quadro(&mut invio_valle, seq, &payload).await?;
                }
                let _ = invio_valle.finish();
                Risultato::Ok(())
            };
            // Ritorno: aggiungi il tuo strato e rimanda a monte.
            let ritorno = async {
                while let Some((seq, mut payload)) = leggi_quadro(&mut ricezione_valle).await? {
                    cipolla::sbuccia(seq, &chiave, &mut payload);
                    scrivi_quadro(&mut invio_monte, seq, &payload).await?;
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

    // 1) Primo quadro dati = richiesta di connessione (destinazione).
    let Some((seq, mut payload)) = leggi_quadro(&mut ricezione_monte).await? else {
        return Ok(());
    };
    cipolla::sbuccia(seq, &chiave, &mut payload);
    let destinazione = String::from_utf8_lossy(&payload).to_string();

    let tcp = tokio::net::TcpStream::connect(destinazione.trim()).await;
    // 2) Esito verso il client (quadro di ritorno): "OK" o "NO".
    let esito_seq = seq | BIT_RISPOSTA;
    let mut tcp = match tcp {
        Ok(t) => {
            invia_ritorno(&mut invio_monte, esito_seq, &chiave, b"OK").await?;
            t
        }
        Err(e) => {
            invia_ritorno(&mut invio_monte, esito_seq, &chiave, b"NO").await?;
            let _ = invio_monte.finish();
            return Err(format!("uscita: connessione a {destinazione} fallita: {e}").into());
        }
    };
    let (mut tcp_lettura, mut tcp_scrittura) = tcp.split();

    // 3) Tramite bidirezionale.
    let andata = async {
        while let Some((seq, mut payload)) = leggi_quadro(&mut ricezione_monte).await? {
            cipolla::sbuccia(seq, &chiave, &mut payload);
            tcp_scrittura.write_all(&payload).await?;
        }
        let _ = tcp_scrittura.shutdown().await;
        Risultato::Ok(())
    };
    let ritorno = async {
        let mut buffer = vec![0u8; 16 * 1024];
        let mut contatore: u64 = 1; // 0 è riservato all'esito della connessione
        loop {
            let n = tcp_lettura.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            invia_ritorno(&mut invio_monte, BIT_RISPOSTA | contatore, &chiave, &buffer[..n]).await?;
            contatore += 1;
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
    seq: u64,
    chiave: &[u8; 32],
    dati: &[u8],
) -> Risultato<()> {
    let mut payload = dati.to_vec();
    cipolla::sbuccia(seq, chiave, &mut payload);
    scrivi_quadro(invio, seq, &payload).await
}

async fn collega_valle(
    prossimo: SocketAddr,
    pacchetto_apertura: Vec<u8>,
) -> Risultato<(quinn::Endpoint, quinn::SendStream, quinn::RecvStream)> {
    let locale: SocketAddr = if prossimo.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse()?;
    let mut endpoint = quinn::Endpoint::client(locale)?;
    endpoint.set_default_client_config(crate::tls::config_client_insicuro()?);
    let connessione = endpoint.connect(prossimo, "nyctalus.invalid")?.await?;
    let (mut invio, ricezione) = connessione.open_bi().await?;
    scrivi_quadro(&mut invio, SEQ_APERTURA, &pacchetto_apertura).await?;
    Ok((endpoint, invio, ricezione))
}
