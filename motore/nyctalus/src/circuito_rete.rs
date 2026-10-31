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

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::sync::Mutex;

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

/// Nodi dell'elenco annunciati con un NOME (es. `miacasa.ddns.net:4601`, IP dinamico): per
/// loro il «prossimo salto» nel pacchetto Sphinx non è l'IP ma la chiave X25519 del nodo, che
/// il nodo precedente cerca nell'elenco firmato e risolve al momento (l'IP può cambiare).
static PER_NOME: Mutex<Option<HashSet<[u8; 32]>>> = Mutex::new(None);

/// Tipo dell'indirizzo «per chiave»: `[6][primi 31 byte della chiave X25519]`.
pub const PER_CHIAVE: u8 = 6;

/// Indirizzo di una voce dell'elenco: IP così com'è, oppure un nome risolto ORA (si prende il
/// primo indirizzo IPv4). Le voci con un nome vengono ricordate per codificarle «per chiave».
pub fn risolvi_voce(indirizzo: &str, x25519: &[u8; 32]) -> Risultato<SocketAddr> {
    if let Ok(a) = indirizzo.parse::<SocketAddr>() {
        return Ok(a);
    }
    let a = indirizzo
        .to_socket_addrs()
        .map_err(|e| format!("{indirizzo}: nome non risolto ({e})"))?
        .find(SocketAddr::is_ipv4)
        .ok_or_else(|| format!("{indirizzo}: nessun indirizzo IPv4"))?;
    PER_NOME.lock().unwrap().get_or_insert_with(HashSet::new).insert(*x25519);
    Ok(a)
}

/// Il «prossimo salto» di un nodo del percorso: per IP, o per chiave se annunciato per nome.
pub fn codifica_nodo(indirizzo: SocketAddr, x25519: &[u8; 32]) -> Risultato<[u8; 32]> {
    if PER_NOME.lock().unwrap().as_ref().is_some_and(|s| s.contains(x25519)) {
        let mut byte = [0u8; 32];
        byte[0] = PER_CHIAVE;
        byte[1..].copy_from_slice(&x25519[..31]);
        return Ok(byte);
    }
    codifica_indirizzo(indirizzo)
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
