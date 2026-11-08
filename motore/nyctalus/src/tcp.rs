// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! **Ripiego su TCP** (PIANO-ANONIMATO, invisibilità punto 2b): dove UDP è bloccato (alcune reti
//! aziendali, alcuni Wi-Fi pubblici, alcuni paesi) i pacchetti QUIC viaggiano dentro un
//! collegamento TCP verso l'entrata. Sopra non cambia niente: QUIC, circuiti, cipolla, ragnatela,
//! camuffamento e filtro anti-sonda restano gli stessi (questo è solo un altro «socket» per quinn).
//!
//! - **Nodo**: socket ibrido, UDP come sempre più le connessioni TCP sulla stessa porta; per quinn
//!   ogni connessione TCP è un indirizzo come un altro.
//! - **Client**: socket solo TCP verso l'entrata (quando UDP non risponde, o se forzato).
//!
//! Ogni datagramma va in un quadro «lunghezza (2 byte) + dati». Con la chiave della rete la
//! **lunghezza è cifrata** (flusso di chiave BLAKE3 per direzione): sul filo non si vede una
//! sequenza regolare di lunghezze, solo byte a caso. Costo onesto: su TCP un pacchetto perso
//! ferma tutto il collegamento (come in Tor); è un ripiego, non la strada normale.

use std::collections::HashMap;
use std::io::{self, IoSliceMut};
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use quinn::udp::{RecvMeta, Transmit};
use quinn::{AsyncUdpSocket, UdpPoller};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

/// Datagramma più grande accettato (QUIC sta sotto i 1500 byte; margine per il camuffamento).
const DATAGRAMMA_MAX: usize = 2048;
/// Datagrammi in coda verso una connessione TCP prima di scartarli (come farebbe UDP).
const CODA: usize = 512;
/// Connessioni TCP contemporanee al massimo per nodo (contro chi ne apre migliaia per intasarlo).
const CONNESSIONI_MAX: usize = 1024;

/// Flusso di chiave per cifrare le lunghezze dei quadri (una per direzione).
struct Lunghezze {
    lettore: Option<blake3::OutputReader>,
}

impl Lunghezze {
    fn nuove(chiave: Option<&[u8; 32]>, direzione: &str) -> Self {
        let lettore = chiave.map(|k| {
            let mut h = blake3::Hasher::new_derive_key("Nyctalus 2026-11-08 lunghezze dei quadri TCP");
            h.update(k);
            h.update(direzione.as_bytes());
            h.finalize_xof()
        });
        Self { lettore }
    }

    fn maschera(&mut self) -> [u8; 2] {
        let mut m = [0u8; 2];
        if let Some(l) = self.lettore.as_mut() {
            l.fill(&mut m);
        }
        m
    }
}

/// Avvia lettura e scrittura su una connessione TCP: i datagrammi letti vanno in `entrata` con
/// l'indirizzo `chi`; restituisce la coda per scriverci.
fn servi_connessione(
    tcp: TcpStream,
    chi: SocketAddr,
    chiave: Option<[u8; 32]>,
    lato_nodo: bool,
    entrata: mpsc::UnboundedSender<(SocketAddr, Vec<u8>)>,
    uscite: Option<Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<Vec<u8>>>>>>,
) -> mpsc::Sender<Vec<u8>> {
    let _ = tcp.set_nodelay(true);
    let (mut lettura, mut scrittura) = tcp.into_split();
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(CODA);
    let (verso_nodo, verso_client) = ("client→nodo", "nodo→client");
    let (dir_in, dir_out) = if lato_nodo { (verso_nodo, verso_client) } else { (verso_client, verso_nodo) };
    tokio::spawn(async move {
        let mut l = Lunghezze::nuove(chiave.as_ref(), dir_out);
        while let Some(d) = rx.recv().await {
            let m = l.maschera();
            let n = (d.len() as u16).to_be_bytes();
            let mut q = Vec::with_capacity(2 + d.len());
            q.extend_from_slice(&[n[0] ^ m[0], n[1] ^ m[1]]);
            q.extend_from_slice(&d);
            if scrittura.write_all(&q).await.is_err() {
                break;
            }
        }
    });
    tokio::spawn(async move {
        let mut l = Lunghezze::nuove(chiave.as_ref(), dir_in);
        let mut buf = vec![0u8; DATAGRAMMA_MAX];
        loop {
            let mut n = [0u8; 2];
            if lettura.read_exact(&mut n).await.is_err() {
                break;
            }
            let m = l.maschera();
            let len = u16::from_be_bytes([n[0] ^ m[0], n[1] ^ m[1]]) as usize;
            if len == 0 || len > DATAGRAMMA_MAX {
                // dati senza senso (chiave sbagliata o una sonda): NON si chiude subito, che sarebbe
                // un'impronta («chiude dopo pochi byte»); si legge e si butta in silenzio finché
                // chiude l'altro, come un server che ignora la richiesta
                let butta = async { while matches!(lettura.read(&mut buf).await, Ok(k) if k > 0) {} };
                let _ = tokio::time::timeout(std::time::Duration::from_secs(60), butta).await;
                break;
            }
            if lettura.read_exact(&mut buf[..len]).await.is_err() {
                break;
            }
            if entrata.send((chi, buf[..len].to_vec())).is_err() {
                break;
            }
        }
        if let Some(u) = uscite {
            u.lock().unwrap().remove(&chi);
        }
    });
    tx
}

/// Socket per quinn: UDP (se c'è) più datagrammi su TCP.
#[derive(Debug)]
pub struct SocketTcp {
    udp: Option<Arc<dyn AsyncUdpSocket>>,
    locale: SocketAddr,
    arrivi: Mutex<mpsc::UnboundedReceiver<(SocketAddr, Vec<u8>)>>,
    uscite: Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<Vec<u8>>>>>,
}

impl std::fmt::Debug for Lunghezze {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Lunghezze")
    }
}

impl SocketTcp {
    /// Lato nodo: `udp` più un ascolto TCP sulla stessa porta.
    pub fn nodo(udp: Arc<dyn AsyncUdpSocket>, ascolta: SocketAddr, chiave: Option<[u8; 32]>) -> Arc<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let uscite: Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<Vec<u8>>>>> = Arc::new(Mutex::new(HashMap::new()));
        let locale = udp.local_addr().unwrap_or(ascolta);
        match std::net::TcpListener::bind(locale).and_then(|l| {
            l.set_nonblocking(true)?;
            tokio::net::TcpListener::from_std(l)
        }) {
            Ok(ascolto) => {
                let u = uscite.clone();
                tokio::spawn(async move {
                    while let Ok((tcp, chi)) = ascolto.accept().await {
                        if u.lock().unwrap().len() >= CONNESSIONI_MAX {
                            continue; // troppe: questa si lascia cadere
                        }
                        let scrittore = servi_connessione(tcp, chi, chiave, true, tx.clone(), Some(u.clone()));
                        u.lock().unwrap().insert(chi, scrittore);
                    }
                });
            }
            Err(e) => eprintln!("WARNING: TCP fallback not available on {locale}: {e}"),
        }
        Arc::new(Self { udp: Some(udp), locale, arrivi: Mutex::new(rx), uscite })
    }

    /// Lato client: solo TCP, verso `destinazione`.
    pub async fn client(destinazione: SocketAddr, chiave: Option<[u8; 32]>) -> io::Result<Arc<Self>> {
        let tcp = tokio::time::timeout(std::time::Duration::from_secs(10), TcpStream::connect(destinazione))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "TCP connect timeout"))??;
        let locale = tcp.local_addr()?;
        let (tx, rx) = mpsc::unbounded_channel();
        let scrittore = servi_connessione(tcp, destinazione, chiave, false, tx, None);
        let uscite = Arc::new(Mutex::new(HashMap::from([(destinazione, scrittore)])));
        Ok(Arc::new(Self { udp: None, locale, arrivi: Mutex::new(rx), uscite }))
    }
}

#[derive(Debug)]
struct SempreScrivibile;
impl UdpPoller for SempreScrivibile {
    fn poll_writable(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl AsyncUdpSocket for SocketTcp {
    fn create_io_poller(self: Arc<Self>) -> Pin<Box<dyn UdpPoller>> {
        match &self.udp {
            Some(u) => u.clone().create_io_poller(),
            None => Box::pin(SempreScrivibile),
        }
    }

    fn try_send(&self, transmit: &Transmit) -> io::Result<()> {
        let verso_tcp = self.uscite.lock().unwrap().get(&transmit.destination).cloned();
        match (verso_tcp, &self.udp) {
            (Some(tx), _) => {
                // coda piena = come un datagramma perso: ci pensa QUIC a rispedire
                let _ = tx.try_send(transmit.contents.to_vec());
                Ok(())
            }
            (None, Some(u)) => u.try_send(transmit),
            (None, None) => Ok(()),
        }
    }

    fn poll_recv(&self, cx: &mut Context, bufs: &mut [IoSliceMut<'_>], meta: &mut [RecvMeta]) -> Poll<io::Result<usize>> {
        if let Some(u) = &self.udp {
            if let Poll::Ready(r) = u.poll_recv(cx, bufs, meta) {
                return Poll::Ready(r);
            }
        }
        match self.arrivi.lock().unwrap().poll_recv(cx) {
            Poll::Ready(Some((chi, d))) => {
                let n = d.len().min(bufs[0].len());
                bufs[0][..n].copy_from_slice(&d[..n]);
                meta[0] = RecvMeta { addr: chi, len: n, stride: n, ecn: None, dst_ip: None };
                Poll::Ready(Ok(1))
            }
            Poll::Ready(None) if self.udp.is_none() => Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "TCP closed"))),
            _ => Poll::Pending,
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.locale)
    }

    fn max_transmit_segments(&self) -> usize {
        1
    }

    fn max_receive_segments(&self) -> usize {
        1
    }

    fn may_fragment(&self) -> bool {
        false
    }
}
