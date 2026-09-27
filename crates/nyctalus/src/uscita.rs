// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Nodo di uscita: riceve dai client, su ogni stream QUIC una richiesta
//! `destinazione`, apre la connessione TCP verso Internet e fa da tramite.
//!
//! Protocollo su ogni stream bidirezionale QUIC (client → uscita):
//! `[porta u16 BE][lunghezza host u8][host]`, poi il flusso grezzo.
//! Risposta dell'uscita: `[esito u8]` (0 = connessa), poi il flusso grezzo.
//!
//! SOLO UN SALTO: non ancora anonimo (vedi nota in `main.rs`).

use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::Risultato;
use crate::travaso::collega;

pub async fn uscita(ascolta: SocketAddr) -> Risultato<()> {
    let identita = crate::tls::identita_server()?;
    let endpoint = quinn::Endpoint::server(identita.config, ascolta)?;
    println!("Nodo di uscita in ascolto su {ascolta}. Sull'altro computer esegui:\n");
    println!(
        "  nyctalus avvia --a <IP-DI-QUESTO-NODO>:{} --impronta {}\n",
        ascolta.port(),
        crate::in_esadecimale(&identita.impronta),
    );

    while let Some(in_arrivo) = endpoint.accept().await {
        tokio::spawn(async move {
            let Ok(connessione) = in_arrivo.await else { return };
            let remoto = connessione.remote_address();
            while let Ok((invio, ricezione)) = connessione.accept_bi().await {
                tokio::spawn(async move {
                    if let Err(errore) = servi_stream(invio, ricezione).await {
                        eprintln!("[{remoto}] stream chiuso: {errore}");
                    }
                });
            }
        });
    }
    Ok(())
}

async fn servi_stream(
    mut invio: quinn::SendStream,
    mut ricezione: quinn::RecvStream,
) -> Risultato<()> {
    let porta = ricezione.read_u16().await?;
    let lunghezza = ricezione.read_u8().await? as usize;
    let mut host = vec![0u8; lunghezza];
    ricezione.read_exact(&mut host).await?;
    let host = String::from_utf8(host)?;

    match TcpStream::connect((host.as_str(), porta)).await {
        Ok(tcp) => {
            invio.write_u8(0).await?; // esito: connessa
            let (tcp_lettura, tcp_scrittura) = tcp.into_split();
            collega(ricezione, tcp_scrittura, tcp_lettura, invio).await;
            Ok(())
        }
        Err(errore) => {
            invio.write_u8(1).await?; // esito: fallita
            let _ = invio.finish();
            Err(format!("connessione a {host}:{porta} fallita: {errore}").into())
        }
    }
}
