// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Elementi comuni al nodo e al client per un circuito sulla rete:
//! codifica degli indirizzi dei nodi e formato dei "quadri" (frame) sullo
//! stream QUIC di un circuito.
//!
//! Formato di un quadro: `[seq u64 BE][lunghezza u32 BE][payload]`.
//! - `seq` = [`SEQ_APERTURA`] → il payload è il pacchetto Sphinx di apertura;
//! - altrimenti `seq` è il numero di sequenza del pacchetto dati (usato come
//!   nonce dagli strati a cipolla). Il bit più alto ([`BIT_RISPOSTA`]) distingue
//!   la direzione di ritorno, così andata e ritorno non riusano mai lo stesso
//!   keystream.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::Risultato;

/// Sequenza riservata al pacchetto di apertura del circuito.
pub const SEQ_APERTURA: u64 = u64::MAX;
/// Bit di direzione: impostato nei quadri di ritorno (uscita → client).
pub const BIT_RISPOSTA: u64 = 1 << 63;

/// Codifica un indirizzo IPv4 nei 32 byte usati da Sphinx come indirizzo di
/// nodo: `[4][octetti(4)][porta(2 BE)][0…]`.
pub fn codifica_indirizzo(indirizzo: SocketAddr) -> Risultato<[u8; 32]> {
    let IpAddr::V4(ip) = indirizzo.ip() else {
        return Err("il prototipo del circuito supporta solo IPv4".into());
    };
    let mut byte = [0u8; 32];
    byte[0] = 4;
    byte[1..5].copy_from_slice(&ip.octets());
    byte[5..7].copy_from_slice(&indirizzo.port().to_be_bytes());
    Ok(byte)
}

/// Decodifica l'indirizzo prodotto da [`codifica_indirizzo`].
pub fn decodifica_indirizzo(byte: &[u8; 32]) -> Risultato<SocketAddr> {
    if byte[0] != 4 {
        return Err("indirizzo di nodo non IPv4".into());
    }
    let ip = Ipv4Addr::new(byte[1], byte[2], byte[3], byte[4]);
    let porta = u16::from_be_bytes([byte[5], byte[6]]);
    Ok(SocketAddr::from((ip, porta)))
}

/// Legge un quadro. `Ok(None)` se lo stream è terminato in modo pulito.
pub async fn leggi_quadro<R: AsyncRead + Unpin>(stream: &mut R) -> Risultato<Option<(u64, Vec<u8>)>> {
    let seq = match stream.read_u64().await {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let lunghezza = stream.read_u32().await? as usize;
    if lunghezza > 1 << 20 {
        return Err("quadro troppo grande".into());
    }
    let mut payload = vec![0u8; lunghezza];
    stream.read_exact(&mut payload).await?;
    Ok(Some((seq, payload)))
}

/// Scrive un quadro.
pub async fn scrivi_quadro<W: AsyncWrite + Unpin>(stream: &mut W, seq: u64, payload: &[u8]) -> Risultato<()> {
    stream.write_u64(seq).await?;
    stream.write_u32(payload.len() as u32).await?;
    stream.write_all(payload).await?;
    Ok(())
}
