// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Trasporto QUIC, con camuffamento opzionale (punto H).
//!
//! Quando è attivo l'offuscamento, il socket UDP viene avvolto: ogni datagramma
//! in uscita è reso indistinguibile da rumore ([`nyctalus_core::offuscamento`])
//! e ogni datagramma in ingresso viene ripulito prima di arrivare a QUIC. La
//! chiave è condivisa da tutta la rete (come il "cert" di un bridge obfs4).

use std::io::{self, IoSliceMut};
use std::net::{SocketAddr, UdpSocket};
use std::pin::Pin;
use std::sync::Arc;
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
            let (socket, runtime) = socket_offuscato(ascolta, segreto)?;
            Ok(Endpoint::new_with_abstract_socket(EndpointConfig::default(), Some(server_config), socket, runtime)?)
        }
    }
}

/// Crea un endpoint client, con offuscamento se è data la chiave di rete.
pub fn endpoint_client(locale: SocketAddr, offusca: Option<[u8; 32]>) -> Risultato<Endpoint> {
    match offusca {
        None => Ok(Endpoint::client(locale)?),
        Some(segreto) => {
            let (socket, runtime) = socket_offuscato(locale, segreto)?;
            Ok(Endpoint::new_with_abstract_socket(EndpointConfig::default(), None, socket, runtime)?)
        }
    }
}

fn socket_offuscato(
    indirizzo: SocketAddr,
    segreto_rete: [u8; 32],
) -> Risultato<(Arc<SocketOffuscato>, Arc<dyn Runtime>)> {
    let runtime = default_runtime().ok_or("nessun runtime asincrono disponibile")?;
    let udp = UdpSocket::bind(indirizzo)?;
    udp.set_nonblocking(true)?;
    let inner = runtime.wrap_udp_socket(udp)?;
    let chiave = offuscamento::chiave_offuscamento(&segreto_rete);
    Ok((Arc::new(SocketOffuscato { inner, chiave }), runtime))
}

/// Socket UDP che offusca ogni datagramma. Il GSO è disabilitato
/// (`max_transmit_segments` = 1) così ogni invio è un singolo datagramma.
#[derive(Debug)]
struct SocketOffuscato {
    inner: Arc<dyn AsyncUdpSocket>,
    chiave: [u8; 32],
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
            match offuscamento::deoffusca(&self.chiave, &appoggio[i][..len]) {
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
