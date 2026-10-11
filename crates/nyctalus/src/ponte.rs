// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Ponte verso Tor (punto I): quando la destinazione è un indirizzo `.onion`,
//! l'uscita del circuito la raggiunge attraverso **un Tor già presente sul
//! sistema**, tramite il suo proxy SOCKS5 (di norma `127.0.0.1:9050`). Per gli
//! altri indirizzi apre una normale connessione TCP.
//!
//! Questa scelta (invece di incorporare il client Tor Arti) tiene la build
//! leggera e riusa il Tor del sistema — per esempio quello di NexusSec. Il
//! Tor risolve e instrada l'indirizzo `.onion`; dal ponte in poi valgono le
//! regole (e la lentezza) di Tor.

use std::net::SocketAddr;
use std::sync::OnceLock;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::Risultato;

/// Un canale bidirezionale verso la destinazione (TCP diretto o tunnel Tor).
pub trait Duplex: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Duplex for T {}

static TOR_SOCKS: OnceLock<SocketAddr> = OnceLock::new();

/// Imposta l'indirizzo del proxy SOCKS del Tor di sistema (una volta, all'avvio
/// del nodo). Se non impostato vale `127.0.0.1:9050`.
pub fn imposta_tor_socks(indirizzo: SocketAddr) {
    let _ = TOR_SOCKS.set(indirizzo);
}

fn tor_socks() -> SocketAddr {
    *TOR_SOCKS.get().unwrap_or(&SocketAddr::from(([127, 0, 0, 1], 9050)))
}

/// Vero se la destinazione `host:porta` è un servizio `.onion`.
pub fn e_onion(destinazione: &str) -> bool {
    let host = destinazione.rsplit_once(':').map_or(destinazione, |(h, _)| h);
    host.trim_end_matches('.').to_ascii_lowercase().ends_with(".onion")
}

/// Apre la connessione verso la destinazione: via Tor se è un `.onion`, via TCP
/// diretto altrimenti.
pub async fn connetti(destinazione: &str) -> Risultato<Box<dyn Duplex>> {
    if e_onion(destinazione) {
        Ok(Box::new(tunnel_tor(destinazione).await?))
    } else {
        Ok(Box::new(TcpStream::connect(destinazione).await?))
    }
}

/// Apre un tunnel verso `host.onion:porta` tramite il proxy SOCKS5 di Tor.
async fn tunnel_tor(destinazione: &str) -> Risultato<TcpStream> {
    let (host, porta) = destinazione.rsplit_once(':').ok_or("indirizzo .onion senza porta")?;
    let porta: u16 = porta.parse()?;
    if host.len() > 255 {
        return Err("nome .onion troppo lungo".into());
    }

    let socks = tor_socks();
    let mut s = TcpStream::connect(socks)
        .await
        .map_err(|e| format!("Tor non raggiungibile su {socks} (è avviato?): {e}"))?;

    // Saluto SOCKS5: nessuna autenticazione.
    s.write_all(&[0x05, 0x01, 0x00]).await?;
    let mut risposta = [0u8; 2];
    s.read_exact(&mut risposta).await?;
    if risposta != [0x05, 0x00] {
        return Err("Tor SOCKS: metodo di autenticazione non accettato".into());
    }

    // CONNECT verso un nome di dominio (l'.onion lo risolve Tor).
    let mut richiesta = vec![0x05, 0x01, 0x00, 0x03, host.len() as u8];
    richiesta.extend_from_slice(host.as_bytes());
    richiesta.extend_from_slice(&porta.to_be_bytes());
    s.write_all(&richiesta).await?;

    // Risposta: [VER, REP, RSV, ATYP, BND.ADDR, BND.PORT].
    let mut testa = [0u8; 4];
    s.read_exact(&mut testa).await?;
    if testa[1] != 0x00 {
        return Err(format!("Tor non ha raggiunto {destinazione} (codice SOCKS {})", testa[1]).into());
    }
    let da_scartare = match testa[3] {
        0x01 => 4,
        0x04 => 16,
        0x03 => {
            let mut l = [0u8; 1];
            s.read_exact(&mut l).await?;
            l[0] as usize
        }
        _ => return Err("tipo di indirizzo nella risposta SOCKS non valido".into()),
    };
    let mut resto = vec![0u8; da_scartare + 2];
    s.read_exact(&mut resto).await?;
    Ok(s)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn riconosce_gli_onion() {
        assert!(e_onion("abcdefghij234567.onion:80"));
        assert!(e_onion("ServiceXYZ.ONION:443"));
        assert!(!e_onion("example.com:80"));
        assert!(!e_onion("127.0.0.1:8080"));
        assert!(!e_onion("onion.example.com:80"));
    }
}
