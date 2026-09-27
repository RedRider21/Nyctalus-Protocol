// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! `nyctalus avvia-circuito`: proxy SOCKS5 locale che instrada ogni
//! connessione attraverso un circuito onion a più nodi (tappa 3).
//!
//! A differenza di `avvia` (un solo salto), qui il traffico passa per
//! guard → nodi intermedi → uscita, e **nessun nodo conosce insieme origine e
//! destinazione**. Per ogni connessione SOCKS si apre un circuito dedicato.
//!
//! Prototipo: i collegamenti fra nodi non sono ancora autenticati, e c'è un
//! circuito per connessione (non condiviso). Vedi STATO.md.

use std::net::SocketAddr;

use nyctalus_core::circuito::{apri_circuito, NodoPercorso};
use nyctalus_core::cipolla;
use nyctalus_core::socks5::{self, Destinazione, Esito, Richiesta, Saluto};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::circuito_rete::{SEQ_APERTURA, codifica_indirizzo, leggi_quadro, scrivi_quadro};
use crate::Risultato;

pub async fn avvia_circuito(
    percorso_rete: Vec<(SocketAddr, [u8; 32])>,
    socks: SocketAddr,
) -> Risultato<()> {
    if percorso_rete.is_empty() {
        return Err("indicare i nodi del circuito con --nodi".into());
    }
    let ascoltatore = TcpListener::bind(socks).await?;
    println!("Proxy SOCKS5 (circuito a {} nodi) in ascolto su {socks}", percorso_rete.len());
    println!("Prova:  curl --socks5-hostname {socks} http://example.com\n");

    loop {
        let (cliente, _) = ascoltatore.accept().await?;
        let percorso = percorso_rete.clone();
        tokio::spawn(async move {
            if let Err(e) = gestisci(cliente, percorso).await {
                eprintln!("connessione chiusa: {e}");
            }
        });
    }
}

async fn gestisci(mut cliente: TcpStream, percorso_rete: Vec<(SocketAddr, [u8; 32])>) -> Risultato<()> {
    // SOCKS5: saluto + richiesta.
    let saluto = leggi_socks(&mut cliente, Saluto::analizza).await?;
    if !saluto.offre_senza_autenticazione() {
        cliente.write_all(&socks5::risposta_saluto(socks5::METODO_NESSUNO_ACCETTABILE)).await?;
        return Err("il client SOCKS non offre 'nessuna autenticazione'".into());
    }
    cliente.write_all(&socks5::risposta_saluto(socks5::METODO_SENZA_AUTENTICAZIONE)).await?;
    let richiesta = leggi_socks(&mut cliente, Richiesta::analizza).await?;
    let host = match &richiesta.destinazione {
        Destinazione::Nome(n) => n.clone(),
        Destinazione::Ipv4(b) => std::net::Ipv4Addr::from(*b).to_string(),
        Destinazione::Ipv6(b) => std::net::Ipv6Addr::from(*b).to_string(),
    };
    let destinazione = format!("{host}:{}", richiesta.porta);

    // Apertura del circuito.
    let percorso: Vec<NodoPercorso> = percorso_rete
        .iter()
        .map(|(a, p)| Ok(NodoPercorso { indirizzo: codifica_indirizzo(*a)?, pubblica: *p }))
        .collect::<Risultato<_>>()?;
    let circuito = apri_circuito(&percorso, b"apertura")?;
    let chiavi = circuito.chiavi_salto;

    let guardiano = percorso_rete[0].0;
    let locale: SocketAddr = if guardiano.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse()?;
    let mut endpoint = quinn::Endpoint::client(locale)?;
    endpoint.set_default_client_config(crate::tls::config_client_insicuro()?);
    let connessione = endpoint.connect(guardiano, "nyctalus.invalid")?.await?;
    let (mut invio, mut ricezione) = connessione.open_bi().await?;

    // Apertura + richiesta di connessione (primo quadro dati, seq 0).
    scrivi_quadro(&mut invio, SEQ_APERTURA, &circuito.pacchetto).await?;
    invia_avvolto(&mut invio, 0, &chiavi, destinazione.as_bytes()).await?;

    // Esito dall'uscita.
    let esito = match leggi_quadro(&mut ricezione).await? {
        Some((seq, mut payload)) => {
            for k in &chiavi {
                cipolla::sbuccia(seq, k, &mut payload);
            }
            payload == b"OK"
        }
        None => false,
    };
    let codice = if esito { Esito::Riuscito } else { Esito::ConnessioneRifiutata };
    cliente.write_all(&socks5::risposta_richiesta(codice)).await?;
    if !esito {
        return Err(format!("l'uscita non ha raggiunto {destinazione}").into());
    }

    // Tramite bidirezionale attraverso il circuito.
    let (mut app_lettura, mut app_scrittura) = cliente.into_split();
    let andata = async {
        let mut buffer = vec![0u8; 16 * 1024];
        let mut seq: u64 = 1; // 0 usato per la richiesta di connessione
        loop {
            let n = app_lettura.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            invia_avvolto(&mut invio, seq, &chiavi, &buffer[..n]).await?;
            seq += 1;
        }
        let _ = invio.finish();
        Risultato::Ok(())
    };
    let ritorno = async {
        while let Some((seq, mut payload)) = leggi_quadro(&mut ricezione).await? {
            for k in &chiavi {
                cipolla::sbuccia(seq, k, &mut payload);
            }
            app_scrittura.write_all(&payload).await?;
        }
        let _ = app_scrittura.shutdown().await;
        Risultato::Ok(())
    };
    let (a, b) = tokio::join!(andata, ritorno);
    a?;
    b?;
    Ok(())
}

async fn invia_avvolto(
    invio: &mut quinn::SendStream,
    seq: u64,
    chiavi: &[[u8; 32]],
    dati: &[u8],
) -> Risultato<()> {
    let avvolto = cipolla::avvolgi(seq, dati, chiavi);
    scrivi_quadro(invio, seq, &avvolto).await
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
            return Err("il client SOCKS ha chiuso durante la negoziazione".into());
        }
        buffer.extend_from_slice(&pezzo[..n]);
    }
}
