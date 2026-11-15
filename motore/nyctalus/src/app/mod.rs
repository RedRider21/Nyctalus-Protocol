// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! `nyctalus app` — l'orchestratore dell'app del telefono, dentro il motore.
//!
//! È lo stesso programma di `mobile/orchestratore/nyctalus_mobile.py` (stesse rotte, stessi file
//! in `~/.nyctalus-mobile`, stesse regole di sicurezza), così l'app Android funziona SENZA Termux:
//! l'APK porta il motore e lancia `libnyctalus.so app`. Le prove dell'orchestratore girano
//! identiche contro tutti e due (`NYCTALUS_ORCHESTRATORE=rust`). Fasi: `documenti/APP-ANDROID-NATIVA.md`.
//!
//! Sicurezza: su Android qualunque app può collegarsi a 127.0.0.1, quindi OGNI richiesta deve
//! presentare il segreto (`~/.nyctalus-mobile/token`, 600): `?t=SEGRETO` una volta → cookie.
//! Le richieste che cambiano qualcosa devono venire dalla nostra pagina (intestazione Origin).

pub mod http;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tokio::io::BufReader;
use tokio::net::{TcpListener, TcpStream};

use crate::Risultato;
use http::Richiesta;

pub const VERSIONE: &str = "0.1";

/// Cartelle e file dell'app (tutto sotto HOME, come il Python).
pub struct Percorsi {
    pub home: PathBuf,
    pub dati: PathBuf,
    pub dati_chat: PathBuf,
    pub config: PathBuf,
    pub segreto: PathBuf,
    pub invio: PathBuf,
    pub cache: PathBuf,
    pub webapp: PathBuf,
}

impl Percorsi {
    fn da_ambiente(webapp: Option<PathBuf>) -> Percorsi {
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()));
        let dati = home.join(".nyctalus-mobile");
        Percorsi {
            dati_chat: home.join(".nyctalus-chat"),
            config: dati.join("config.json"),
            segreto: dati.join("token"),
            invio: dati.join("invio"),
            cache: dati.join("cache"),
            webapp: webapp.or_else(|| std::env::var("NYCTALUS_WEBAPP").ok().map(PathBuf::from)).unwrap_or_else(trova_webapp),
            dati,
            home,
        }
    }
}

/// La web-app: accanto al motore (pacchetto: bin/nyctalus → ../webapp) o nel progetto (mobile/webapp).
fn trova_webapp() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        for antenato in exe.ancestors().skip(1).take(5) {
            for c in [antenato.join("webapp"), antenato.join("mobile").join("webapp")] {
                if c.join("index.html").is_file() {
                    return c;
                }
            }
        }
    }
    PathBuf::from("webapp")
}

/// Porte (blocco 18xxx, come il Python); modificabili in config.json → "porte".
#[derive(Clone)]
pub struct Porte {
    pub http: u16,
    pub controllo: u16,
    pub backend: u16,
    pub socks: u16,
    pub socks_locale: u16,
    pub monitor: u16,
    pub nodi: Vec<u16>,
    pub socks_browser: u16,
}

impl Porte {
    fn da_config(cfg: &Value) -> Porte {
        let p = cfg.get("porte").cloned().unwrap_or(Value::Null);
        let n = |k: &str, d: u16| p.get(k).and_then(Value::as_u64).and_then(|v| u16::try_from(v).ok()).unwrap_or(d);
        let nodi = p.get("nodi").and_then(Value::as_array)
            .map(|a| a.iter().filter_map(|v| v.as_u64().and_then(|x| u16::try_from(x).ok())).collect::<Vec<_>>())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| vec![18840, 18841, 18842, 18843]);
        let http = std::env::var("NYCTALUS_MOBILE_PORT").ok().and_then(|v| v.parse().ok()).unwrap_or_else(|| n("http", 18731));
        Porte {
            http,
            controllo: n("controllo", 18091),
            backend: n("backend", 18082),
            socks: n("socks", 18080),
            socks_locale: n("socks_locale", 18086),
            monitor: n("monitor", 18596),
            nodi,
            socks_browser: n("socks_browser", 18733),
        }
    }
}

pub fn leggi_config(p: &Percorsi) -> Value {
    std::fs::read_to_string(&p.config).ok().and_then(|t| serde_json::from_str(&t).ok()).filter(Value::is_object).unwrap_or_else(|| json!({}))
}

/// Il segreto d'accesso vive nel file; si rilegge se cambia (l'APK può riscriverlo).
pub struct Segreto {
    file: PathBuf,
    cache: Mutex<(Option<String>, Option<std::time::SystemTime>)>,
}

impl Segreto {
    fn nuovo(file: PathBuf) -> Segreto {
        Segreto { file, cache: Mutex::new((None, None)) }
    }

    pub fn valore(&self) -> Option<String> {
        let mut c = self.cache.lock().unwrap();
        match std::fs::metadata(&self.file).and_then(|m| m.modified()) {
            Ok(mt) => {
                if c.1 != Some(mt) {
                    let v = std::fs::read_to_string(&self.file).unwrap_or_default().trim().to_string();
                    *c = ((v.len() >= 16).then_some(v), Some(mt));
                }
            }
            Err(_) => *c = (None, None),
        }
        c.0.clone()
    }

    fn crea_se_manca(&self) -> Risultato<String> {
        if let Some(v) = self.valore() {
            return Ok(v);
        }
        if let Some(d) = self.file.parent() {
            std::fs::create_dir_all(d)?;
        }
        let mut b = [0u8; 16];
        getrandom::fill(&mut b).map_err(|e| e.to_string())?;
        let v: String = b.iter().map(|x| format!("{x:02x}")).collect();
        scrivi_privato(&self.file, v.as_bytes())?;
        self.valore().ok_or_else(|| "segreto non scritto".into())
    }

    pub fn verifica(&self, candidato: Option<&str>) -> bool {
        match (self.valore(), candidato) {
            (Some(a), Some(c)) if !c.is_empty() => uguali_tempo_costante(a.as_bytes(), c.as_bytes()),
            _ => false,
        }
    }
}

fn uguali_tempo_costante(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Scrive un file leggibile solo dal proprietario (600), passando da un file temporaneo.
pub fn scrivi_privato(f: &Path, dati: &[u8]) -> std::io::Result<()> {
    let tmp = f.with_extension("tmp");
    {
        use std::io::Write;
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.mode(0o600);
        }
        let mut h = o.open(&tmp)?;
        h.write_all(dati)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, f)
}

/// Stato dei servizi (motore della chat, rete, servizio .nyct), come `Servizi.info()` del Python.
#[derive(Default)]
pub struct StatoServizi {
    pub stato: String,
    pub dettaglio: String,
    pub modalita: String,
    pub indirizzo: String,
    pub chiusa: bool,
}

pub struct App {
    pub p: Percorsi,
    pub porte: Porte,
    pub segreto: Segreto,
    pub servizi: Mutex<StatoServizi>,
}

impl App {
    pub fn info(&self) -> Value {
        let s = self.servizi.lock().unwrap();
        json!({
            "stato": if s.stato.is_empty() { "fermo" } else { s.stato.as_str() },
            "dettaglio": s.dettaglio,
            "modalita": s.modalita,
            "rete": leggi_config(&self.p).get("nome_rete").and_then(Value::as_str).unwrap_or(""),
            "socks_browser": self.porte.socks_browser,
            "indirizzo": s.indirizzo,
            "binario": std::env::current_exe().is_ok(),
            "chiusa": s.chiusa && s.indirizzo.is_empty(),
            "versione": VERSIONE,
        })
    }

    fn origine_ammessa(&self, o: &str) -> bool {
        o == format!("http://127.0.0.1:{}", self.porte.http) || o == format!("http://localhost:{}", self.porte.http)
    }
}

fn log(msg: &str) {
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    // ora locale semplice (UTC se il fuso non è noto): solo per leggere i registri
    let (h, m, s) = ((t / 3600) % 24, (t / 60) % 60, t % 60);
    println!("{h:02}:{m:02}:{s:02} {msg}");
}

/// `nyctalus app [--webapp DIR]`.
pub async fn esegui(webapp: Option<PathBuf>) -> Risultato<()> {
    let p = Percorsi::da_ambiente(webapp);
    let cfg = leggi_config(&p);
    let porte = Porte::da_config(&cfg);
    for d in [&p.dati, &p.invio, &p.cache] {
        std::fs::create_dir_all(d)?;
    }
    let segreto = Segreto::nuovo(p.segreto.clone());
    let s = segreto.crea_se_manca()?;
    let app = Arc::new(App { p, porte, segreto, servizi: Mutex::new(StatoServizi::default()) });
    let ascolto = TcpListener::bind(("127.0.0.1", app.porte.http)).await?;
    log(&format!("Nyctalus mobile {VERSIONE} — http://127.0.0.1:{}/?t={s}", app.porte.http));
    log(&format!("web-app da: {}", app.p.webapp.display()));
    loop {
        let (conn, _) = ascolto.accept().await?;
        let app = app.clone();
        tokio::spawn(async move {
            let _ = gestisci(app, conn).await;
        });
    }
}

async fn gestisci(app: Arc<App>, conn: TcpStream) -> Risultato<()> {
    let (lettura, mut scrittura) = conn.into_split();
    let mut lettore = BufReader::new(lettura);
    let req = match http::leggi_richiesta(&mut lettore).await {
        Ok(r) => r,
        Err(_) => return Ok(()),
    };
    if let Err(e) = instrada(&app, &req, &mut lettore, &mut scrittura).await {
        log(&format!("errore richiesta {}: {e}", req.percorso));
        let _ = http::testo(&mut scrittura, "500 Internal Server Error", "errore interno").await;
    }
    Ok(())
}

async fn instrada<R, W>(app: &Arc<App>, req: &Richiesta, _lettore: &mut BufReader<R>, w: &mut W) -> Risultato<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    // 1) accesso: ?t=SEGRETO una volta → cookie; poi il cookie
    if let Some(t) = req.param("t") {
        if app.segreto.verifica(Some(t)) {
            let resto: Vec<String> = req.query.iter().filter(|(k, _)| *k != "t").map(|(k, v)| format!("{}={}", http::codifica(k), http::codifica(v))).collect();
            let dest = if resto.is_empty() { req.percorso.clone() } else { format!("{}?{}", req.percorso, resto.join("&")) };
            return http::rispondi(w, "303 See Other", b"", "text/plain; charset=utf-8", &format!(
                "Location: {dest}\r\nSet-Cookie: nyct={t}; Path=/; Max-Age=31536000; HttpOnly; SameSite=Strict\r\n")).await;
        }
    }
    if !app.segreto.verifica(req.cookie("nyct").as_deref()) {
        return http::testo(w, "403 Forbidden", "Accesso negato: apri Nyctalus dall'app, oppure usa l'indirizzo con ?t=… stampato da nyctalus-mobile.").await;
    }
    // le richieste che cambiano qualcosa devono venire dalla nostra pagina
    if req.metodo == "POST" {
        if let Some(o) = req.h.get("origin") {
            if !app.origine_ammessa(o) {
                return http::testo(w, "403 Forbidden", "origine non ammessa").await;
            }
        }
    }
    let p = req.percorso.as_str();
    if p == "/api/stato" {
        return http::json(w, "200 OK", &app.info()).await;
    }
    if p.starts_with("/api/") || p == "/ws" || p.starts_with("/ws/") {
        return http::testo(w, "404 Not Found", "non trovato").await;
    }
    servi_statico(app, req, w).await
}

async fn servi_statico<W: tokio::io::AsyncWrite + Unpin>(app: &Arc<App>, req: &Richiesta, w: &mut W) -> Risultato<()> {
    let rel = req.percorso.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    let base = match app.p.webapp.canonicalize() {
        Ok(b) => b,
        Err(_) => return http::testo(w, "404 Not Found", "non trovato").await,
    };
    match base.join(rel).canonicalize() {
        Ok(f) if f.starts_with(&base) && f.is_file() => http::servi_file(w, req, &f, None, "").await,
        _ => http::testo(w, "404 Not Found", "non trovato").await,
    }
}
