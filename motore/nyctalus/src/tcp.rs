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
//! **Forma HTTP/3** della rete: il TCP è un **vero TLS** (sembra un normale sito HTTPS, ALPN
//! http/1.1, nome scelto dal gestore). Dentro TLS il client manda per primo il contrassegno della
//! rete; chi non lo ha (un censore che sonda, o un browser) riceve una normale risposta HTTP «404
//! Not Found» e la connessione si chiude, come da un sito qualunque. TLS è rustls guidato a mano
//! (`tubo_tls`): nessuna libreria in più.
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
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
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
fn servi_connessione<S: AsyncRead + AsyncWrite + Send + 'static>(
    flusso: S,
    chi: SocketAddr,
    chiave: Option<[u8; 32]>,
    lato_nodo: bool,
    entrata: mpsc::UnboundedSender<(SocketAddr, Vec<u8>)>,
    uscite: Option<Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<Vec<u8>>>>>>,
) -> mpsc::Sender<Vec<u8>> {
    let (mut lettura, mut scrittura) = tokio::io::split(flusso);
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

/// Risposta di facciata a chi entra in TLS senza contrassegno: quella di un sito qualunque.
const RISPOSTA_404: &[u8] = b"HTTP/1.1 404 Not Found\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: 153\r\nConnection: close\r\n\r\n<!DOCTYPE html>\n<html><head><title>404 Not Found</title></head>\n<body><h1>Not Found</h1>\n<p>The requested URL was not found on this server.</p></body></html>\n";

/// TLS guidato a mano sopra il TCP: fa la stretta di mano, poi restituisce un canale interno
/// in chiaro (il resto del ripiego lo usa come se fosse il TCP).
async fn tubo_tls(tcp: TcpStream, mut conn: rustls::Connection) -> io::Result<tokio::io::DuplexStream> {
    let _ = tcp.set_nodelay(true);
    let (mut r, mut w) = tcp.into_split();
    let mut buf = vec![0u8; 16384];
    let scrivi_tls = |c: &mut rustls::Connection| {
        let mut o = Vec::new();
        while c.wants_write() {
            if c.write_tls(&mut o).is_err() {
                break;
            }
        }
        o
    };
    while conn.is_handshaking() {
        let o = scrivi_tls(&mut conn);
        if !o.is_empty() {
            w.write_all(&o).await?;
        }
        if !conn.is_handshaking() {
            break;
        }
        if conn.wants_read() {
            let n = r.read(&mut buf).await?;
            if n == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "TLS closed during handshake"));
            }
            conn.read_tls(&mut &buf[..n])?;
            conn.process_new_packets().map_err(io::Error::other)?;
        }
    }
    let o = scrivi_tls(&mut conn);
    if !o.is_empty() {
        w.write_all(&o).await?;
    }
    // dati arrivati insieme alla fine della stretta di mano (es. curl manda la richiesta nello
    // stesso pacchetto): rustls li ha già decifrati, vanno consegnati subito
    let mut gia_arrivati = Vec::new();
    let mut tmp = [0u8; 4096];
    while let Ok(k) = conn.reader().read(&mut tmp) {
        if k == 0 {
            break;
        }
        gia_arrivati.extend_from_slice(&tmp[..k]);
    }
    let (interno, esterno) = tokio::io::duplex(1 << 16);
    let (mut er, mut ew) = tokio::io::split(esterno);
    if !gia_arrivati.is_empty() {
        ew.write_all(&gia_arrivati).await?;
    }
    let conn = Arc::new(Mutex::new(conn));
    let (tx_tls, mut rx_tls) = mpsc::unbounded_channel::<Vec<u8>>();
    // in arrivo: TCP → TLS → chiaro
    let c1 = conn.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 16384];
        let mut chiaro = vec![0u8; 16384];
        loop {
            let n = match r.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let (dati, uscita) = {
                let mut c = c1.lock().unwrap();
                if c.read_tls(&mut &buf[..n]).is_err() || c.process_new_packets().is_err() {
                    break;
                }
                let mut d = Vec::new();
                while let Ok(k) = c.reader().read(&mut chiaro) {
                    if k == 0 {
                        break;
                    }
                    d.extend_from_slice(&chiaro[..k]);
                }
                (d, scrivi_tls(&mut c))
            };
            if !uscita.is_empty() {
                let _ = tx_tls.send(uscita);
            }
            if !dati.is_empty() && ew.write_all(&dati).await.is_err() {
                break;
            }
        }
        let _ = ew.shutdown().await;
    });
    // in partenza: chiaro → TLS → TCP
    tokio::spawn(async move {
        let mut buf = vec![0u8; 16384];
        loop {
            tokio::select! {
                letto = er.read(&mut buf) => {
                    let n = match letto { Ok(0) | Err(_) => break, Ok(n) => n };
                    let o = {
                        let mut c = conn.lock().unwrap();
                        if c.writer().write_all(&buf[..n]).is_err() {
                            break;
                        }
                        scrivi_tls(&mut c)
                    };
                    if w.write_all(&o).await.is_err() {
                        break;
                    }
                }
                Some(o) = rx_tls.recv() => {
                    if w.write_all(&o).await.is_err() {
                        break;
                    }
                }
            }
        }
        let o = {
            let mut c = conn.lock().unwrap();
            c.send_close_notify();
            scrivi_tls(&mut c)
        };
        let _ = w.write_all(&o).await;
        let _ = w.shutdown().await;
    });
    Ok(interno)
}

/// Lato nodo, forma HTTP/3: stretta TLS, contrassegno; senza contrassegno, la pagina 404.
async fn accogli_tls(
    tcp: TcpStream,
    tls: Arc<rustls::ServerConfig>,
    chiave: Option<[u8; 32]>,
) -> Option<tokio::io::DuplexStream> {
    let conn = rustls::Connection::Server(rustls::ServerConnection::new(tls).ok()?);
    let mut interno = tokio::time::timeout(std::time::Duration::from_secs(15), tubo_tls(tcp, conn)).await.ok()?.ok()?;
    let atteso = chiave.map(|k| crate::trasporto::contrassegno(&k));
    let mut letto = [0u8; crate::trasporto::CONTRASSEGNO];
    let pieno = tokio::time::timeout(std::time::Duration::from_secs(5), interno.read_exact(&mut letto)).await.is_ok_and(|r| r.is_ok());
    if pieno && atteso.is_some_and(|a| a == letto) {
        return Some(interno);
    }
    // non è dei nostri: si legge la sua richiesta e si risponde come un sito qualunque
    let mut richiesta = letto.to_vec();
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let mut b = [0u8; 2048];
        while !richiesta.windows(4).any(|w| w == b"\r\n\r\n") && richiesta.len() < 16384 {
            match interno.read(&mut b).await {
                Ok(0) | Err(_) => break,
                Ok(k) => richiesta.extend_from_slice(&b[..k]),
            }
        }
    })
    .await;
    let _ = interno.write_all(RISPOSTA_404).await;
    let _ = interno.shutdown().await;
    None
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
    pub fn nodo(
        udp: Arc<dyn AsyncUdpSocket>,
        ascolta: SocketAddr,
        chiave: Option<[u8; 32]>,
        tls: Option<Arc<rustls::ServerConfig>>,
    ) -> Arc<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let uscite: Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<Vec<u8>>>>> = Arc::new(Mutex::new(HashMap::new()));
        let locale = udp.local_addr().unwrap_or(ascolta);
        match std::net::TcpListener::bind(locale).and_then(|l| {
            l.set_nonblocking(true)?;
            tokio::net::TcpListener::from_std(l)
        }) {
            Ok(ascolto) => {
                let u = uscite.clone();
                let in_corso = Arc::new(AtomicUsize::new(0));
                tokio::spawn(async move {
                    while let Ok((tcp, chi)) = ascolto.accept().await {
                        if u.lock().unwrap().len() + in_corso.load(Ordering::Relaxed) >= CONNESSIONI_MAX {
                            continue; // troppe: questa si lascia cadere
                        }
                        let _ = tcp.set_nodelay(true);
                        match tls.clone() {
                            None => {
                                let scrittore = servi_connessione(tcp, chi, chiave, true, tx.clone(), Some(u.clone()));
                                u.lock().unwrap().insert(chi, scrittore);
                            }
                            Some(tls) => {
                                // forma HTTP/3: TLS e contrassegno (dentro TLS le lunghezze non servono cifrate)
                                let (u, tx, in_corso) = (u.clone(), tx.clone(), in_corso.clone());
                                in_corso.fetch_add(1, Ordering::Relaxed);
                                tokio::spawn(async move {
                                    let interno = accogli_tls(tcp, tls, chiave).await;
                                    in_corso.fetch_sub(1, Ordering::Relaxed);
                                    if let Some(interno) = interno {
                                        let scrittore = servi_connessione(interno, chi, None, true, tx, Some(u.clone()));
                                        u.lock().unwrap().insert(chi, scrittore);
                                    }
                                });
                            }
                        }
                    }
                });
            }
            Err(e) => eprintln!("WARNING: TCP fallback not available on {locale}: {e}"),
        }
        Arc::new(Self { udp: Some(udp), locale, arrivi: Mutex::new(rx), uscite })
    }

    /// Lato client: solo TCP, verso `destinazione`.
    pub async fn client(
        destinazione: SocketAddr,
        chiave: Option<[u8; 32]>,
        tls: Option<(Arc<rustls::ClientConfig>, rustls::pki_types::ServerName<'static>)>,
    ) -> io::Result<Arc<Self>> {
        let tcp = tokio::time::timeout(std::time::Duration::from_secs(10), TcpStream::connect(destinazione))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "TCP connect timeout"))??;
        let _ = tcp.set_nodelay(true);
        let locale = tcp.local_addr()?;
        let (tx, rx) = mpsc::unbounded_channel();
        let scrittore = match tls {
            None => servi_connessione(tcp, destinazione, chiave, false, tx, None),
            Some((cfg, nome)) => {
                // forma HTTP/3: un vero TLS verso il «sito», poi il contrassegno della rete
                let conn = rustls::Connection::Client(rustls::ClientConnection::new(cfg, nome).map_err(io::Error::other)?);
                let mut interno = tubo_tls(tcp, conn).await?;
                let k = chiave.ok_or_else(|| io::Error::other("HTTP/3 form needs the network key"))?;
                interno.write_all(&crate::trasporto::contrassegno(&k)).await?;
                servi_connessione(interno, destinazione, None, false, tx, None)
            }
        };
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
