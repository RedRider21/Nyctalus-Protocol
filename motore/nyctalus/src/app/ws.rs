// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Il ponte fra la web-app e il motore della chat: WebSocket `/ws` ↔ porta di controllo (le righe
//! passano così come sono; il segreto del motore resta qui), il flusso delle notifiche per il
//! servizio Android (`/api/notifiche`, una riga JSON per ogni messaggio, file o chiamata in arrivo)
//! e i comandi secchi (rifiuta una chiamata dalla notifica). Come il Python.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use super::http::Richiesta;
use super::{leggi_config, App};
use crate::Risultato;

pub const MAX_FRAME_WS: usize = 1024 * 1024;
/// Righe del motore: le immagini delle videochiamate superano i 64 KB.
pub const RIGA_MOTORE_MAX: usize = 1024 * 1024;
const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

pub fn base64(b: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for g in b.chunks(3) {
        let n = g.iter().fold(0u32, |a, &x| (a << 8) | x as u32) << (8 * (3 - g.len()));
        for i in 0..4 {
            if i <= g.len() {
                out.push(A[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn da_hex(h: &str) -> String {
    if h.len() % 2 != 0 {
        return String::new();
    }
    let b: Option<Vec<u8>> = (0..h.len() / 2).map(|i| u8::from_str_radix(h.get(2 * i..2 * i + 2)?, 16).ok()).collect();
    b.map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
}

fn taglia(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

pub async fn ws_invia<W: AsyncWrite + Unpin>(w: &mut W, testo: &str) -> Risultato<()> {
    let d = testo.as_bytes();
    let mut testa = vec![0x81u8];
    if d.len() < 126 {
        testa.push(d.len() as u8);
    } else if d.len() < 65536 {
        testa.push(126);
        testa.extend_from_slice(&(d.len() as u16).to_be_bytes());
    } else {
        testa.push(127);
        testa.extend_from_slice(&(d.len() as u64).to_be_bytes());
    }
    w.write_all(&testa).await?;
    w.write_all(d).await?;
    w.flush().await?;
    Ok(())
}

/// Il testo del prossimo messaggio, o None a connessione chiusa (o frame non ammesso).
pub async fn ws_ricevi<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(r: &mut R, w: &Mutex<&mut W>) -> Option<String> {
    let mut pezzi = Vec::new();
    loop {
        let mut t = [0u8; 2];
        r.read_exact(&mut t).await.ok()?;
        let (fin, op, mascherato, mut n) = (t[0] & 0x80 != 0, t[0] & 0x0f, t[1] & 0x80 != 0, (t[1] & 0x7f) as usize);
        if n == 126 {
            let mut x = [0u8; 2];
            r.read_exact(&mut x).await.ok()?;
            n = u16::from_be_bytes(x) as usize;
        } else if n == 127 {
            let mut x = [0u8; 8];
            r.read_exact(&mut x).await.ok()?;
            n = usize::try_from(u64::from_be_bytes(x)).unwrap_or(usize::MAX);
        }
        if n > MAX_FRAME_WS || !mascherato {
            return None; // il client DEVE mascherare
        }
        let mut maschera = [0u8; 4];
        r.read_exact(&mut maschera).await.ok()?;
        let mut d = vec![0u8; n];
        r.read_exact(&mut d).await.ok()?;
        for (i, b) in d.iter_mut().enumerate() {
            *b ^= maschera[i % 4];
        }
        match op {
            0x8 => return None,
            0x9 => {
                let mut g = w.lock().await;
                let corpo = &d[..d.len().min(125)];
                let mut f = vec![0x8a, corpo.len() as u8];
                f.extend_from_slice(corpo);
                g.write_all(&f).await.ok()?;
                g.flush().await.ok()?;
                continue;
            }
            0xa => continue,
            _ => {}
        }
        pezzi.extend_from_slice(&d);
        if pezzi.len() > MAX_FRAME_WS {
            return None;
        }
        if fin {
            return Some(String::from_utf8_lossy(&pezzi).into_owned());
        }
    }
}

async fn collega_motore(app: &App) -> std::io::Result<TcpStream> {
    tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(("127.0.0.1", app.porte.controllo)))
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "motore"))?
}

async fn leggi_riga<R: AsyncRead + Unpin>(r: &mut BufReader<R>) -> Option<String> {
    let mut b = Vec::new();
    let n = r.read_until(b'\n', &mut b).await.ok()?;
    if n == 0 || b.len() > RIGA_MOTORE_MAX {
        return None;
    }
    if b.ends_with(b"\n") {
        b.pop();
    }
    Some(String::from_utf8_lossy(&b).into_owned())
}

/// WebSocket ↔ porta di controllo del motore.
pub async fn ponte_chat<R, W>(app: &App, req: &Richiesta, lettore: &mut BufReader<R>, w: &mut W) -> Risultato<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let chiave = req.intestazione("sec-websocket-key");
    let h = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, format!("{chiave}{WS_GUID}").as_bytes());
    let risposta = format!("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n", base64(h.as_ref()));
    w.write_all(risposta.as_bytes()).await?;
    w.flush().await?;
    let motore = match collega_motore(app).await {
        Ok(m) => m,
        Err(_) => return ws_invia(w, "ERR motore non raggiungibile").await,
    };
    let (m_lett, mut m_scr) = motore.into_split();
    let mut m_lett = BufReader::with_capacity(64 * 1024, m_lett);
    m_scr.write_all(format!("AUTH {}\n", app.segreto_motore).as_bytes()).await?;
    let w = Mutex::new(w);
    let dal_motore = async {
        while let Some(riga) = leggi_riga(&mut m_lett).await {
            if riga != "AUTHOK" {
                let mut g = w.lock().await;
                if ws_invia(&mut **g, &riga).await.is_err() {
                    break;
                }
            }
        }
    };
    let dalla_webapp = async {
        while let Some(testo) = ws_ricevi(lettore, &w).await {
            for riga in testo.lines() {
                if riga.starts_with("AUTH") {
                    continue; // il segreto del motore resta qui
                }
                if m_scr.write_all(format!("{riga}\n").as_bytes()).await.is_err() {
                    return;
                }
            }
        }
    };
    tokio::select! {
        _ = dal_motore => {},
        _ = dalla_webapp => {},
    }
    Ok(())
}

/// Un comando secco al motore (es. «CALLNO addr id» dal pulsante Rifiuta della notifica).
pub async fn comando_motore(app: &App, riga: &str) -> bool {
    let Ok(m) = collega_motore(app).await else { return false };
    let (lett, mut scr) = m.into_split();
    if scr.write_all(format!("AUTH {}\n{riga}\n", app.segreto_motore).as_bytes()).await.is_err() {
        return false;
    }
    let mut lett = BufReader::new(lett);
    tokio::time::timeout(Duration::from_secs(5), leggi_riga(&mut lett)).await.ok().flatten().is_some()
}

/// Per il servizio di notifica dell'app Android: una riga JSON per ogni messaggio, file o chiamata
/// in ARRIVO. Resta aperto; ogni 25 s «{}» per accorgersi se la connessione è caduta.
pub async fn flusso_notifiche<W: AsyncWrite + Unpin>(app: &App, w: &mut W) -> Risultato<()> {
    w.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n").await?;
    w.flush().await?;
    async fn riga<W: AsyncWrite + Unpin>(w: &mut W, v: Value) -> Risultato<()> {
        // come json.dumps(ensure_ascii=False): UTF-8 così com'è
        let mut t = super::http::json_formattato(&v, false);
        t.push('\n');
        w.write_all(t.as_bytes()).await?;
        w.flush().await?;
        Ok(())
    }
    let Ok(m) = collega_motore(app).await else { return riga(w, json!({"tipo": "motore", "ok": false})).await };
    let (lett, mut scr) = m.into_split();
    let mut lett = BufReader::with_capacity(64 * 1024, lett);
    scr.write_all(format!("AUTH {}\nLIST\nSUB\n", app.segreto_motore).as_bytes()).await?;
    riga(w, json!({"tipo": "motore", "ok": true})).await?;
    let mut nomi: HashMap<String, String> = HashMap::new();
    loop {
        let dati = match tokio::time::timeout(Duration::from_secs(25), leggi_riga(&mut lett)).await {
            Err(_) => {
                riga(w, json!({})).await?; // «sono qui»: se l'app non c'è più, si chiude
                continue;
            }
            Ok(None) => return riga(w, json!({"tipo": "motore", "ok": false})).await,
            Ok(Some(d)) => d,
        };
        let p: Vec<&str> = dati.split(' ').collect();
        let rubrica = leggi_config(&app.p).get("rubrica").and_then(Value::as_str).unwrap_or("").to_string();
        let nome = |nomi: &HashMap<String, String>, a: &str| nomi.get(a).cloned().unwrap_or_else(|| taglia(a, 10));
        if p[0] == "C" && p.len() >= 3 {
            let n = da_hex(p[2]);
            nomi.insert(p[1].to_string(), if n.is_empty() { taglia(p[1], 10) } else { n });
        } else if p.len() >= 2 && p[1] == rubrica {
            continue;
        } else if p[0] == "MSG" && p.len() >= 4 {
            riga(w, json!({"tipo": "msg", "da": p[1], "nome": nome(&nomi, p[1]), "testo": taglia(&da_hex(p[3]), 200)})).await?;
        } else if p[0] == "CALLIN" && p.len() >= 3 {
            riga(w, json!({"tipo": "chiamata", "da": p[1], "nome": nome(&nomi, p[1]), "id": p[2], "video": p.len() >= 4 && p[3] == "v"})).await?;
        } else if ["CALLEND", "CALLOK", "CALLNO"].contains(&p[0]) && p.len() >= 3 {
            riga(w, json!({"tipo": "fine_chiamata", "da": p[1], "id": p[2]})).await?;
        } else if p[0] == "XNEW" && p.len() >= 9 && p[2] == "in" && p[7] == "chiede" {
            let testo = format!("📦 vuole mandarti «{}»: apri File per accettare", taglia(&da_hex(p[6]), 70));
            riga(w, json!({"tipo": "msg", "da": p[3], "nome": nome(&nomi, p[3]), "testo": testo})).await?;
        } else if p[0] == "FILE" && p.len() >= 9 && p[2] == "in" {
            riga(w, json!({"tipo": "file", "da": p[1], "nome": nome(&nomi, p[1]), "mime": p[6], "file": taglia(&da_hex(p[7]), 80)})).await?;
        }
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn base64_e_hex() {
        assert_eq!(base64(b"ciao"), "Y2lhbw==");
        // l'esempio della RFC 6455
        let h = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, format!("dGhlIHNhbXBsZSBub25jZQ=={WS_GUID}").as_bytes());
        assert_eq!(base64(h.as_ref()), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
        assert_eq!(da_hex("6369616f"), "ciao");
        assert_eq!(da_hex("zz"), "");
    }
}
