// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Trasporto QUIC, con camuffamento opzionale (punto H).
//!
//! Quando è attivo l'offuscamento, il socket UDP viene avvolto: ogni datagramma
//! in uscita è reso indistinguibile da rumore ([`nyctalus_core::offuscamento`])
//! e ogni datagramma in ingresso viene ripulito prima di arrivare a QUIC. La
//! chiave è condivisa da tutta la rete (come il "cert" di un bridge obfs4); un **ponte
//! nascosto** ne ha una sua per chi entra.
//!
//! **Resistenza alle sonde** (lato server): il camuffamento non autentica, quindi un pacchetto
//! con la chiave sbagliata arriva a QUIC come byte a caso, e QUIC a certi byte a caso risponde
//! (negoziazione di versione, reset). Chi sondava un indirizzo poteva così scoprire un nodo. Ora
//! da un indirizzo sconosciuto passa solo un datagramma che, ripulito, è un vero `Initial` di
//! QUIC v1 (intestazione lunga, versione 1, almeno 1200 byte): una sonda a caso ci riesce una
//! volta su ~68 miliardi; a tutto il resto il nodo **non risponde**. Dopo un `Initial` valido
//! quell'indirizzo **con la sua porta** resta ammesso per 10 minuti dall'ultimo pacchetto. Non
//! l'IP da solo: chi sta dietro lo stesso NAT di un utente vero (o sullo stesso computer) potrebbe
//! sondare da un'altra porta (trovato dalla prova 32). Il segnale di vita di QUIC ogni 5 s tiene
//! ferma la porta nei router.

use std::io::{self, IoSliceMut};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::task::{Context, Poll};

use nyctalus_core::offuscamento::{self, LUNGHEZZA_NONCE};
use quinn::udp::{RecvMeta, Transmit};
use quinn::{
    AsyncUdpSocket, Endpoint, EndpointConfig, Runtime, ServerConfig, UdpPoller, default_runtime,
};

use crate::Risultato;

/// Crea un endpoint server, con offuscamento se è data la chiave di rete.
pub fn endpoint_server(
    server_config: ServerConfig,
    ascolta: SocketAddr,
    offusca: Option<[u8; 32]>,
) -> Risultato<Endpoint> {
    match offusca {
        None => Ok(Endpoint::server(server_config, ascolta)?),
        Some(segreto) => {
            let (socket, runtime) = socket_offuscato(ascolta, segreto, true)?;
            Ok(Endpoint::new_with_abstract_socket(EndpointConfig::default(), Some(server_config), socket, runtime)?)
        }
    }
}

/// Crea un endpoint client, con offuscamento se è data la chiave di rete.
pub fn endpoint_client(locale: SocketAddr, offusca: Option<[u8; 32]>) -> Risultato<Endpoint> {
    match offusca {
        None => Ok(Endpoint::client(locale)?),
        Some(segreto) => {
            let (socket, runtime) = socket_offuscato(locale, segreto, false)?;
            Ok(Endpoint::new_with_abstract_socket(EndpointConfig::default(), None, socket, runtime)?)
        }
    }
}

fn socket_offuscato(
    indirizzo: SocketAddr,
    segreto_rete: [u8; 32],
    server: bool,
) -> Risultato<(Arc<SocketOffuscato>, Arc<dyn Runtime>)> {
    let runtime = default_runtime().ok_or("nessun runtime asincrono disponibile")?;
    let udp = UdpSocket::bind(indirizzo)?;
    udp.set_nonblocking(true)?;
    let inner = runtime.wrap_udp_socket(udp)?;
    let chiave = offuscamento::chiave_offuscamento(&segreto_rete);
    let ammessi = server.then(|| Mutex::new(HashMap::new()));
    Ok((Arc::new(SocketOffuscato { inner, chiave, ammessi }), runtime))
}

/// Socket UDP che offusca ogni datagramma. Il GSO è disabilitato
/// (`max_transmit_segments` = 1) così ogni invio è un singolo datagramma.
#[derive(Debug)]
struct SocketOffuscato {
    inner: Arc<dyn AsyncUdpSocket>,
    chiave: [u8; 32],
    /// solo lato server: indirizzi che hanno aperto un collegamento valido, con l'ultimo contatto
    ammessi: Option<Mutex<HashMap<SocketAddr, Instant>>>,
}

/// Per quanto un indirizzo resta ammesso dopo l'ultimo pacchetto.
const AMMESSO_PER: Duration = Duration::from_secs(600);
/// Tetto della tabella (oltre si tolgono i scaduti; se non basta, i più vecchi).
const AMMESSI_MAX: usize = 50_000;
/// Un `Initial` del client QUIC è sempre riempito fino ad almeno 1200 byte (RFC 9000 §14.1).
const INITIAL_MIN: usize = 1200;

/// Il datagramma (già ripulito) è l'apertura di un collegamento QUIC v1?
pub fn e_initial_quic(d: &[u8]) -> bool {
    d.len() >= INITIAL_MIN
        && d[0] & 0xc0 == 0xc0              // intestazione lunga + bit fisso
        && (d[0] >> 4) & 0x03 == 0          // tipo Initial
        && d[1..5] == [0, 0, 0, 1]          // versione 1
}

impl SocketOffuscato {
    /// Lato server: il datagramma ripulito `d` arrivato da `da` va passato a QUIC?
    fn ammetti(&self, da: SocketAddr, d: &[u8]) -> bool {
        let Some(tabella) = &self.ammessi else { return true };
        let ora = Instant::now();
        let mut t = tabella.lock().unwrap_or_else(|e| e.into_inner());
        let ip = da;
        let gia = t.get(&ip).is_some_and(|ultimo| ora.duration_since(*ultimo) < AMMESSO_PER);
        if !gia && !e_initial_quic(d) {
            return false;
        }
        if !gia && t.len() >= AMMESSI_MAX {
            t.retain(|_, ultimo| ora.duration_since(*ultimo) < AMMESSO_PER);
            if t.len() >= AMMESSI_MAX {
                if let Some(vecchio) = t.iter().min_by_key(|(_, u)| **u).map(|(k, _)| *k) {
                    t.remove(&vecchio);
                }
            }
        }
        t.insert(ip, ora);
        true
    }
}

impl AsyncUdpSocket for SocketOffuscato {
    fn create_io_poller(self: Arc<Self>) -> Pin<Box<dyn UdpPoller>> {
        self.inner.clone().create_io_poller()
    }

    fn try_send(&self, transmit: &Transmit) -> io::Result<()> {
        let mut nonce = [0u8; LUNGHEZZA_NONCE];
        getrandom::fill(&mut nonce).map_err(io::Error::other)?;
        let offuscato = offuscamento::offusca(&self.chiave, nonce, transmit.contents);
        self.inner.try_send(&Transmit {
            destination: transmit.destination,
            ecn: transmit.ecn,
            contents: &offuscato,
            segment_size: None,
            src_ip: transmit.src_ip,
        })
    }

    fn poll_recv(
        &self,
        cx: &mut Context,
        bufs: &mut [IoSliceMut<'_>],
        meta: &mut [RecvMeta],
    ) -> Poll<io::Result<usize>> {
        // Buffer d'appoggio per i datagrammi ancora offuscati.
        let mut appoggio: Vec<Vec<u8>> = bufs.iter().map(|b| vec![0u8; b.len()]).collect();
        let quanti = {
            let mut fette: Vec<IoSliceMut> = appoggio.iter_mut().map(|v| IoSliceMut::new(v)).collect();
            match self.inner.poll_recv(cx, &mut fette, meta) {
                Poll::Ready(Ok(n)) => n,
                altro => return altro,
            }
        };
        for i in 0..quanti {
            let len = meta[i].len;
            match offuscamento::deoffusca(&self.chiave, &appoggio[i][..len])
                .filter(|chiaro| self.ammetti(meta[i].addr, chiaro))
            {
                Some(chiaro) => {
                    let m = chiaro.len().min(bufs[i].len());
                    bufs[i][..m].copy_from_slice(&chiaro[..m]);
                    meta[i].len = m;
                    meta[i].stride = m;
                }
                None => {
                    meta[i].len = 0;
                    meta[i].stride = 0;
                }
            }
        }
        Poll::Ready(Ok(quanti))
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    fn max_transmit_segments(&self) -> usize {
        1
    }

    fn max_receive_segments(&self) -> usize {
        1
    }

    fn may_fragment(&self) -> bool {
        self.inner.may_fragment()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn riconosce_solo_l_apertura_quic_v1() {
        let mut d = vec![0u8; 1200];
        d[0] = 0xc3;
        d[1..5].copy_from_slice(&[0, 0, 0, 1]);
        assert!(e_initial_quic(&d));
        assert!(!e_initial_quic(&d[..1199]), "troppo corto: un Initial vero è riempito a 1200");
        let mut v = d.clone();
        v[4] = 2;
        assert!(!e_initial_quic(&v), "versione sconosciuta");
        let mut h = d.clone();
        h[0] = 0xe3;                                  // intestazione lunga ma tipo Handshake
        assert!(!e_initial_quic(&h));
        let mut c = d.clone();
        c[0] = 0x43;                                  // intestazione corta
        assert!(!e_initial_quic(&c));
        // byte a caso: quasi mai (1 su ~68 miliardi)
        let mut passati = 0;
        for _ in 0..20_000 {
            let mut r = vec![0u8; 1300];
            getrandom::fill(&mut r).unwrap();
            passati += e_initial_quic(&r) as u32;
        }
        assert_eq!(passati, 0);
    }
}
