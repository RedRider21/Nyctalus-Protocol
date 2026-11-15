// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Inoltratore SOCKS per il browser dell'app (modalità Nyctalus): TUTTO il traffico del browser
//! passa da qui ed entra nella rete attraverso il SOCKS del demone. In più:
//!  - una pagina di VERIFICA che esiste solo qui (`http://verifica.nyctalus.invalid/…`): il browser
//!    la chiede prima di navigare; se non arriva, non apre nulla («chiuso in caso di errore»);
//!  - niente indirizzi locali (127.x, 10.x, 192.168.x, localhost…): i siti non devono poter
//!    raggiungere i servizi del telefono o della rete di casa attraverso il browser;
//!  - solo CONNECT (niente UDP: niente fughe); a rete non pronta, nessuna scorciatoia.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use super::{log, App};

pub const DOMINIO_VERIFICA: &str = "verifica.nyctalus.invalid";

/// Un indirizzo della macchina o della rete locale (mai raggiungibile dal browser).
pub fn indirizzo_locale(host: &str) -> bool {
    let h = host.trim_matches(['[', ']']).to_ascii_lowercase();
    if h == "localhost" || h == "localhost.localdomain" || h.ends_with(".localhost") || h.ends_with(".local") {
        return true;
    }
    match h.parse::<IpAddr>() {
        Ok(IpAddr::V4(a)) => v4_locale(a),
        Ok(IpAddr::V6(a)) => {
            if let Some(v4) = a.to_ipv4_mapped() {
                return v4_locale(v4);
            }
            let s = a.segments();
            a.is_loopback() || a.is_unspecified() || a.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00          // indirizzi locali unici
                || (s[0] & 0xffc0) == 0xfe80          // link-local
                || (s[0] == 0x2001 && s[1] == 0x0db8) // documentazione
        }
        Err(_) => false,
    }
}

fn v4_locale(a: std::net::Ipv4Addr) -> bool {
    let o = a.octets();
    a.is_private() || a.is_loopback() || a.is_link_local() || a.is_unspecified() || a.is_multicast()
        || a.is_documentation() || a.is_broadcast()
        || o[0] == 0 || o[0] >= 240
        || (o[0] == 100 && (64..=127).contains(&o[1]))   // CGNAT
        || (o[0] == 198 && (18..=19).contains(&o[1]))    // prove di prestazione
        || (o[0] == 192 && o[1] == 0 && o[2] == 0)
}

async fn rispondi<W: AsyncWrite + Unpin>(w: &mut W, codice: u8) {
    let _ = w.write_all(&[5, codice, 0, 1, 0, 0, 0, 0, 0, 0]).await;
    let _ = w.flush().await;
}

pub async fn ascolta(app: Arc<App>) {
    let ascolto = match TcpListener::bind(("127.0.0.1", app.porte.socks_browser)).await {
        Ok(a) => a,
        Err(e) => return log(&format!("inoltratore del browser non avviato (porta {}): {e}", app.porte.socks_browser)),
    };
    loop {
        let Ok((c, _)) = ascolto.accept().await else { continue };
        let app = app.clone();
        tokio::spawn(async move {
            let _ = tokio::time::timeout(Duration::from_secs(24 * 3600), gestisci(app, c)).await;
        });
    }
}

async fn leggi<R: AsyncRead + Unpin>(r: &mut R, n: usize, secondi: u64) -> Option<Vec<u8>> {
    let mut b = vec![0u8; n];
    tokio::time::timeout(Duration::from_secs(secondi), r.read_exact(&mut b)).await.ok()?.ok()?;
    Some(b)
}

async fn gestisci(app: Arc<App>, c: TcpStream) -> Option<()> {
    let (lett, mut w) = c.into_split();
    let mut r = BufReader::new(lett);
    let testa = leggi(&mut r, 2, 10).await?;
    if testa[0] != 5 {
        return None;
    }
    let metodi = leggi(&mut r, testa[1] as usize, 10).await?;
    if !metodi.contains(&0) {
        let _ = w.write_all(b"\x05\xff").await;
        return None;
    }
    w.write_all(b"\x05\x00").await.ok()?;
    let rich = leggi(&mut r, 4, 10).await?;
    let (ver, cmd, atyp) = (rich[0], rich[1], rich[3]);
    let host = match atyp {
        1 => {
            let b = leggi(&mut r, 4, 10).await?;
            std::net::Ipv4Addr::new(b[0], b[1], b[2], b[3]).to_string()
        }
        4 => {
            let b: [u8; 16] = leggi(&mut r, 16, 10).await?.try_into().ok()?;
            std::net::Ipv6Addr::from(b).to_string()
        }
        3 => {
            let n = leggi(&mut r, 1, 10).await?[0] as usize;
            String::from_utf8_lossy(&leggi(&mut r, n, 10).await?).into_owned()
        }
        _ => {
            rispondi(&mut w, 8).await;
            return None;
        }
    };
    let porta = u16::from_be_bytes(leggi(&mut r, 2, 10).await?.try_into().ok()?);
    if ver != 5 || cmd != 1 {
        rispondi(&mut w, 7).await; // solo CONNECT
        return None;
    }
    if host.eq_ignore_ascii_case(DOMINIO_VERIFICA) {
        rispondi(&mut w, 0).await;
        let mut riga = Vec::new();
        tokio::time::timeout(Duration::from_secs(10), r.read_until(b'\n', &mut riga)).await.ok()?.ok()?;
        let riga = String::from_utf8_lossy(&riga);
        let percorso = riga.split(' ').nth(1).unwrap_or("/");
        let pulito: String = percorso.chars().filter(|c| c.is_ascii_alphanumeric() || "/_-".contains(*c)).take(80).collect();
        let corpo = format!("<!doctype html><title>nyctalus-ok</title><body>nyctalus-ok {pulito}</body>");
        let t = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}", corpo.len());
        let _ = w.write_all(t.as_bytes()).await;
        return None;
    }
    if indirizzo_locale(&host) {
        rispondi(&mut w, 2).await; // vietato dalle regole
        return None;
    }
    let (stato, modalita) = {
        let s = app.servizi.lock().unwrap();
        (s.stato.clone(), s.modalita.clone())
    };
    if stato != "pronto" {
        rispondi(&mut w, 1).await; // rete non pronta: nessuna scorciatoia
        return None;
    }
    let sopra = if modalita == "pubblica" { app.porte.socks } else { app.porte.socks_locale };
    let s2 = tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(("127.0.0.1", sopra))).await.ok()?.ok()?;
    let (l2, mut w2) = s2.into_split();
    let mut r2 = BufReader::new(l2);
    w2.write_all(b"\x05\x01\x00").await.ok()?;
    if leggi(&mut r2, 2, 30).await? != b"\x05\x00" {
        rispondi(&mut w, 1).await;
        return None;
    }
    let h: Vec<u8> = host.bytes().filter(u8::is_ascii).collect(); // (Chromium manda già i nomi in punycode)
    let mut req = vec![5, 1, 0, 3, h.len().min(255) as u8];
    req.extend_from_slice(&h[..h.len().min(255)]);
    req.extend_from_slice(&porta.to_be_bytes());
    w2.write_all(&req).await.ok()?;
    let risposta = leggi(&mut r2, 4, 60).await?;
    match risposta[3] {
        1 => drop(leggi(&mut r2, 6, 60).await?),
        4 => drop(leggi(&mut r2, 18, 60).await?),
        _ => {
            let n = leggi(&mut r2, 1, 60).await?[0] as usize;
            leggi(&mut r2, n + 2, 60).await?;
        }
    }
    if risposta[1] != 0 {
        rispondi(&mut w, risposta[1]).await;
        return None;
    }
    rispondi(&mut w, 0).await;
    let andata = async {
        let _ = tokio::io::copy_buf(&mut r, &mut w2).await;
        let _ = w2.shutdown().await;
    };
    let ritorno = async {
        let _ = tokio::io::copy_buf(&mut r2, &mut w).await;
        let _ = w.shutdown().await;
    };
    tokio::join!(andata, ritorno);
    Some(())
}

#[cfg(test)]
mod prove {
    use super::indirizzo_locale;

    #[test]
    fn indirizzi_locali() {
        for h in ["localhost", "x.localhost", "stampante.local", "127.0.0.1", "10.1.2.3", "192.168.1.1", "172.16.0.1",
                  "169.254.1.1", "0.0.0.0", "[::1]", "::", "fe80::1", "fd00::1", "::ffff:192.168.1.1", "100.64.0.1", "224.0.0.1"] {
            assert!(indirizzo_locale(h), "{h} dovrebbe essere locale");
        }
        for h in ["example.com", "8.8.8.8", "1.1.1.1", "2606:4700::1111", "abc.nyct"] {
            assert!(!indirizzo_locale(h), "{h} non è locale");
        }
    }
}
