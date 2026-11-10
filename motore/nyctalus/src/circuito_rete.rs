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

/// Tipo dell'indirizzo «per chiave» CON ritardo di mix: `[7][primi 29 byte della chiave][ms (2 BE)]`.
pub const PER_CHIAVE_MIX: u8 = 7;
/// Marca del ritardo di mix nell'indirizzo IPv4 (byte 7; i byte 8-9 sono i millisecondi).
const MARCA_MIX: u8 = b'M';
/// Ritardo massimo che un nodo accetta di applicare (oltre: lo si limita).
pub const RITARDO_MIX_MAX: std::time::Duration = std::time::Duration::from_secs(20);

/// **Chat da mixnet** (PIANO-ANONIMATO, invisibilità punto 3): aggiunge all'indirizzo del nodo
/// `i` l'istruzione per il nodo `i-1` di trattenere le celle di questo circuito per `ms`
/// millisecondi prima di inoltrarle (ritardo scelto dal mittente, come in Loopix). Solo il nodo
/// che sbuccia quello strato Sphinx la legge. I nodi più vecchi ignorano i byte in più
/// dell'indirizzo IPv4 (inoltrano subito).
pub fn con_ritardo(mut byte: [u8; 32], ms: u16) -> [u8; 32] {
    match byte[0] {
        4 => {
            byte[7] = MARCA_MIX;
            byte[8..10].copy_from_slice(&ms.to_be_bytes());
        }
        PER_CHIAVE => {
            byte[0] = PER_CHIAVE_MIX;
            byte[30..32].copy_from_slice(&ms.to_be_bytes());
        }
        _ => {}
    }
    byte
}

/// Il ritardo di mix chiesto a questo nodo, se c'è.
pub fn ritardo_di(byte: &[u8; 32]) -> Option<std::time::Duration> {
    let ms = match byte[0] {
        4 if byte[7] == MARCA_MIX => u16::from_be_bytes([byte[8], byte[9]]),
        PER_CHIAVE_MIX => u16::from_be_bytes([byte[30], byte[31]]),
        _ => return None,
    };
    (ms > 0).then(|| std::time::Duration::from_millis(ms as u64).min(RITARDO_MIX_MAX))
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

#[cfg(test)]
mod test_mix {
    use super::*;

    #[test]
    fn ritardo_di_mix_nell_indirizzo() {
        let a = codifica_indirizzo("81.10.0.1:4601".parse().unwrap()).unwrap();
        assert_eq!(ritardo_di(&a), None, "senza mix nessun ritardo");
        let m = con_ritardo(a, 1500);
        assert_eq!(ritardo_di(&m), Some(std::time::Duration::from_millis(1500)));
        assert_eq!(decodifica_indirizzo(&m).unwrap(), "81.10.0.1:4601".parse().unwrap(), "l'indirizzo resta lo stesso");
        assert_eq!(ritardo_di(&con_ritardo(a, 60000)), Some(RITARDO_MIX_MAX), "limitato");
        let mut k = [0u8; 32];
        k[0] = PER_CHIAVE;
        k[1..].copy_from_slice(&[9u8; 31]);
        let km = con_ritardo(k, 700);
        assert_eq!(km[0], PER_CHIAVE_MIX);
        assert_eq!(&km[1..30], &[9u8; 29]);
        assert_eq!(ritardo_di(&km), Some(std::time::Duration::from_millis(700)));
    }
}
