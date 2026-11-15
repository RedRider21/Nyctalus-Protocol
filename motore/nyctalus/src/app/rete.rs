// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! La rete dell'app: codice di rete `NYCT1:…`, reti salvate, ponti nascosti, protezione
//! (camuffamento, rumore, forma), manopole avanzate del motore, nodi «vicini» (rete ospitata nella
//! stessa casa) e lo scaricamento dell'elenco firmato (http e https). Come il Python.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Map, Value};

use crate::Risultato;

pub const PREFISSO_CODICE: &str = "NYCT1:";

fn esadecimale(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

fn stringa<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}

/// Indirizzo `.nyct` (56 caratteri base32 minuscoli + `.nyct`).
pub fn indirizzo_nyct(s: &str) -> bool {
    s.strip_suffix(".nyct").is_some_and(|b| b.len() == 56 && b.bytes().all(|c| c.is_ascii_lowercase() || (b'2'..=b'7').contains(&c)))
}

/// Riga di un PONTE NASCOSTO (segreta): `IP:PORTA,chiave,impronta,chiave-del-ponte`.
pub fn ponte_valido(r: &str) -> bool {
    let parti: Vec<&str> = r.split(',').collect();
    if parti.len() != 4 || !parti[1..].iter().all(|p| esadecimale(p, 64)) {
        return false;
    }
    let Some((host, porta)) = parti[0].rsplit_once(':') else { return false };
    !host.is_empty()
        && host.bytes().all(|c| c.is_ascii_alphanumeric() || b".:-[]".contains(&c))
        && (1..=5).contains(&porta.len())
        && porta.bytes().all(|c| c.is_ascii_digit())
}

/// Un IPv4 della rete di casa (192.168…, 10…, 172.16-31…).
pub fn ip_di_casa(x: &str) -> bool {
    match x.parse::<std::net::Ipv4Addr>() {
        Ok(a) => {
            let o = a.octets();
            o[0] == 10 || (o[0] == 172 && (16..=31).contains(&o[1])) || (o[0] == 192 && o[1] == 168)
        }
        Err(_) => false,
    }
}

fn base64_url(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut acc, mut bit) = (0u32, 0u32);
    for c in s.trim_end_matches('=').bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bit += 6;
        if bit >= 8 {
            bit -= 8;
            out.push((acc >> bit) as u8);
            acc &= (1 << bit) - 1;
        }
    }
    Some(out)
}

/// Camuffamento, rumore e forma della rete (dal codice o dalla configurazione).
pub fn prot_da_codice(cfg: &Value) -> Map<String, Value> {
    let mut out = Map::new();
    let cam = stringa(cfg, "camuffamento");
    if esadecimale(cam, 64) {
        out.insert("camuffamento".into(), json!(cam));
        let r = cfg.get("rumore").and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))).unwrap_or(0.0);
        if r > 0.0 && r <= 10.0 {
            out.insert("rumore".into(), json!(r));
        }
        match stringa(cfg, "forma") {
            "webrtc" => {
                out.insert("forma".into(), json!("webrtc"));
            }
            "h3" => {
                out.insert("forma".into(), json!("h3"));
                let n = stringa(cfg, "nome_h3");
                let ok = !n.is_empty() && n.len() <= 253 && n.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
                    && n.as_bytes()[0].is_ascii_alphanumeric() && n.as_bytes()[n.len() - 1].is_ascii_alphanumeric();
                if ok {
                    out.insert("nome_h3".into(), json!(n));
                }
            }
            _ => {}
        }
    }
    out
}

/// Opzioni del motore per la rete configurata (`--obfuscation`, `--cover`, forma).
pub fn opz_prot(cfg: &Value, rumore: bool) -> Vec<String> {
    let p = prot_da_codice(cfg);
    let mut o = Vec::new();
    if let Some(c) = p.get("camuffamento").and_then(Value::as_str) {
        o.extend(["--obfuscation".to_string(), c.to_string()]);
    }
    match p.get("forma").and_then(Value::as_str) {
        Some("h3") => {
            o.push("--mimic".into());
            if let Some(n) = p.get("nome_h3").and_then(Value::as_str) {
                o.extend(["--mimic-name".to_string(), n.to_string()]);
            }
        }
        Some("webrtc") => o.push("--webrtc".into()),
        _ => {}
    }
    if rumore {
        if let Some(r) = p.get("rumore").and_then(Value::as_f64) {
            o.extend(["--cover".to_string(), formato_g(r)]);
        }
    }
    o
}

/// Come `"%g" % x` del Python per i valori che servono (0,2 → «0.2», 1.0 → «1»).
fn formato_g(x: f64) -> String {
    let s = format!("{x}");
    s.strip_suffix(".0").map(String::from).unwrap_or(s)
}

/// Codice di rete → {nome, directory_url, authority, rubrica?, casella?, lan?, protezione…}.
pub fn leggi_codice(testo: &str) -> Result<Map<String, Value>, String> {
    let testo = testo.trim();
    if !testo.starts_with(PREFISSO_CODICE) || testo.len() > 2000 {
        return Err(format!("non è un codice di rete Nyctalus (inizia con {PREFISSO_CODICE})"));
    }
    let cfg: Value = base64_url(&testo[PREFISSO_CODICE.len()..])
        .and_then(|b| serde_json::from_slice(&b).ok())
        .ok_or("codice di rete rovinato: copialo per intero")?;
    let (url, aut) = (stringa(&cfg, "directory_url"), stringa(&cfg, "authority"));
    let url_ok = (url.starts_with("http://") || url.starts_with("https://")) && url.len() > 8 && !url.chars().any(char::is_whitespace);
    if !url_ok || !esadecimale(aut, 64) {
        return Err("codice di rete incompleto".into());
    }
    let nome: String = Some(stringa(&cfg, "nome")).filter(|n| !n.is_empty()).unwrap_or("Rete Nyctalus").chars().take(60).collect();
    let mut rete = Map::new();
    rete.insert("nome".into(), json!(nome));
    rete.insert("directory_url".into(), json!(url));
    rete.insert("authority".into(), json!(aut));
    for k in ["rubrica", "casella"] {
        if indirizzo_nyct(stringa(&cfg, k)) {
            rete.insert(k.into(), json!(stringa(&cfg, k)));
        }
    }
    if ip_di_casa(stringa(&cfg, "lan")) {
        rete.insert("lan".into(), json!(stringa(&cfg, "lan")));
    }
    rete.extend(prot_da_codice(&cfg));
    Ok(rete)
}

/// Impronta breve della rete (uguale a quella di nyctalus-rete sul PC).
pub fn impronta_rete(autorita: &str) -> String {
    let b: Vec<u8> = (0..autorita.len() / 2).filter_map(|i| u8::from_str_radix(&autorita[2 * i..2 * i + 2], 16).ok()).collect();
    let h = ring::digest::digest(&ring::digest::SHA256, &b);
    let x: String = h.as_ref()[..4].iter().map(|v| format!("{v:02X}")).collect();
    format!("{}-{}", &x[..4], &x[4..])
}

/// Le reti importate (si passa dall'una all'altra senza reimportare).
pub fn reti_salvate(cfg: &Value) -> Vec<Map<String, Value>> {
    let mut reti: Vec<Map<String, Value>> = cfg.get("reti").and_then(Value::as_array).map(|a| {
        a.iter().filter_map(|r| r.as_object().cloned())
            .filter(|r| !stringa(&Value::Object(r.clone()), "directory_url").is_empty() && !stringa(&Value::Object(r.clone()), "authority").is_empty())
            .collect()
    }).unwrap_or_default();
    let (url, aut) = (stringa(cfg, "directory_url"), stringa(cfg, "authority"));
    if !url.is_empty() && !aut.is_empty() && !reti.iter().any(|r| r.get("authority").and_then(Value::as_str) == Some(aut) && r.get("directory_url").and_then(Value::as_str) == Some(url)) {
        let nome = Some(stringa(cfg, "nome_rete")).filter(|n| !n.is_empty()).unwrap_or("Rete Nyctalus");
        let mut r = Map::new();
        r.insert("nome".into(), json!(nome));
        r.insert("directory_url".into(), json!(url));
        r.insert("authority".into(), json!(aut));
        reti.push(r);
    }
    reti
}

/// I ponti nascosti della rete in uso (tenuti per autorità).
pub fn ponti_di(cfg: &Value) -> Vec<String> {
    let aut = stringa(cfg, "authority");
    cfg.get("ponti").and_then(|p| p.get(aut)).and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).filter(|r| ponte_valido(r)).map(String::from).collect())
        .unwrap_or_default()
}

/// Variabili d'ambiente delle Impostazioni avanzate (finestra, congestione, ripiego, riuso…).
pub fn manopole_motore(cfg: &Value) -> Vec<(String, String)> {
    let mut e = Vec::new();
    let k = cfg.get("finestra_kb").and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())));
    if let Some(k) = k.filter(|k| (16..=4096).contains(k)) {
        e.push(("NYCTALUS_FINESTRA_KB".into(), k.to_string()));
    }
    if let c @ ("cubic" | "newreno") = stringa(cfg, "congestione") {
        e.push(("NYCTALUS_CONGESTIONE".into(), c.into()));
    }
    match stringa(cfg, "tcp") {
        "sempre" => e.push(("NYCTALUS_TCP".into(), "1".into())),
        "mai" => e.push(("NYCTALUS_TCP".into(), "0".into())),
        _ => {}
    }
    if stringa(cfg, "riuso") == "no" {
        e.push(("NYCTALUS_RIUSO".into(), "0".into()));
    }
    if let Some(u) = cfg.get("uscite").and_then(Value::as_i64).filter(|u| (1..=4).contains(u)) {
        e.push(("NYCTALUS_USCITE".into(), u.to_string()));
    }
    match stringa(cfg, "costante") {
        "uso" => e.push(("NYCTALUS_COSTANTE".into(), "60".into())),
        "sempre" => {
            e.push(("NYCTALUS_COSTANTE".into(), "60".into()));
            e.push(("NYCTALUS_COSTANTE_SEMPRE".into(), "1".into()));
        }
        _ => {}
    }
    e
}

fn parti_url(url: &str) -> Option<(bool, String, u16, String)> {
    let (https, resto) = if let Some(r) = url.strip_prefix("https://") { (true, r) } else { (false, url.strip_prefix("http://")?) };
    let (hostporta, percorso) = resto.split_once('/').map(|(a, b)| (a, format!("/{b}"))).unwrap_or((resto, "/".into()));
    let (host, porta) = match hostporta.rsplit_once(':') {
        Some((h, p)) if !h.ends_with(']') || p.parse::<u16>().is_ok() => (h.trim_matches(['[', ']']).to_string(), p.parse().ok()?),
        _ => (hostporta.trim_matches(['[', ']']).to_string(), if https { 443 } else { 80 }),
    };
    Some((https, host, porta, percorso))
}

fn risolvi4(host: &str, porta: u16) -> Option<SocketAddr> {
    (host, porta).to_socket_addrs().ok()?.find(SocketAddr::is_ipv4)
}

/// Rete ospitata nella casa in cui si trova il telefono: la regola `NYCTALUS_VICINI`
/// («IP pubblico=IP interno») e l'URL dell'elenco con l'indirizzo interno. Lontano da casa
/// l'indirizzo interno non risponde e non cambia niente.
pub fn vicini(cfg: &Value) -> (String, String) {
    let url = stringa(cfg, "directory_url").to_string();
    let lan = stringa(cfg, "lan");
    if url.is_empty() || !ip_di_casa(lan) {
        return (String::new(), url);
    }
    let Some((https, host, porta, percorso)) = parti_url(&url) else { return (String::new(), url) };
    let Some(pubblico) = risolvi4(&host, porta).map(|a| a.ip().to_string()) else { return (String::new(), url) };
    if ip_di_casa(&pubblico) || pubblico.starts_with("127.") {
        return (String::new(), url);
    }
    let interno: SocketAddr = match format!("{lan}:{porta}").parse() {
        Ok(a) => a,
        Err(_) => return (String::new(), url),
    };
    if TcpStream::connect_timeout(&interno, Duration::from_millis(800)).is_err() {
        return (String::new(), url);
    }
    let schema = if https { "https" } else { "http" };
    (format!("{pubblico}={lan}"), format!("{schema}://{lan}:{porta}{percorso}"))
}

/// Scarica l'elenco firmato (http o https; al massimo 4 MB). La firma la controlla il motore.
pub fn scarica(url: &str) -> Risultato<Vec<u8>> {
    let (https, host, porta, percorso) = parti_url(url).ok_or("indirizzo dell'elenco non valido")?;
    let ind = (host.as_str(), porta).to_socket_addrs()?.next().ok_or("nome non risolto")?;
    let tcp = TcpStream::connect_timeout(&ind, Duration::from_secs(25))?;
    tcp.set_read_timeout(Some(Duration::from_secs(25)))?;
    tcp.set_write_timeout(Some(Duration::from_secs(25)))?;
    let richiesta = format!("GET {percorso} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: nyctalus\r\nAccept: */*\r\nConnection: close\r\n\r\n");
    let mut grezzo = Vec::new();
    if https {
        let mut radici = rustls::RootCertStore::empty();
        for c in certificati_di_sistema() {
            let _ = radici.add(c);
        }
        let conf = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()?
            .with_root_certificates(radici)
            .with_no_client_auth();
        let nome = rustls::pki_types::ServerName::try_from(host.clone())?;
        let mut sessione = rustls::ClientConnection::new(Arc::new(conf), nome)?;
        let mut tcp = tcp;
        let mut flusso = rustls::Stream::new(&mut sessione, &mut tcp);
        flusso.write_all(richiesta.as_bytes())?;
        let _ = flusso.take(5 * 1024 * 1024).read_to_end(&mut grezzo);
    } else {
        let mut tcp = tcp;
        tcp.write_all(richiesta.as_bytes())?;
        tcp.take(5 * 1024 * 1024).read_to_end(&mut grezzo)?;
    }
    let fine = grezzo.windows(4).position(|w| w == b"\r\n\r\n").ok_or("risposta non valida")?;
    let testa = String::from_utf8_lossy(&grezzo[..fine]).to_string();
    let stato = testa.split_whitespace().nth(1).unwrap_or("");
    if stato != "200" {
        return Err(format!("l'elenco non si scarica (HTTP {stato})").into());
    }
    let mut corpo = grezzo[fine + 4..].to_vec();
    if testa.to_ascii_lowercase().contains("transfer-encoding: chunked") {
        corpo = dechunk(&corpo);
    }
    Ok(corpo)
}

fn dechunk(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(fine) = b[i..].windows(2).position(|w| w == b"\r\n") {
        let n = usize::from_str_radix(String::from_utf8_lossy(&b[i..i + fine]).split(';').next().unwrap_or("").trim(), 16).unwrap_or(0);
        i += fine + 2;
        if n == 0 || i + n > b.len() {
            break;
        }
        out.extend_from_slice(&b[i..i + n]);
        i += n + 2;
    }
    out
}

/// Le autorità di certificazione del sistema (Android: cacerts di sistema e di Conscrypt;
/// Linux: /etc/ssl/certs), in PEM.
fn certificati_di_sistema() -> Vec<rustls::pki_types::CertificateDer<'static>> {
    let mut out = Vec::new();
    let file_singoli = ["/etc/ssl/certs/ca-certificates.crt", "/etc/pki/tls/certs/ca-bundle.crt", "/data/data/com.termux/files/usr/etc/tls/cert.pem"];
    let cartelle = ["/apex/com.android.conscrypt/cacerts", "/system/etc/security/cacerts"];
    let mut testi = Vec::new();
    for f in file_singoli {
        if let Ok(t) = std::fs::read_to_string(f) {
            testi.push(t);
        }
    }
    if testi.is_empty() {
        for c in cartelle {
            if let Ok(voci) = std::fs::read_dir(c) {
                for v in voci.flatten() {
                    if let Ok(t) = std::fs::read_to_string(v.path()) {
                        testi.push(t);
                    }
                }
            }
        }
    }
    for t in testi {
        for blocco in t.split("-----BEGIN CERTIFICATE-----").skip(1) {
            let corpo: String = blocco.split("-----END CERTIFICATE-----").next().unwrap_or("").chars().filter(|c| !c.is_whitespace()).collect();
            if let Some(der) = base64_std(&corpo) {
                out.push(rustls::pki_types::CertificateDer::from(der));
            }
        }
    }
    out
}

fn base64_std(s: &str) -> Option<Vec<u8>> {
    base64_url(s)
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn codice_di_rete() {
        let cfg = json!({"v": 1, "nome": "Casa", "directory_url": "http://87.7.168.88:18800/directory.txt",
            "authority": "ab".repeat(32), "camuffamento": "cd".repeat(32), "rumore": 0.2, "lan": "192.168.1.14",
            "rubrica": format!("{}.nyct", "a".repeat(56))});
        let b = serde_json::to_vec(&cfg).unwrap();
        let mut enc = String::new();
        let alfabeto = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        for gruppo in b.chunks(3) {
            let n = gruppo.iter().fold(0u32, |a, &x| (a << 8) | x as u32) << (8 * (3 - gruppo.len()));
            for i in 0..=gruppo.len() {
                enc.push(alfabeto[((n >> (18 - 6 * i)) & 63) as usize] as char);
            }
        }
        let r = leggi_codice(&format!("NYCT1:{enc}")).unwrap();
        assert_eq!(r["nome"], "Casa");
        assert_eq!(r["lan"], "192.168.1.14");
        assert_eq!(r["rumore"], 0.2);
        assert!(r.contains_key("rubrica"));
        assert!(leggi_codice("NYCT1:@@@").is_err());
        assert!(leggi_codice("ciao").is_err());
        assert_eq!(opz_prot(&Value::Object(r), true), vec!["--obfuscation".to_string(), "cd".repeat(32), "--cover".into(), "0.2".into()]);
    }

    #[test]
    fn piccole_regole() {
        assert!(ip_di_casa("192.168.1.14") && ip_di_casa("10.0.0.1") && ip_di_casa("172.20.1.1"));
        assert!(!ip_di_casa("8.8.8.8") && !ip_di_casa("172.32.0.1") && !ip_di_casa("x"));
        assert_eq!(impronta_rete(&"00".repeat(32)).len(), 9);
        assert!(ponte_valido(&format!("1.2.3.4:443,{},{},{}", "a".repeat(64), "b".repeat(64), "c".repeat(64))));
        assert!(!ponte_valido("1.2.3.4:443,x,y,z"));
        assert_eq!(formato_g(0.2), "0.2");
        assert_eq!(formato_g(1.0), "1");
        assert_eq!(parti_url("https://a.b/c/d.txt"), Some((true, "a.b".into(), 443, "/c/d.txt".into())));
        assert_eq!(parti_url("http://1.2.3.4:18800/directory.txt"), Some((false, "1.2.3.4".into(), 18800, "/directory.txt".into())));
        assert_eq!(dechunk(b"3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n"), b"abcde");
    }
}
