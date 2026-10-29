// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Elementi comuni al nodo e al client per un circuito sulla rete:
//! codifica degli indirizzi dei nodi e formato dei "quadri" (frame) sullo
//! stream QUIC di un circuito.
//!
//! Formato di un quadro: `[lunghezza u32 BE][payload]`. Nient'altro viaggia in
//! chiaro: il **primo** quadro dello stream è il pacchetto Sphinx di apertura,
//! i successivi sono pacchetti dati.
//!
//! **Anti-correlazione (punto C):** il numero di sequenza usato come nonce
//! dagli strati a cipolla NON è sul filo. Essendo lo stream QUIC ordinato e
//! affidabile, ogni nodo lo ricostruisce con un **contatore locale** (uno per
//! direzione). Così lo stesso pacchetto non mostra alcun valore comune fra una
//! tratta e l'altra. Il bit [`BIT_RISPOSTA`] distingue la direzione di ritorno
//! nel nonce, così andata e ritorno non riusano mai lo stesso keystream.
//! (L'ID di circuito esplicito per tratta arriverà con la multiplazione, punto D.)

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::Risultato;

/// Bit di direzione nel nonce: impostato per i pacchetti di ritorno (uscita → client).
pub const BIT_RISPOSTA: u64 = 1 << 63;

/// Codifica un indirizzo IPv4 nei 32 byte usati da Sphinx come indirizzo di
/// nodo: `[4][octetti(4)][porta(2 BE)][0…]`.
pub fn codifica_indirizzo(indirizzo: SocketAddr) -> Risultato<[u8; 32]> {
    let IpAddr::V4(ip) = indirizzo.ip() else {
        return Err("the circuit prototype only supports IPv4".into());
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
        return Err("node address is not IPv4".into());
    }
    let ip = Ipv4Addr::new(byte[1], byte[2], byte[3], byte[4]);
    let porta = u16::from_be_bytes([byte[5], byte[6]]);
    Ok(SocketAddr::from((ip, porta)))
}

/// Legge un quadro (solo il payload). `Ok(None)` se lo stream è terminato.
pub async fn leggi_quadro<R: AsyncRead + Unpin>(stream: &mut R) -> Risultato<Option<Vec<u8>>> {
    let lunghezza = match stream.read_u32().await {
        Ok(v) => v as usize,
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if lunghezza > 1 << 20 {
        return Err("frame too large".into());
    }
    let mut payload = vec![0u8; lunghezza];
    stream.read_exact(&mut payload).await?;
    Ok(Some(payload))
}

/// Scrive un quadro.
pub async fn scrivi_quadro<W: AsyncWrite + Unpin>(stream: &mut W, payload: &[u8]) -> Risultato<()> {
    stream.write_u32(payload.len() as u32).await?;
    stream.write_all(payload).await?;
    Ok(())
}
