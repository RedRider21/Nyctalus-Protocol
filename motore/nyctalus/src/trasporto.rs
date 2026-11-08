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
    AsyncUdpSocket, Endpoint, EndpointConfig, ServerConfig, UdpPoller, default_runtime,
};

use crate::Risultato;

// --- forma «HTTP/3» (trasporto mimetico, PIANO-ANONIMATO invisibilità punto 2) ----------------
//
// Invece del rumore puro, il traffico ha l'aspetto di un normale sito HTTP/3: QUIC in chiaro
// (niente offuscamento ChaCha20), stretta di mano con ALPN `h3`, nome del server scelto dal
// gestore, porta di solito 443. La chiave della rete NON cifra più i pacchetti: diventa un
// **contrassegno** di 16 byte che il client scrive all'inizio di ogni flusso, DENTRO la cifratura
// TLS (un osservatore non lo vede). Chi si collega senza contrassegno, come un censore che sonda,
// riceve una vera risposta HTTP/3 (404) e niente altro: sembra un sito web.

static MIMETICO: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();

/// Accende la forma HTTP/3 per tutto il processo (prima di creare endpoint e configurazioni).
/// `nome`: il nome del server mandato nel primo pacchetto (vuoto = nessun nome).
pub fn imposta_mimetico(nome: &str) {
    let _ = MIMETICO.set(Some(nome.trim().to_string()));
}

/// La forma HTTP/3 è accesa?
pub fn mimetico() -> bool {
    MIMETICO.get().is_some_and(|m| m.is_some())
}

/// Il nome del server per la forma HTTP/3, se il gestore ne ha scelto uno.
pub fn nome_mimetico() -> Option<String> {
    MIMETICO.get().and_then(|m| m.clone()).filter(|n| !n.is_empty())
}

/// Lunghezza del contrassegno.
pub const CONTRASSEGNO: usize = 16;

/// Contrassegno della rete (o del ponte) ricavato dalla sua chiave.
pub fn contrassegno(chiave: &[u8; 32]) -> [u8; CONTRASSEGNO] {
    let k = blake3::derive_key("Nyctalus 2026-11-07 contrassegno della forma HTTP/3", chiave);
    let mut c = [0u8; CONTRASSEGNO];
    c.copy_from_slice(&k[..CONTRASSEGNO]);
    c
}

/// Apre un flusso verso un nodo: nella forma HTTP/3 scrive per primo il contrassegno.
pub async fn apri_flusso(
    conn: &quinn::Connection,
    chiave: Option<[u8; 32]>,
) -> Risultato<(quinn::SendStream, quinn::RecvStream)> {
    let (mut invio, ricezione) = conn.open_bi().await?;
    if mimetico() {
        let k = chiave.ok_or("HTTP/3 form needs the network key (--obfuscation)")?;
        invio.write_all(&contrassegno(&k)).await?;
    }
    Ok((invio, ricezione))
}

/// Lato server, all'arrivo di un collegamento nella forma HTTP/3: come ogni server HTTP/3 apre il
/// flusso di controllo con le impostazioni (SETTINGS vuote). I nostri client lo ignorano.
pub fn apri_controllo_h3(conn: &quinn::Connection) {
    if !mimetico() {
        return;
    }
    let conn = conn.clone();
    tokio::spawn(async move {
        if let Ok(mut u) = conn.open_uni().await {
            // tipo di flusso 0x00 = controllo; quadro SETTINGS (0x04) lungo 0
            let _ = u.write_all(&[0x00, 0x04, 0x00]).await;
            // il flusso di controllo resta aperto per tutta la vita del collegamento
            std::mem::forget(u);
        }
    });
}

/// Lato server, su ogni flusso in arrivo: `true` se porta il contrassegno giusto (e allora i byte
/// letti erano solo quelli); altrimenti risponde come un sito HTTP/3 (404) e chiude il flusso.
pub async fn controlla_contrassegno(
    invio: &mut quinn::SendStream,
    ricezione: &mut quinn::RecvStream,
    chiave: Option<[u8; 32]>,
) -> bool {
    if !mimetico() {
        return true;
    }
    let atteso = chiave.map(|k| contrassegno(&k));
    let mut letto = [0u8; CONTRASSEGNO];
    let pieno = tokio::time::timeout(std::time::Duration::from_secs(5), ricezione.read_exact(&mut letto))
        .await
        .is_ok_and(|r| r.is_ok());
    if pieno && atteso.is_some_and(|a| a == letto) {
        return true;
    }
    // non è dei nostri: una normale risposta HTTP/3
    let _ = invio.write_all(&risposta_h3()).await;
    let _ = invio.finish();
    let _ = ricezione.stop(quinn::VarInt::from_u32(0x100)); // H3_NO_ERROR
    false
}

/// Una risposta HTTP/3 completa: HEADERS (:status 404, content-type text/html) + DATA.
pub fn risposta_h3() -> Vec<u8> {
    const CORPO: &[u8] = b"<!DOCTYPE html>\n<html><head><title>404 Not Found</title></head>\n<body><h1>Not Found</h1>\n<p>The requested URL was not found on this server.</p></body></html>\n";
    // QPACK: Required Insert Count 0, Base 0, poi campi dalla tabella statica (RFC 9204, appendice A):
    // 27 = ":status 404", 52 = "content-type text/html; charset=utf-8"
    let intestazioni = [0x00u8, 0x00, 0xc0 | 27, 0xc0 | 52];
    let mut v = vec![0x01, intestazioni.len() as u8];
    v.extend_from_slice(&intestazioni);
    v.push(0x00); // DATA
    v.extend_from_slice(&varint(CORPO.len() as u64));
    v.extend_from_slice(CORPO);
    v
}

fn varint(n: u64) -> Vec<u8> {
    match n {
        0..=63 => vec![n as u8],
        64..=16383 => ((n as u16) | 0x4000).to_be_bytes().to_vec(),
        _ => ((n as u32) | 0x8000_0000).to_be_bytes().to_vec(),
    }
}

/// Crea un endpoint server, con offuscamento se è data la chiave di rete.
pub fn endpoint_server(
    server_config: ServerConfig,
    ascolta: SocketAddr,
    offusca: Option<[u8; 32]>,
    tls_tcp: Option<Arc<rustls::ServerConfig>>,
) -> Risultato<Endpoint> {
    // UDP come sempre, più il ripiego su TCP sulla stessa porta (crate::tcp): un unico socket.
    // Nella forma HTTP/3 il TCP è un vero TLS (sembra HTTPS), altrimenti byte camuffati.
    let runtime = default_runtime().ok_or("nessun runtime asincrono disponibile")?;
    let udp = UdpSocket::bind(ascolta)?;
    udp.set_nonblocking(true)?;
    let ibrido: Arc<dyn AsyncUdpSocket> =
        crate::tcp::SocketTcp::nodo(runtime.wrap_udp_socket(udp)?, ascolta, offusca, tls_tcp.filter(|_| mimetico()));
    // nella forma HTTP/3 i pacchetti restano QUIC in chiaro: la chiave fa da contrassegno
    let socket: Arc<dyn AsyncUdpSocket> = match offusca.filter(|_| !mimetico()) {
        None => ibrido,
        Some(segreto) => avvolgi(ibrido, segreto, true),
    };
    Ok(Endpoint::new_with_abstract_socket(EndpointConfig::default(), Some(server_config), socket, runtime)?)
}

/// Endpoint client **su TCP** verso `destinazione` (ripiego dove UDP è bloccato).
pub async fn endpoint_client_tcp(
    destinazione: SocketAddr,
    offusca: Option<[u8; 32]>,
    impronta: Option<[u8; 32]>,
) -> Risultato<Endpoint> {
    let runtime = default_runtime().ok_or("nessun runtime asincrono disponibile")?;
    // forma HTTP/3: il TCP è un vero TLS verso il «sito» (nome del gestore), con l'impronta del nodo
    let tls = if mimetico() {
        let nome = rustls::pki_types::ServerName::try_from(crate::tls::nome_per(&destinazione))?.to_owned();
        Some((crate::tls::config_tls_tcp(impronta)?, nome))
    } else {
        None
    };
    let tcp: Arc<dyn AsyncUdpSocket> = crate::tcp::SocketTcp::client(destinazione, offusca, tls).await?;
    let socket: Arc<dyn AsyncUdpSocket> = match offusca.filter(|_| !mimetico()) {
        None => tcp,
        Some(segreto) => avvolgi(tcp, segreto, false),
    };
    Ok(Endpoint::new_with_abstract_socket(EndpointConfig::default(), None, socket, runtime)?)
}

/// Collegamento all'ENTRATA della rete: UDP; se in 6 secondi non risponde nessuno (UDP bloccato),
/// lo stesso collegamento su **TCP** alla stessa porta. `NYCTALUS_TCP=1` sempre TCP, `=0` mai.
pub async fn collega_entrata(
    destinazione: SocketAddr,
    offusca: Option<[u8; 32]>,
    config: quinn::ClientConfig,
    impronta: Option<[u8; 32]>,
) -> Risultato<(Endpoint, quinn::Connection)> {
    let modo = std::env::var("NYCTALUS_TCP").unwrap_or_default();
    let nome = crate::tls::nome_per(&destinazione);
    if modo != "1" {
        let locale: SocketAddr = if destinazione.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse()?;
        let endpoint = endpoint_client(locale, offusca)?;
        let tentativo = endpoint.connect_with(config.clone(), destinazione, &nome)?;
        match tokio::time::timeout(std::time::Duration::from_secs(6), tentativo).await {
            Ok(Ok(c)) => return Ok((endpoint, c)),
            Ok(Err(e)) if modo == "0" => return Err(e.into()),
            Err(_) if modo == "0" => return Err("entry did not answer over UDP".into()),
            _ => eprintln!("UDP to {destinazione} did not answer: trying TCP"),
        }
    }
    let endpoint = endpoint_client_tcp(destinazione, offusca, impronta).await?;
    let c = endpoint.connect_with(config, destinazione, &nome)?.await?;
    println!("  entry over TCP (UDP blocked or NYCTALUS_TCP=1)");
    Ok((endpoint, c))
}

/// Crea un endpoint client, con offuscamento se è data la chiave di rete.
pub fn endpoint_client(locale: SocketAddr, offusca: Option<[u8; 32]>) -> Risultato<Endpoint> {
    match offusca.filter(|_| !mimetico()) {
        None => Ok(Endpoint::client(locale)?),
        Some(segreto) => {
            let runtime = default_runtime().ok_or("nessun runtime asincrono disponibile")?;
            let udp = UdpSocket::bind(locale)?;
            udp.set_nonblocking(true)?;
            let socket = avvolgi(runtime.wrap_udp_socket(udp)?, segreto, false);
            Ok(Endpoint::new_with_abstract_socket(EndpointConfig::default(), None, socket, runtime)?)
        }
    }
}

/// Avvolge un socket (UDP, o UDP+TCP) con il camuffamento della rete.
fn avvolgi(inner: Arc<dyn AsyncUdpSocket>, segreto_rete: [u8; 32], server: bool) -> Arc<SocketOffuscato> {
    let chiave = offuscamento::chiave_offuscamento(&segreto_rete);
    let ammessi = server.then(|| Mutex::new(HashMap::new()));
    Arc::new(SocketOffuscato { inner, chiave, ammessi })
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
    fn risposta_h3_ben_formata() {
        let r = risposta_h3();
        assert_eq!(&r[..6], &[0x01, 4, 0x00, 0x00, 0xdb, 0xf4], "HEADERS con :status 404 e content-type");
        assert_eq!(r[6], 0x00, "poi DATA");
        let (l, n) = if r[7] & 0xc0 == 0x40 { (u16::from_be_bytes([r[7] & 0x3f, r[8]]) as usize, 2) } else { (r[7] as usize, 1) };
        assert_eq!(r.len(), 7 + n + l, "lunghezza del corpo coerente");
        assert!(std::str::from_utf8(&r[7 + n..]).unwrap().contains("404 Not Found"));
        assert_ne!(contrassegno(&[1; 32]), contrassegno(&[2; 32]));
    }

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
