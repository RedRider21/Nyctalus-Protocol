// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Demone client: apre un proxy SOCKS5 locale e inoltra ogni connessione a un
//! nodo di uscita tramite QUIC (VISIONE §8.2, tappa 6a-ii).
//!
//! SOLO UN SALTO: non ancora anonimo (vedi nota in `main.rs`).

use std::net::SocketAddr;

use nyctalus_core::socks5::{self, Destinazione, Esito, Richiesta, Saluto};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::Risultato;
use crate::travaso::collega;

pub async fn avvia(uscita: SocketAddr, impronta: [u8; 32], socks: SocketAddr) -> Risultato<()> {
    let locale: SocketAddr = if uscita.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse()?;
    let mut endpoint = quinn::Endpoint::client(locale)?;
    endpoint.set_default_client_config(crate::tls::config_client(impronta)?);
    let connessione = endpoint.connect(uscita, crate::tls::NOME_SERVER)?.await?;
    println!("Collegato al nodo di uscita {uscita}");

    let ascoltatore = TcpListener::bind(socks).await?;
    println!("Proxy SOCKS5 in ascolto su {socks}");
    println!("Prova:  curl --socks5-hostname {socks} https://example.com\n");

    loop {
        let (cliente, _) = ascoltatore.accept().await?;
        let connessione = connessione.clone();
        tokio::spawn(async move {
            if let Err(errore) = gestisci_cliente(cliente, connessione).await {
                eprintln!("connessione SOCKS chiusa: {errore}");
            }
        });
    }
}

async fn gestisci_cliente(mut cliente: TcpStream, connessione: quinn::Connection) -> Risultato<()> {
    // 1. Saluto SOCKS5.
    let saluto = leggi(&mut cliente, Saluto::analizza).await?;
    if !saluto.offre_senza_autenticazione() {
        cliente.write_all(&socks5::risposta_saluto(socks5::METODO_NESSUNO_ACCETTABILE)).await?;
        return Err("il client SOCKS non offre 'nessuna autenticazione'".into());
    }
    cliente.write_all(&socks5::risposta_saluto(socks5::METODO_SENZA_AUTENTICAZIONE)).await?;

    // 2. Richiesta CONNECT.
    let richiesta = leggi(&mut cliente, Richiesta::analizza).await?;
    let host = match &richiesta.destinazione {
        Destinazione::Nome(n) => n.clone(),
        Destinazione::Ipv4(b) => std::net::Ipv4Addr::from(*b).to_string(),
        Destinazione::Ipv6(b) => std::net::Ipv6Addr::from(*b).to_string(),
    };

    // 3. Apre uno stream verso l'uscita e invia [porta][len host][host].
    let (mut invio, mut ricezione) = connessione.open_bi().await?;
    invio.write_u16(richiesta.porta).await?;
    invio.write_u8(host.len() as u8).await?;
    invio.write_all(host.as_bytes()).await?;

    // 4. Esito dall'uscita → risposta SOCKS al client.
    let esito = ricezione.read_u8().await.unwrap_or(1);
    let (codice, ok) = if esito == 0 {
        (Esito::Riuscito, true)
    } else {
        (Esito::ConnessioneRifiutata, false)
    };
    cliente.write_all(&socks5::risposta_richiesta(codice)).await?;
    if !ok {
        return Err(format!("l'uscita non ha raggiunto {host}:{}", richiesta.porta).into());
    }

    // 5. Travaso bidirezionale.
    let (cliente_lettura, cliente_scrittura) = cliente.into_split();
    collega(ricezione, cliente_scrittura, cliente_lettura, invio).await;
    Ok(())
}

/// Legge dal socket finché la funzione di analisi ha byte a sufficienza.
async fn leggi<T>(
    cliente: &mut TcpStream,
    analizza: impl Fn(&[u8]) -> Result<T, socks5::ErroreSocks>,
) -> Risultato<T> {
    let mut buffer = Vec::with_capacity(262);
    loop {
        match analizza(&buffer) {
            Ok(valore) => return Ok(valore),
            Err(socks5::ErroreSocks::Incompleto) => {}
            Err(altro) => return Err(altro.into()),
        }
        let mut pezzo = [0u8; 262];
        let n = cliente.read(&mut pezzo).await?;
        if n == 0 {
            return Err("il client SOCKS ha chiuso durante la negoziazione".into());
        }
        buffer.extend_from_slice(&pezzo[..n]);
    }
}
