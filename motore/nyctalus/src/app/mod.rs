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

pub mod file;
pub mod http;
pub mod rete;
pub mod servizi;
pub mod ws;

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
    pub processi: servizi::Processi,
    /// segreto della porta di controllo del motore della chat (lo usa anche nyctalus-cli)
    pub segreto_motore: String,
    pub galleria: file::CacheGalleria,
    pub miniature_ripulite: Mutex<bool>,
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

pub fn log(msg: &str) {
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
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|e| e.to_string())?;
    let segreto_motore: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let app = Arc::new(App { p, porte, segreto, servizi: Mutex::new(StatoServizi::default()), processi: servizi::Processi::default(), segreto_motore,
        galleria: file::CacheGalleria::default(), miniature_ripulite: Mutex::new(false) });
    let ascolto = TcpListener::bind(("127.0.0.1", app.porte.http)).await?;
    log(&format!("Nyctalus mobile {VERSIONE} — http://127.0.0.1:{}/?t={s}", app.porte.http));
    log(&format!("web-app da: {}", app.p.webapp.display()));
    // spegnendo l'app (SIGTERM/SIGINT) si spengono anche i servizi: nessun processo orfano
    {
        let app = app.clone();
        tokio::spawn(async move {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};
                if let (Ok(mut t), Ok(mut i)) = (signal(SignalKind::terminate()), signal(SignalKind::interrupt())) {
                    tokio::select! { _ = t.recv() => {}, _ = i.recv() => {} }
                }
            }
            #[cfg(not(unix))]
            let _ = tokio::signal::ctrl_c().await;
            servizi::ferma(&app);
            std::process::exit(0);
        });
    }
    if std::env::var("NYCTALUS_AVVIO_AUTOMATICO").map(|v| v == "1").unwrap_or(true) {
        servizi::avvia(&app);
    }
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
    let post = req.metodo == "POST";
    if p == "/ws" && req.intestazione("upgrade").eq_ignore_ascii_case("websocket") {
        return ws::ponte_chat(app, req, _lettore, w).await;
    }
    if p == "/ws/terminale" {
        if !app.origine_ammessa(req.intestazione("origin")) {
            return http::testo(w, "403 Forbidden", "origine non ammessa").await;
        }
        // il ⌨ Terminale (nyctalus-cli) c'è solo nella versione con Termux
        return http::testo(w, "404 Not Found", "terminale disponibile solo con Termux").await;
    }
    if p == "/api/notifiche" {
        return ws::flusso_notifiche(app, w).await;
    }
    if p == "/api/chiamata/rifiuta" && post {
        // pulsante «Rifiuta» della notifica, ad app chiusa: si rifiuta senza aprire nulla
        let (da, id) = (req.param("da").unwrap_or(""), req.param("id").unwrap_or(""));
        if !rete::indirizzo_nyct(da) || id.is_empty() || id.len() > 32 || !id.bytes().all(|c| c.is_ascii_alphanumeric()) {
            return http::json(w, "400 Bad Request", &json!({"ok": false, "errore": "parametri non validi"})).await;
        }
        let ok = ws::comando_motore(app, &format!("CALLNO {da} {id}")).await;
        return http::json(w, "200 OK", &json!({"ok": ok})).await;
    }
    if p == "/api/stato" {
        return http::json(w, "200 OK", &app.info()).await;
    }
    let percorso_p = || req.param("p").and_then(file::da_hex_percorso);
    if p == "/api/file" {
        let f = file::in_chiaro(app, percorso_p().and_then(|x| file::leggibile(app, &x))).await;
        return match f {
            Some(f) => http::servi_file(w, req, &f, None, "").await,
            None => http::testo(w, "404 Not Found", "file non disponibile").await,
        };
    }
    if p == "/api/miniatura" {
        let orig = percorso_p().and_then(|x| file::leggibile(app, &x));
        let f = file::in_chiaro(app, orig.clone()).await;
        let Some(f) = f else { return http::testo(w, "404 Not Found", "nessuna miniatura").await };
        // file cifrato: miniatura nella vista in chiaro (mai sul disco); ricevuti in chiaro: a parte
        let dove = if Some(&f) != orig.as_ref() { f.parent().map(Path::to_path_buf) }
                   else if file::e_ricevuto(app, &f) { Some(app.p.cache.join("ricevuti")) } else { None };
        let (a, f2) = (app.clone(), f.clone());
        let m = tokio::task::spawn_blocking(move || file::miniatura(&a, &f2, dove)).await?;
        return match m {
            Some(m) => {
                let tipo = if m == f { None } else { Some("image/jpeg") };
                http::servi_file(w, req, &m, tipo, "Cache-Control: private, max-age=86400\r\n").await
            }
            None => http::testo(w, "404 Not Found", "nessuna miniatura").await,
        };
    }
    if p == "/api/galleria" {
        let da: usize = req.param("da").and_then(|v| v.parse().ok()).unwrap_or(0);
        let n: usize = req.param("n").and_then(|v| v.parse().ok()).unwrap_or(30).clamp(1, 60);
        let a = app.clone();
        let v = tokio::task::spawn_blocking(move || file::galleria(&a, da, n)).await?;
        return http::json(w, "200 OK", &v).await;
    }
    if p == "/api/apri" && post {
        let f = file::in_chiaro(app, percorso_p().and_then(|x| file::leggibile(app, &x))).await;
        let v = f.map(|f| file::apri_o_condividi(&f, false)).unwrap_or_else(|| json!({"ok": false}));
        return http::json(w, "200 OK", &v).await;
    }
    if p == "/api/carica" && post {
        let n: i64 = req.intestazione("content-length").parse().unwrap_or(-1);
        let (stato, v) = file::carica(app, req.param("nome").unwrap_or("file"), n, _lettore).await;
        return http::json(w, stato, &v).await;
    }
    if p == "/api/sfoglia" {
        let d = req.param("d").and_then(file::da_hex_percorso).map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        let a = app.clone();
        let v = tokio::task::spawn_blocking(move || file::sfoglia(&a, &d)).await?;
        return http::json(w, "200 OK", &v).await;
    }
    if p == "/api/archivio" {
        let a = app.clone();
        let v = tokio::task::spawn_blocking(move || file::archivio(&a)).await?;
        return http::json(w, "200 OK", &v).await;
    }
    if p == "/api/elimina" && post {
        let v = match file::nell_archivio(app, req.param("p").unwrap_or("")) {
            None => json!({"ok": false, "errore": "file non nell'archivio"}),
            Some(f) => match std::fs::remove_file(&f) {
                Ok(()) => json!({"ok": true}),
                Err(e) => json!({"ok": false, "errore": e.to_string()}),
            },
        };
        return http::json(w, "200 OK", &v).await;
    }
    if p == "/api/condividi" && post {
        let f = file::in_chiaro(app, file::nell_archivio(app, req.param("p").unwrap_or(""))).await;
        let v = f.map(|f| file::apri_o_condividi(&f, true)).unwrap_or_else(|| json!({"ok": false}));
        return http::json(w, "200 OK", &v).await;
    }
    if p == "/api/rete" {
        return if post { cambia_rete(app, req, _lettore, w).await } else { http::json(w, "200 OK", &info_rete(app)).await };
    }
    if p == "/api/impostazioni" {
        if post {
            return imposta_avanzate(app, req, _lettore, w).await;
        }
        let cfg = leggi_config(&app.p);
        let o = |k: &str, d: Value| cfg.get(k).cloned().unwrap_or(d);
        return http::json(w, "200 OK", &json!({"finestra_kb": o("finestra_kb", json!(256)), "congestione": o("congestione", json!("bbr")),
            "tcp": o("tcp", json!("auto")), "riuso": o("riuso", json!("si")), "uscite": o("uscite", json!(1)), "costante": o("costante", json!("no"))})).await;
    }
    if p == "/api/servizi/avvia" && post {
        servizi::avvia(app);
        return http::json(w, "200 OK", &app.info()).await;
    }
    if p == "/api/servizi/ferma" && post {
        let a = app.clone();
        tokio::task::spawn_blocking(move || servizi::ferma(&a)).await?;
        return http::json(w, "200 OK", &app.info()).await;
    }
    if p == "/api/esci" && post {
        http::json(w, "200 OK", &json!({"ok": true})).await?;
        let a = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(300));
            servizi::ferma(&a);
            std::process::exit(0);
        });
        return Ok(());
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

/// Corpo JSON di una richiesta (al massimo 4 KB, come il Python).
async fn corpo_json<R: tokio::io::AsyncRead + Unpin>(req: &Richiesta, lettore: &mut BufReader<R>) -> Result<Value, &'static str> {
    use tokio::io::AsyncReadExt;
    let n: i64 = req.intestazione("content-length").parse().unwrap_or(-1);
    if !(1..=4096).contains(&n) {
        return Err("richiesta non valida");
    }
    let mut b = vec![0u8; n as usize];
    lettore.read_exact(&mut b).await.map_err(|_| "richiesta non valida")?;
    serde_json::from_slice(&b).map_err(|_| "JSON non valido")
}

pub fn scrivi_config(p: &Percorsi, cfg: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(&p.dati)?;
    let mut t = serde_json::to_string_pretty(cfg).unwrap_or_else(|_| "{}".into());
    t.push('\n');
    scrivi_privato(&p.config, t.as_bytes())
}

fn info_rete(app: &App) -> Value {
    let cfg = leggi_config(&app.p);
    let s = |k: &str| cfg.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let condivisa = !s("directory_url").is_empty() && !s("authority").is_empty();
    let reti: Vec<Value> = rete::reti_salvate(&cfg).iter().map(|r| {
        let ra = r.get("authority").and_then(Value::as_str).unwrap_or("");
        let ru = r.get("directory_url").and_then(Value::as_str).unwrap_or("");
        json!({"nome": r.get("nome").cloned().unwrap_or(json!("")), "impronta": rete::impronta_rete(ra),
               "attiva": condivisa && ra == s("authority") && ru == s("directory_url")})
    }).collect();
    let prot = rete::prot_da_codice(&cfg);
    let se = |v: String| if condivisa { v } else { String::new() };
    json!({
        "condivisa": condivisa,
        "nome": s("nome_rete"),
        "rubrica": se(s("rubrica")),
        "url": se(s("directory_url")),
        "impronta": if condivisa { rete::impronta_rete(&s("authority")) } else { String::new() },
        // solo gli indirizzi: le chiavi dei ponti non escono mai
        "ponti": if condivisa { rete::ponti_di(&cfg).iter().map(|r| r.split(',').next().unwrap_or("").to_string()).collect::<Vec<_>>() } else { vec![] },
        "forma": se(prot.get("forma").and_then(Value::as_str).unwrap_or("").to_string()),
        "nome_h3": se(prot.get("nome_h3").and_then(Value::as_str).unwrap_or("").to_string()),
        "reti": reti,
    })
}

const CHIAVI_RETE: [&str; 7] = ["rubrica", "casella", "camuffamento", "rumore", "forma", "nome_h3", "lan"];

fn applica_rete(cfg: &mut serde_json::Map<String, Value>, rete: &serde_json::Map<String, Value>) {
    for k in ["directory_url", "authority"] {
        cfg.insert(k.into(), rete.get(k).cloned().unwrap_or(Value::Null));
    }
    cfg.insert("nome_rete".into(), rete.get("nome").cloned().unwrap_or(json!("")));
    for k in CHIAVI_RETE {
        cfg.remove(k);
        if let Some(v) = rete.get(k).filter(|v| !v.is_null() && v.as_str() != Some("")) {
            cfg.insert(k.into(), v.clone());
        }
    }
}

/// Cambia la rete di tutto il corredo, poi riavvia i servizi:
/// {"codice"} importa · {"usa": N} · {"togli": N} · {"prova": true} · {"ponte": RIGA} · {"ponte_togli": IP:PORTA|"tutti"}.
async fn cambia_rete<R, W>(app: &Arc<App>, req: &Richiesta, lettore: &mut BufReader<R>, w: &mut W) -> Risultato<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let errore = |m: &str| json!({"errore": m});
    let mut dati = match corpo_json(req, lettore).await {
        Ok(Value::Object(o)) => o,
        Ok(_) => return http::json(w, "400 Bad Request", &errore("richiesta non valida")).await,
        Err(m) => return http::json(w, "400 Bad Request", &errore(m)).await,
    };
    let mut cfg = match leggi_config(&app.p) {
        Value::Object(o) => o,
        _ => serde_json::Map::new(),
    };
    let cfgv = Value::Object(cfg.clone());
    let mut reti = rete::reti_salvate(&cfgv);
    let indice = |k: &str, dati: &serde_json::Map<String, Value>| dati.get(k).and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok())));
    for k in ["usa", "togli"] {
        if dati.contains_key(k) && !indice(k, &dati).is_some_and(|i| i >= 0 && (i as usize) < reti.len()) {
            return http::json(w, "400 Bad Request", &errore("rete non trovata")).await;
        }
    }
    let s = |k: &str| cfgv.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    if let Some(i) = indice("togli", &dati) {
        let via = reti.remove(i as usize);
        if via.get("authority").and_then(Value::as_str) == Some(&s("authority")) && via.get("directory_url").and_then(Value::as_str) == Some(&s("directory_url")) {
            dati = serde_json::Map::new();
            dati.insert("prova".into(), json!(true));
        }
    }
    let nome: String;
    if dati.contains_key("ponte") || dati.contains_key("ponte_togli") {
        if s("directory_url").is_empty() || s("authority").is_empty() {
            return http::json(w, "400 Bad Request", &errore("i ponti servono su una rete condivisa: prima importa il codice di rete")).await;
        }
        let prima = rete::ponti_di(&cfgv);
        let nuovi: Vec<String> = if let Some(p) = dati.get("ponte") {
            let t = p.as_str().map(str::to_string).unwrap_or_else(|| p.to_string());
            let riga = t.split_whitespace().last().unwrap_or("").to_string();
            if !rete::ponte_valido(&riga) {
                return http::json(w, "400 Bad Request", &errore("riga del ponte non valida (attesa: IP:PORTA,chiave,impronta,chiave-ponte)")).await;
            }
            let ind = riga.split(',').next().unwrap_or("").to_string();
            prima.into_iter().filter(|r| r.split(',').next() != Some(ind.as_str())).chain([riga]).collect()
        } else {
            let via = dati.get("ponte_togli").map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).unwrap_or_default();
            if via == "tutti" { vec![] } else { prima.into_iter().filter(|r| r.split(',').next() != Some(via.as_str())).collect() }
        };
        let mut tutti = cfg.get("ponti").and_then(Value::as_object).cloned().unwrap_or_default();
        tutti.insert(s("authority"), json!(nuovi));
        tutti.retain(|_, v| v.as_array().is_some_and(|a| !a.is_empty()));
        cfg.insert("ponti".into(), Value::Object(tutti));
        nome = s("nome_rete");
    } else if dati.get("prova").is_some_and(|v| v.as_bool().unwrap_or(!v.is_null())) {
        for k in ["directory_url", "authority", "nome_rete"].iter().chain(CHIAVI_RETE.iter()) {
            cfg.remove(*k);
        }
        nome = String::new();
    } else if let Some(i) = indice("usa", &dati) {
        let r = reti[i as usize].clone();
        applica_rete(&mut cfg, &r);
        nome = r.get("nome").and_then(Value::as_str).unwrap_or("").to_string();
    } else if dati.contains_key("togli") {
        nome = s("nome_rete");
    } else {
        let codice = dati.get("codice").and_then(Value::as_str).unwrap_or("");
        let r = match rete::leggi_codice(codice) {
            Ok(r) => r,
            Err(m) => return http::json(w, "400 Bad Request", &errore(&m)).await,
        };
        applica_rete(&mut cfg, &r);
        nome = r.get("nome").and_then(Value::as_str).unwrap_or("").to_string();
        reti.retain(|x| !(x.get("authority") == r.get("authority") && x.get("directory_url") == r.get("directory_url")));
        reti.push(r);
    }
    let da = reti.len().saturating_sub(20);
    cfg.insert("reti".into(), Value::Array(reti[da..].iter().cloned().map(Value::Object).collect()));
    let cfgv = Value::Object(cfg);
    scrivi_config(&app.p, &cfgv)?;
    log(&if nome.is_empty() { "rete: prova locale".to_string() } else { format!("rete: «{nome}» ({})", cfgv.get("directory_url").and_then(Value::as_str).unwrap_or("")) });
    let a = app.clone();
    std::thread::spawn(move || {
        servizi::ferma(&a);
        std::thread::sleep(std::time::Duration::from_millis(500));
        servizi::avvia(&a);
    });
    http::json(w, "200 OK", &json!({"ok": true, "nome": nome})).await
}

/// Le manopole avanzate del motore: valgono al prossimo avvio della rete.
async fn imposta_avanzate<R, W>(app: &Arc<App>, req: &Richiesta, lettore: &mut BufReader<R>, w: &mut W) -> Risultato<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let dati = match corpo_json(req, lettore).await {
        Ok(v) => v,
        Err(m) => return http::json(w, "400 Bad Request", &json!({"errore": m})).await,
    };
    let mut cfg = match leggi_config(&app.p) {
        Value::Object(o) => o,
        _ => serde_json::Map::new(),
    };
    let st = |k: &str| dati.get(k).and_then(Value::as_str).unwrap_or("");
    let k = dati.get("finestra_kb").and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())));
    if let Some(k) = k.filter(|k| (16..=4096).contains(k)) {
        cfg.insert("finestra_kb".into(), json!(k));
    }
    for (chiave, ammessi) in [("congestione", &["bbr", "cubic", "newreno"][..]), ("tcp", &["auto", "sempre", "mai"][..]),
                              ("riuso", &["si", "no"][..]), ("costante", &["no", "uso", "sempre"][..])] {
        if ammessi.contains(&st(chiave)) {
            cfg.insert(chiave.into(), json!(st(chiave)));
        }
    }
    if let Some(u) = dati.get("uscite").and_then(Value::as_i64).filter(|u| (1..=4).contains(u)) {
        cfg.insert("uscite".into(), json!(u));
    }
    scrivi_config(&app.p, &Value::Object(cfg))?;
    let riavvia = dati.get("riavvia").is_some_and(|v| v.as_bool().unwrap_or(false)) && app.servizi.lock().unwrap().stato == "pronto";
    if riavvia {
        servizi::avvia(app);
    }
    http::json(w, "200 OK", &json!({"ok": true, "riavviata": riavvia})).await
}
