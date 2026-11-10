// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! **Forma «videochiamata»** (PIANO-ANONIMATO, invisibilità punto 2): il traffico ha la forma di
//! una normale videochiamata WebRTC, il traffico UDP più comune dopo il web:
//!
//! - **STUN** (RFC 5389/8445): all'inizio e poi ogni ~5 secondi il client manda una «Binding
//!   Request» di ICE (USERNAME, PRIORITY, ICE-CONTROLLING, MESSAGE-INTEGRITY HMAC-SHA1,
//!   FINGERPRINT), come fa ogni browser per controllare la linea (consent freshness, RFC 7675);
//!   il nodo risponde «Binding Success» con XOR-MAPPED-ADDRESS **solo** se l'integrità è giusta;
//! - **SRTP** (RFC 3550/3711): ogni pacchetto di Nyctalus viaggia come un pacchetto video:
//!   intestazione RTP in chiaro (versione 2, tipo 96 = VP8, sequenza, orologio a 90 kHz, SSRC del
//!   flusso), contenuto cifrato (ChaCha20, nonce da SSRC/orologio/sequenza: nessun byte in più) e
//!   in fondo 10 byte di **etichetta di autenticazione** (come HMAC-SHA1-80 di SRTP) che qui è un
//!   vero codice BLAKE3 con la chiave della rete: un pacchetto senza la chiave viene scartato
//!   subito, quindi chi sonda il nodo non riceve niente.
//!
//! È una scelta della rete (`--webrtc`, codice di rete `"forma": "webrtc"`); dentro c'è il solito
//! QUIC con la cipolla. Limite onesto: ritmo e dimensioni dei pacchetti sono quelli di Nyctalus,
//! non di un video a 30 fotogrammi (il rumore di fondo regolare aiuta).

use std::collections::HashMap;
use std::io::{self, IoSliceMut};
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use chacha20::cipher::{KeyIvInit, StreamCipher};
use quinn::udp::{RecvMeta, Transmit};
use quinn::{AsyncUdpSocket, UdpPoller};

static ATTIVA: AtomicBool = AtomicBool::new(false);

/// Accende la forma «videochiamata» per tutto il processo.
pub fn imposta() {
    ATTIVA.store(true, Ordering::Relaxed);
}

pub fn attiva() -> bool {
    ATTIVA.load(Ordering::Relaxed)
}

const MAGIA_STUN: u32 = 0x2112_A442;
const PT_VIDEO: u8 = 96;
const RTP: usize = 12;
const ETICHETTA: usize = 10;
/// Ogni quanto il client ricontrolla la linea con STUN (come i browser: consent freshness).
const OGNI_STUN: Duration = Duration::from_millis(4500);

/// Chiavi derivate dalla chiave della rete.
#[derive(Clone)]
pub struct Chiavi {
    cifra: [u8; 32],
    mac: [u8; 32],
    /// password ICE (stringa) per MESSAGE-INTEGRITY
    password: Vec<u8>,
}

impl Chiavi {
    pub fn da(chiave_rete: &[u8; 32]) -> Self {
        let cifra = blake3::derive_key("Nyctalus 2026-11-10 videochiamata: cifratura", chiave_rete);
        let mac = blake3::derive_key("Nyctalus 2026-11-10 videochiamata: etichetta", chiave_rete);
        let p = blake3::derive_key("Nyctalus 2026-11-10 videochiamata: password ICE", chiave_rete);
        // 24 caratteri «base64» come le password ICE dei browser
        const ALFABETO: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let password = p.iter().take(24).map(|b| ALFABETO[(*b as usize) % 64]).collect();
        Self { cifra, mac, password }
    }
}

// --- RTP/SRTP -------------------------------------------------------------------------------

fn nonce(ssrc: u32, ts: u32, seq: u16) -> [u8; 12] {
    let mut n = [0u8; 12];
    n[..4].copy_from_slice(&ssrc.to_be_bytes());
    n[4..8].copy_from_slice(&ts.to_be_bytes());
    n[8..10].copy_from_slice(&seq.to_be_bytes());
    n
}

fn etichetta(k: &Chiavi, dati: &[u8]) -> [u8; ETICHETTA] {
    let h = blake3::keyed_hash(&k.mac, dati);
    let mut e = [0u8; ETICHETTA];
    e.copy_from_slice(&h.as_bytes()[..ETICHETTA]);
    e
}

/// Un datagramma di Nyctalus vestito da pacchetto SRTP video.
pub fn incarta(k: &Chiavi, ssrc: u32, seq: u16, ts: u32, marcatore: bool, carico: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(RTP + carico.len() + ETICHETTA);
    v.push(0x80); // V=2, nessun riempimento, nessuna estensione, CC=0
    v.push(PT_VIDEO | if marcatore { 0x80 } else { 0 });
    v.extend_from_slice(&seq.to_be_bytes());
    v.extend_from_slice(&ts.to_be_bytes());
    v.extend_from_slice(&ssrc.to_be_bytes());
    let inizio = v.len();
    v.extend_from_slice(carico);
    let mut c = chacha20::ChaCha20::new(&k.cifra.into(), &nonce(ssrc, ts, seq).into());
    c.apply_keystream(&mut v[inizio..]);
    let e = etichetta(k, &v);
    v.extend_from_slice(&e);
    v
}

/// Il contenuto di un pacchetto SRTP nostro, oppure `None` (non è dei nostri: si scarta).
pub fn scarta_o_apri(k: &Chiavi, d: &[u8]) -> Option<Vec<u8>> {
    if d.len() < RTP + 1 + ETICHETTA || d[0] >> 6 != 2 {
        return None;
    }
    let (corpo, e) = d.split_at(d.len() - ETICHETTA);
    if etichetta(k, corpo) != e {
        return None;
    }
    let seq = u16::from_be_bytes([corpo[2], corpo[3]]);
    let ts = u32::from_be_bytes(corpo[4..8].try_into().ok()?);
    let ssrc = u32::from_be_bytes(corpo[8..12].try_into().ok()?);
    let mut carico = corpo[RTP..].to_vec();
    let mut c = chacha20::ChaCha20::new(&k.cifra.into(), &nonce(ssrc, ts, seq).into());
    c.apply_keystream(&mut carico);
    Some(carico)
}

// --- STUN (ICE) -----------------------------------------------------------------------------

fn crc32(d: &[u8]) -> u32 {
    let mut c: u32 = 0xFFFF_FFFF;
    for b in d {
        c ^= *b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { (c >> 1) ^ 0xEDB8_8320 } else { c >> 1 };
        }
    }
    !c
}

fn hmac_sha1(chiave: &[u8], d: &[u8]) -> [u8; 20] {
    let k = ring::hmac::Key::new(ring::hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, chiave);
    let t = ring::hmac::sign(&k, d);
    let mut o = [0u8; 20];
    o.copy_from_slice(t.as_ref());
    o
}

fn attributo(v: &mut Vec<u8>, tipo: u16, valore: &[u8]) {
    v.extend_from_slice(&tipo.to_be_bytes());
    v.extend_from_slice(&(valore.len() as u16).to_be_bytes());
    v.extend_from_slice(valore);
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

/// Chiude un messaggio STUN: MESSAGE-INTEGRITY (HMAC-SHA1 con la password ICE) e FINGERPRINT.
fn chiudi_stun(mut v: Vec<u8>, password: &[u8]) -> Vec<u8> {
    let lung_con_mi = (v.len() - 20 + 24) as u16;
    v[2..4].copy_from_slice(&lung_con_mi.to_be_bytes());
    let mi = hmac_sha1(password, &v);
    attributo(&mut v, 0x0008, &mi);
    let lung_con_fp = (v.len() - 20 + 8) as u16;
    v[2..4].copy_from_slice(&lung_con_fp.to_be_bytes());
    let fp = crc32(&v) ^ 0x5354_554e;
    attributo(&mut v, 0x8028, &fp.to_be_bytes());
    v
}

fn testa_stun(tipo: u16, transazione: &[u8; 12]) -> Vec<u8> {
    let mut v = Vec::with_capacity(120);
    v.extend_from_slice(&tipo.to_be_bytes());
    v.extend_from_slice(&[0, 0]);
    v.extend_from_slice(&MAGIA_STUN.to_be_bytes());
    v.extend_from_slice(transazione);
    v
}

/// Binding Request di ICE, come quella di un browser.
pub fn richiesta_stun(k: &Chiavi, utente: &str) -> Vec<u8> {
    let mut t = [0u8; 12];
    getrandom::fill(&mut t).expect("generatore casuale di sistema");
    let mut v = testa_stun(0x0001, &t);
    attributo(&mut v, 0x0006, utente.as_bytes()); // USERNAME «remoto:locale»
    let mut spareggio = [0u8; 8];
    getrandom::fill(&mut spareggio).expect("generatore casuale di sistema");
    attributo(&mut v, 0x802A, &spareggio); // ICE-CONTROLLING
    attributo(&mut v, 0x0024, &0x6E7F_1EFFu32.to_be_bytes()); // PRIORITY (candidato host UDP)
    chiudi_stun(v, &k.password)
}

/// È un messaggio STUN (primi due bit 00 e magia al posto giusto)?
pub fn e_stun(d: &[u8]) -> bool {
    d.len() >= 20 && d[0] >> 6 == 0 && u32::from_be_bytes([d[4], d[5], d[6], d[7]]) == MAGIA_STUN
}

/// Lato nodo: se `d` è una Binding Request con l'integrità giusta, la risposta da mandare.
pub fn rispondi_stun(k: &Chiavi, d: &[u8], da: SocketAddr) -> Option<Vec<u8>> {
    if !e_stun(d) || u16::from_be_bytes([d[0], d[1]]) != 0x0001 {
        return None;
    }
    // si cerca MESSAGE-INTEGRITY e si ricalcola sull'inizio del messaggio
    let mut i = 20;
    let mut mi_ok = false;
    while i + 4 <= d.len() {
        let t = u16::from_be_bytes([d[i], d[i + 1]]);
        let l = u16::from_be_bytes([d[i + 2], d[i + 3]]) as usize;
        if i + 4 + l > d.len() {
            return None;
        }
        if t == 0x0008 && l == 20 {
            let mut testa = d[..i].to_vec();
            testa[2..4].copy_from_slice(&((i - 20 + 24) as u16).to_be_bytes());
            mi_ok = hmac_sha1(&k.password, &testa) == d[i + 4..i + 24];
            break;
        }
        i += 4 + (l + 3) / 4 * 4;
    }
    if !mi_ok {
        return None; // non è dei nostri: nessuna risposta (come un agente ICE con credenziali diverse)
    }
    let mut t = [0u8; 12];
    t.copy_from_slice(&d[8..20]);
    let mut v = testa_stun(0x0101, &t);
    // XOR-MAPPED-ADDRESS
    let porta = da.port() ^ (MAGIA_STUN >> 16) as u16;
    let mut x = vec![0u8];
    match da.ip() {
        IpAddr::V4(a) => {
            x.push(0x01);
            x.extend_from_slice(&porta.to_be_bytes());
            let o = a.octets();
            let m = MAGIA_STUN.to_be_bytes();
            x.extend((0..4).map(|j| o[j] ^ m[j]));
        }
        IpAddr::V6(a) => {
            x.push(0x02);
            x.extend_from_slice(&porta.to_be_bytes());
            let o = a.octets();
            let mut m = MAGIA_STUN.to_be_bytes().to_vec();
            m.extend_from_slice(&t);
            x.extend((0..16).map(|j| o[j] ^ m[j]));
        }
    }
    attributo(&mut v, 0x0020, &x);
    Some(chiudi_stun(v, &k.password))
}

// --- il socket per quinn --------------------------------------------------------------------

struct Flusso {
    ssrc: u32,
    seq: u16,
    ts0: u32,
    inizio: Instant,
    ultimo_stun: Option<Instant>,
    utente: String,
}

fn casuale_u32() -> u32 {
    let mut b = [0u8; 4];
    getrandom::fill(&mut b).expect("generatore casuale di sistema");
    u32::from_le_bytes(b)
}

fn frammento_ice() -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut b = [0u8; 4];
    getrandom::fill(&mut b).expect("generatore casuale di sistema");
    b.iter().map(|x| A[(*x as usize) % 64] as char).collect()
}

/// Socket che veste i datagrammi da videochiamata WebRTC.
pub struct SocketWebRtc {
    inner: Arc<dyn AsyncUdpSocket>,
    chiavi: Chiavi,
    server: bool,
    flussi: Mutex<HashMap<SocketAddr, Flusso>>,
}

impl std::fmt::Debug for SocketWebRtc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SocketWebRtc")
    }
}

impl SocketWebRtc {
    pub fn nuovo(inner: Arc<dyn AsyncUdpSocket>, chiave_rete: &[u8; 32], server: bool) -> Arc<Self> {
        Arc::new(Self { inner, chiavi: Chiavi::da(chiave_rete), server, flussi: Mutex::new(HashMap::new()) })
    }

    fn manda_grezzo(&self, a: SocketAddr, dati: &[u8], modello: &Transmit) -> io::Result<()> {
        self.inner.try_send(&Transmit { destination: a, ecn: None, contents: dati, segment_size: None, src_ip: modello.src_ip })
    }
}

impl AsyncUdpSocket for SocketWebRtc {
    fn create_io_poller(self: Arc<Self>) -> Pin<Box<dyn UdpPoller>> {
        self.inner.clone().create_io_poller()
    }

    fn try_send(&self, transmit: &Transmit) -> io::Result<()> {
        let (pacchetto, stun) = {
            let mut fl = self.flussi.lock().unwrap_or_else(|e| e.into_inner());
            let f = fl.entry(transmit.destination).or_insert_with(|| Flusso {
                ssrc: casuale_u32(),
                seq: casuale_u32() as u16,
                ts0: casuale_u32(),
                inizio: Instant::now(),
                ultimo_stun: None,
                utente: format!("{}:{}", frammento_ice(), frammento_ice()),
            });
            // il client ricontrolla la linea con STUN all'inizio e poi ogni ~5 s
            let stun = if !self.server && f.ultimo_stun.is_none_or(|t| t.elapsed() >= OGNI_STUN) {
                f.ultimo_stun = Some(Instant::now());
                Some(richiesta_stun(&self.chiavi, &f.utente))
            } else {
                None
            };
            f.seq = f.seq.wrapping_add(1);
            let ts = f.ts0.wrapping_add((f.inizio.elapsed().as_micros() as u64 * 9 / 100) as u32); // 90 kHz
            let marcatore = f.seq % 4 == 0;
            (incarta(&self.chiavi, f.ssrc, f.seq, ts, marcatore, transmit.contents), stun)
        };
        if let Some(s) = stun {
            let _ = self.manda_grezzo(transmit.destination, &s, transmit);
        }
        self.manda_grezzo(transmit.destination, &pacchetto, transmit)
    }

    fn poll_recv(&self, cx: &mut Context, bufs: &mut [IoSliceMut<'_>], meta: &mut [RecvMeta]) -> Poll<io::Result<usize>> {
        let mut appoggio: Vec<Vec<u8>> = bufs.iter().map(|b| vec![0u8; b.len()]).collect();
        let quanti = {
            let mut fette: Vec<IoSliceMut> = appoggio.iter_mut().map(|v| IoSliceMut::new(v)).collect();
            match self.inner.poll_recv(cx, &mut fette, meta) {
                Poll::Ready(Ok(n)) => n,
                altro => return altro,
            }
        };
        for i in 0..quanti {
            let d = &appoggio[i][..meta[i].len];
            let mut tieni = 0;
            if e_stun(d) {
                if self.server {
                    if let Some(r) = rispondi_stun(&self.chiavi, d, meta[i].addr) {
                        let _ = self.inner.try_send(&Transmit {
                            destination: meta[i].addr,
                            ecn: None,
                            contents: &r,
                            segment_size: None,
                            src_ip: meta[i].dst_ip,
                        });
                    }
                }
            } else if let Some(c) = scarta_o_apri(&self.chiavi, d) {
                let m = c.len().min(bufs[i].len());
                bufs[i][..m].copy_from_slice(&c[..m]);
                tieni = m;
            }
            meta[i].len = tieni;
            meta[i].stride = tieni;
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
    fn srtp_andata_e_ritorno_e_estranei_scartati() {
        let k = Chiavi::da(&[7; 32]);
        let p = incarta(&k, 0x1234_5678, 42, 900_000, true, b"un pacchetto QUIC qualunque");
        assert_eq!(p[0] >> 6, 2, "RTP versione 2");
        assert_eq!(p[1] & 0x7f, PT_VIDEO);
        assert_eq!(scarta_o_apri(&k, &p).unwrap(), b"un pacchetto QUIC qualunque");
        let altra = Chiavi::da(&[8; 32]);
        assert!(scarta_o_apri(&altra, &p).is_none(), "chiave sbagliata: scartato");
        let mut alterato = p.clone();
        alterato[15] ^= 1;
        assert!(scarta_o_apri(&k, &alterato).is_none(), "alterato: scartato");
    }

    #[test]
    fn stun_con_integrita_giusta_risponde_altrimenti_no() {
        let k = Chiavi::da(&[7; 32]);
        let r = richiesta_stun(&k, "abcd:efgh");
        assert!(e_stun(&r));
        // FINGERPRINT corretto (come lo controllerebbe Wireshark)
        let n = r.len();
        let fp = u32::from_be_bytes(r[n - 4..].try_into().unwrap());
        assert_eq!(fp, crc32(&r[..n - 8]) ^ 0x5354_554e);
        let da: SocketAddr = "203.0.113.7:40000".parse().unwrap();
        let risposta = rispondi_stun(&k, &r, da).expect("integrità giusta: risponde");
        assert_eq!(u16::from_be_bytes([risposta[0], risposta[1]]), 0x0101);
        assert_eq!(&risposta[8..20], &r[8..20], "stessa transazione");
        assert!(rispondi_stun(&Chiavi::da(&[9; 32]), &r, da).is_none(), "integrità sbagliata: nessuna risposta");
    }

    #[test]
    fn hmac_sha1_vettore_rfc2202() {
        // RFC 2202, caso 2
        let h = hmac_sha1(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(h.iter().map(|b| format!("{b:02x}")).collect::<String>(), "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79");
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }
}
