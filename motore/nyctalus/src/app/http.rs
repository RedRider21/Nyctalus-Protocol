// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! HTTP/1.1 minimo per l'app (solo 127.0.0.1): richiesta, risposte con le intestazioni di
//! sicurezza, file con `Range` (per scorrere audio e video). Stesso comportamento
//! dell'orchestratore Python (`mobile/orchestratore/nyctalus_mobile.py`).

use std::collections::HashMap;
use std::path::Path;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt, BufReader};

use crate::Risultato;

/// Intestazione della richiesta al massimo (come il Python).
pub const MAX_INTESTAZIONE: usize = 16 * 1024;

pub const INTESTAZIONI_SICUREZZA: &str = "X-Content-Type-Options: nosniff\r\n\
Referrer-Policy: no-referrer\r\n\
Cache-Control: no-store\r\n\
Content-Security-Policy: default-src 'self'; img-src 'self' blob: data:; media-src 'self' blob:; \
connect-src 'self' ws://127.0.0.1:* ws://localhost:*; style-src 'self' 'unsafe-inline'; frame-ancestors 'none'\r\n";

/// Una richiesta: metodo, percorso (già decodificato), parametri e intestazioni (nomi minuscoli).
#[derive(Debug, Default)]
pub struct Richiesta {
    pub metodo: String,
    pub percorso: String,
    pub query: HashMap<String, String>,
    pub h: HashMap<String, String>,
}

impl Richiesta {
    pub fn cookie(&self, nome: &str) -> Option<String> {
        self.h.get("cookie")?.split(';').find_map(|parte| {
            let (k, v) = parte.trim().split_once('=')?;
            (k == nome).then(|| v.to_string())
        })
    }

    pub fn intestazione(&self, nome: &str) -> &str {
        self.h.get(nome).map(String::as_str).unwrap_or("")
    }

    pub fn param(&self, nome: &str) -> Option<&str> {
        self.query.get(nome).map(String::as_str)
    }
}

/// Decodifica `%xx` (e, nei parametri, `+` → spazio). I byte non validi restano come sono.
pub fn decodifica(s: &str, piu_spazio: bool) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 3;
                        continue;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            b'+' if piu_spazio => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Codifica per un parametro di URL (come `urllib.parse.urlencode`).
pub fn codifica(s: &str) -> String {
    let mut out = String::new();
    for &c in s.as_bytes() {
        if c.is_ascii_alphanumeric() || b"-_.~".contains(&c) {
            out.push(c as char);
        } else if c == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    out
}

pub fn leggi_query(q: &str) -> HashMap<String, String> {
    q.split('&')
        .filter(|p| !p.is_empty())
        .filter_map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (!v.is_empty()).then(|| (decodifica(k, true), decodifica(v, true)))
        })
        .collect()
}

/// Legge la riga di richiesta e le intestazioni (fino alla riga vuota, al massimo 16 KB).
pub async fn leggi_richiesta<R: tokio::io::AsyncRead + Unpin>(lettore: &mut BufReader<R>) -> Risultato<Richiesta> {
    let mut testa = Vec::new();
    loop {
        let n = lettore.read_until(b'\n', &mut testa).await?;
        if n == 0 {
            return Err("connessione chiusa".into());
        }
        if testa.len() > MAX_INTESTAZIONE {
            return Err("intestazione troppo grande".into());
        }
        if testa.ends_with(b"\r\n\r\n") || testa == b"\r\n" {
            break;
        }
    }
    let testo: String = testa.iter().map(|&b| b as char).collect(); // latin-1, come il Python
    let mut righe = testo.split("\r\n");
    let prima = righe.next().unwrap_or("");
    let mut parti = prima.splitn(3, ' ');
    let (metodo, uri) = (parti.next().unwrap_or(""), parti.next().unwrap_or(""));
    if metodo.is_empty() || uri.is_empty() {
        return Err("richiesta non valida".into());
    }
    let mut h = HashMap::new();
    for r in righe {
        if let Some((k, v)) = r.split_once(':') {
            h.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
    }
    let (percorso, query) = uri.split_once('?').unwrap_or((uri, ""));
    let percorso = percorso.split('#').next().unwrap_or("");
    Ok(Richiesta { metodo: metodo.to_string(), percorso: decodifica(percorso, false), query: leggi_query(query), h })
}

pub async fn rispondi<W: AsyncWrite + Unpin>(w: &mut W, stato: &str, corpo: &[u8], tipo: &str, extra: &str) -> Risultato<()> {
    let testa = format!(
        "HTTP/1.1 {stato}\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\n{INTESTAZIONI_SICUREZZA}{extra}Connection: close\r\n\r\n",
        corpo.len()
    );
    w.write_all(testa.as_bytes()).await?;
    w.write_all(corpo).await?;
    w.flush().await?;
    Ok(())
}

pub async fn testo<W: AsyncWrite + Unpin>(w: &mut W, stato: &str, t: &str) -> Risultato<()> {
    rispondi(w, stato, t.as_bytes(), "text/plain; charset=utf-8", "").await
}

pub async fn json<W: AsyncWrite + Unpin>(w: &mut W, stato: &str, v: &serde_json::Value) -> Risultato<()> {
    // come json.dumps del Python: separatori «, » e «: », caratteri non ASCII come \uXXXX
    rispondi(w, stato, json_come_python(v).as_bytes(), "application/json", "").await
}

/// JSON con lo stesso aspetto di `json.dumps` (spazi dopo «,» e «:», non-ASCII in `\uXXXX`).
pub fn json_come_python(v: &serde_json::Value) -> String {
    use serde_json::Value;
    fn stringa(s: &str, out: &mut String) {
        out.push('"');
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                    let mut buf = [0u16; 2];
                    for u in c.encode_utf16(&mut buf) {
                        out.push_str(&format!("\\u{u:04x}"));
                    }
                }
                c => out.push(c),
            }
        }
        out.push('"');
    }
    fn scrivi(v: &Value, out: &mut String) {
        match v {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => out.push_str(&n.to_string()),
            Value::String(s) => stringa(s, out),
            Value::Array(a) => {
                out.push('[');
                for (i, x) in a.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    scrivi(x, out);
                }
                out.push(']');
            }
            Value::Object(o) => {
                out.push('{');
                for (i, (k, x)) in o.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    stringa(k, out);
                    out.push_str(": ");
                    scrivi(x, out);
                }
                out.push('}');
            }
        }
    }
    let mut out = String::new();
    scrivi(v, &mut out);
    out
}

/// Tipo MIME dall'estensione (quelli che servono alla web-app e agli allegati).
pub fn tipo_mime(p: &Path) -> &'static str {
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "html" | "htm" => "text/html",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "json" => "application/json",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/vnd.microsoft.icon",
        "txt" | "md" => "text/plain",
        "pdf" => "application/pdf",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "wav" => "audio/x-wav",
        "zip" => "application/zip",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        _ => "application/octet-stream",
    }
}

/// Un file, con `Range` (necessario per scorrere video e audio).
pub async fn servi_file<W: AsyncWrite + Unpin>(w: &mut W, req: &Richiesta, percorso: &Path, tipo: Option<&str>, extra: &str) -> Risultato<()> {
    let tipo = tipo.unwrap_or_else(|| tipo_mime(percorso));
    let totale = tokio::fs::metadata(percorso).await?.len();
    let (mut inizio, mut fine, mut stato) = (0u64, totale.saturating_sub(1), "200 OK");
    let r = req.intestazione("range");
    if let Some(spec) = r.strip_prefix("bytes=").filter(|_| totale > 0) {
        if let Some((a, b)) = spec.split_once('-').filter(|(a, b)| a.bytes().all(|c| c.is_ascii_digit()) && b.bytes().all(|c| c.is_ascii_digit()) && !(a.is_empty() && b.is_empty())) {
            if !a.is_empty() {
                inizio = a.parse().unwrap_or(0);
                fine = if b.is_empty() { totale - 1 } else { b.parse().unwrap_or(totale - 1) };
            } else {
                inizio = totale.saturating_sub(b.parse().unwrap_or(0));
            }
            fine = fine.min(totale - 1);
            if inizio > fine {
                return rispondi(w, "416 Range Not Satisfiable", b"", "text/plain; charset=utf-8", &format!("Content-Range: bytes */{totale}\r\n")).await;
            }
            stato = "206 Partial Content";
        }
    }
    let lunghezza = if totale > 0 { fine - inizio + 1 } else { 0 };
    let intervallo = if stato.starts_with("206") { format!("Content-Range: bytes {inizio}-{fine}/{totale}\r\n") } else { String::new() };
    let testa = format!(
        "HTTP/1.1 {stato}\r\nContent-Type: {tipo}\r\nContent-Length: {lunghezza}\r\nAccept-Ranges: bytes\r\n{INTESTAZIONI_SICUREZZA}{intervallo}{extra}Connection: close\r\n\r\n"
    );
    w.write_all(testa.as_bytes()).await?;
    let mut f = tokio::fs::File::open(percorso).await?;
    f.seek(std::io::SeekFrom::Start(inizio)).await?;
    let mut resto = lunghezza;
    let mut buf = vec![0u8; 65536];
    while resto > 0 {
        let n = f.read(&mut buf[..(resto.min(65536) as usize)]).await?;
        if n == 0 {
            break;
        }
        w.write_all(&buf[..n]).await?;
        resto -= n as u64;
    }
    w.flush().await?;
    Ok(())
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn decodifica_e_parametri() {
        assert_eq!(decodifica("/a%20b/%2e%2e", false), "/a b/..");
        assert_eq!(decodifica("100%", false), "100%");
        assert_eq!(decodifica("a+b", true), "a b");
        let q = leggi_query("nome=..%2F..%2F.bashrc&t=abc&vuoto=");
        assert_eq!(q.get("nome").unwrap(), "../../.bashrc");
        assert_eq!(q.get("t").unwrap(), "abc");
        assert!(!q.contains_key("vuoto"));
        assert_eq!(codifica("a b/ç"), "a+b%2F%C3%A7");
    }

    #[test]
    fn json_come_il_python() {
        let v = serde_json::json!({"a": 1, "b": "è", "c": [true, null]});
        assert_eq!(json_come_python(&v), "{\"a\": 1, \"b\": \"\\u00e8\", \"c\": [true, null]}");
    }
}
