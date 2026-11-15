// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! I file dell'app: dove si può leggere (mai fuori dalle cartelle consentite), nomi sicuri,
//! caricamenti, galleria delle foto recenti, «sfoglia» in sola lettura, archivio dei ricevuti,
//! miniature (quella che le fotocamere mettono già nel JPEG, EXIF) e i file della cassaforte,
//! che si leggono da una copia in chiaro fatta dal motore (APRI). Come il Python.
//!
//! Telefono senza Termux: le cartelle condivise le indica l'app Android con `NYCTALUS_GALLERIA`
//! e `NYCTALUS_SFOGLIA` (es. /storage/emulated/0/DCIM); aprire e condividere un file lo fa
//! l'app Android (`NYCTALUS_ANDROID=1`: la risposta porta il percorso da passarle).

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::App;

pub const MAX_CARICAMENTO: u64 = 200 * 1024 * 1024;
const ESTENSIONI_MEDIA: [&str; 11] = ["jpg", "jpeg", "png", "webp", "gif", "heic", "mp4", "mov", "webm", "3gp", "mkv"];
const GALLERIA_DURATA: Duration = Duration::from_secs(45);
const GALLERIA_MAX: usize = 300;
const SIGILLO: &[u8] = b"NYCS1";

fn lista_ambiente(nome: &str) -> Vec<PathBuf> {
    std::env::var_os(nome).map(|v| std::env::split_paths(&v).filter(|p| !p.as_os_str().is_empty()).collect()).unwrap_or_default()
}

pub fn hex(p: &Path) -> String {
    p.to_string_lossy().as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn da_hex_percorso(h: &str) -> Option<PathBuf> {
    let s = super::ws::da_hex(h);
    (!s.is_empty()).then(|| PathBuf::from(s))
}

fn secondi(t: std::io::Result<SystemTime>) -> u64 {
    t.ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn cartelle_galleria(app: &App) -> Vec<PathBuf> {
    let mut v = lista_ambiente("NYCTALUS_GALLERIA");
    if std::env::var_os("NYCTALUS_GALLERIA_SOLO").is_none() {
        let s = app.p.home.join("storage");
        v.extend([s.join("dcim"), s.join("pictures"), s.join("movies")]);
    }
    v
}

/// Il percorso (risolto: niente ../ né collegamenti fuori) sta dentro una delle radici?
pub fn dentro(percorso: &Path, radici: &[PathBuf]) -> Option<PathBuf> {
    let reale = percorso.canonicalize().ok()?;
    radici.iter().filter_map(|r| r.canonicalize().ok()).any(|r| reale.starts_with(&r)).then_some(reale)
}

fn radici_lettura(app: &App) -> Vec<PathBuf> {
    let mut v = vec![app.p.dati_chat.join("allegati"), app.p.invio.clone(), app.p.cache.clone()];
    v.extend(cartelle_galleria(app).into_iter().filter(|g| g.exists()));
    v
}

/// Il file è dentro una delle cartelle consentite?
pub fn leggibile(app: &App, p: &Path) -> Option<PathBuf> {
    dentro(p, &radici_lettura(app)).filter(|f| f.is_file())
}

pub fn nome_sicuro(nome: &str) -> String {
    let base = nome.rsplit(['/', '\\']).next().unwrap_or("");
    let base: String = base.chars().map(|c| if (c as u32) < 32 || ":*?\"<>|".contains(c) { '_' } else { c }).collect();
    let base = base.trim().trim_start_matches('.');
    let b: String = base.chars().take(120).collect();
    if b.is_empty() { "file".into() } else { b }
}

pub fn percorso_libero(cartella: &Path, nome: &str) -> PathBuf {
    let mut p = cartella.join(nome);
    let (stem, ext) = match nome.rfind('.').filter(|&i| i > 0) {
        Some(i) => (&nome[..i], &nome[i..]),
        None => (nome, ""),
    };
    let mut n = 2;
    while p.exists() {
        p = cartella.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    p
}

/// Riceve un file dal telefono (corpo grezzo, nome in ?nome=) in `invio/`.
pub async fn carica<R: tokio::io::AsyncRead + Unpin>(app: &App, nome: &str, lunghezza: i64, lettore: &mut BufReader<R>) -> (&'static str, Value) {
    if lunghezza < 0 || lunghezza as u64 > MAX_CARICAMENTO {
        return ("413 Payload Too Large", json!({"errore": "dimensione non valida"}));
    }
    if tokio::fs::create_dir_all(&app.p.invio).await.is_err() {
        return ("500 Internal Server Error", json!({"errore": "cartella non scrivibile"}));
    }
    let dest = percorso_libero(&app.p.invio, &nome_sicuro(nome));
    let Ok(mut f) = tokio::fs::File::create(&dest).await else {
        return ("500 Internal Server Error", json!({"errore": "file non scrivibile"}));
    };
    let mut resto = lunghezza as u64;
    while resto > 0 {
        let pezzo = match lettore.fill_buf().await {
            Ok(b) if !b.is_empty() => b,
            _ => break,
        };
        let n = pezzo.len().min(resto as usize);
        if f.write_all(&pezzo[..n]).await.is_err() {
            break;
        }
        lettore.consume(n);
        resto -= n as u64;
    }
    let _ = f.flush().await;
    if resto > 0 {
        let _ = tokio::fs::remove_file(&dest).await;
        return ("400 Bad Request", json!({"errore": "caricamento interrotto"}));
    }
    ("200 OK", json!({"percorso": dest.to_string_lossy()}))
}

/// Foto e video più recenti (dal più nuovo), senza cartelle nascoste, al massimo 3 livelli.
fn elenco_galleria(app: &App, quanti: usize) -> Vec<(u64, PathBuf, u64)> {
    let mut voci = Vec::new();
    let mut visti = std::collections::HashSet::new();
    for radice in cartelle_galleria(app) {
        if !radice.exists() {
            continue;
        }
        let mut pila = vec![(radice.clone(), 0usize)];
        while let Some((cart, prof)) = pila.pop() {
            let Ok(voci_dir) = std::fs::read_dir(&cart) else { continue };
            for e in voci_dir.flatten() {
                let nome = e.file_name().to_string_lossy().to_string();
                if nome.starts_with('.') {
                    continue;
                }
                let pf = e.path();
                let Ok(st) = std::fs::metadata(&pf) else { continue };
                if st.is_dir() {
                    if prof < 3 {
                        pila.push((pf, prof + 1));
                    }
                    continue;
                }
                let ext = pf.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
                if !ESTENSIONI_MEDIA.contains(&ext.as_str()) {
                    continue;
                }
                if visti.insert(pf.canonicalize().unwrap_or_else(|_| pf.clone())) {
                    voci.push((secondi(st.modified()), pf, st.len()));
                }
            }
        }
    }
    voci.sort_by(|a, b| b.cmp(a));
    voci.truncate(quanti);
    voci
}

pub struct CacheGalleria(Mutex<Option<(Instant, Vec<(u64, PathBuf, u64)>)>>);

impl Default for CacheGalleria {
    fn default() -> Self {
        CacheGalleria(Mutex::new(None))
    }
}

pub fn galleria(app: &App, da: usize, quante: usize) -> Value {
    let tutte = {
        let mut c = app.galleria.0.lock().unwrap();
        match c.as_ref() {
            Some((t, v)) if t.elapsed() < GALLERIA_DURATA => v.clone(),
            _ => {
                let v = elenco_galleria(app, GALLERIA_MAX);
                *c = Some((Instant::now(), v.clone()));
                v
            }
        }
    };
    let voci: Vec<Value> = tutte.iter().skip(da).take(quante).map(|(ts, pf, dim)| json!({
        "p": hex(pf), "nome": pf.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        "dim": dim, "ts": ts, "mime": super::http::tipo_mime(pf)})).collect();
    json!({"disponibile": cartelle_galleria(app).iter().any(|g| g.exists()), "voci": voci, "altre": da + quante < tutte.len()})
}

/// Dove si possono SCEGLIERE i file da mandare (sola lettura).
fn cartelle_condivise(app: &App) -> Vec<(String, PathBuf)> {
    let nome_di = |p: &PathBuf| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.to_string_lossy().to_string());
    let extra = lista_ambiente("NYCTALUS_SFOGLIA");
    let mut voci: Vec<(String, PathBuf)> = extra.iter().map(|p| (nome_di(p), p.clone())).collect();
    if std::env::var_os("NYCTALUS_SFOGLIA_SOLO").is_some() {
        return voci;
    }
    let condivisa = app.p.home.join("storage").join("shared");
    if condivisa.exists() {
        for (nome, sotto) in [("Download", "Download"), ("Documenti", "Documents"), ("Foto e video", "DCIM"), ("Immagini", "Pictures"), ("Film", "Movies"), ("Musica", "Music")] {
            if condivisa.join(sotto).is_dir() {
                voci.push((nome.into(), condivisa.join(sotto)));
            }
        }
        voci.push(("Tutta la memoria".into(), condivisa));
    }
    voci
}

pub fn sfoglia(app: &App, d: &str) -> Value {
    let radici = cartelle_condivise(app);
    if d.is_empty() {
        let voci: Vec<Value> = radici.iter().filter(|(_, p)| p.exists()).map(|(n, p)| json!({"nome": n, "p": hex(p), "dir": true})).collect();
        return json!({"su": null, "voci": voci});
    }
    let solo: Vec<PathBuf> = radici.iter().map(|(_, p)| p.clone()).collect();
    let Some(cart) = dentro(Path::new(d), &solo).filter(|c| c.is_dir()) else {
        return json!({"errore": "cartella non consentita"});
    };
    let Ok(lettura) = std::fs::read_dir(&cart) else { return json!({"errore": "cartella non leggibile"}) };
    let mut elenco: Vec<(bool, String, PathBuf)> = lettura.flatten().map(|e| {
        let p = e.path();
        (!p.is_dir(), e.file_name().to_string_lossy().to_lowercase(), p)
    }).collect();
    elenco.sort();
    let mut voci = Vec::new();
    for (_, _, p) in elenco.into_iter().take(3000) {
        let nome = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if nome.starts_with('.') {
            continue;
        }
        let Ok(st) = std::fs::metadata(&p) else { continue };
        let mut v = json!({"nome": nome, "p": hex(&p), "dir": st.is_dir()});
        if !st.is_dir() {
            v["dim"] = json!(st.len());
            v["ts"] = json!(secondi(st.modified()));
            v["mime"] = json!(super::http::tipo_mime(&p));
        }
        voci.push(v);
    }
    let alla_radice = solo.iter().filter_map(|r| r.canonicalize().ok()).any(|r| r == cart);
    json!({"su": if alla_radice { Value::Null } else { json!(hex(cart.parent().unwrap_or(&cart))) },
           "titolo": cart.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), "voci": voci})
}

/// I file ricevuti (cartella privata), dai più grandi; spazio usato e libero.
pub fn archivio(app: &App) -> Value {
    let base = app.p.dati_chat.join("allegati");
    let mut voci = Vec::new();
    let mut totale = 0u64;
    let mut pila = vec![base.clone()];
    while let Some(c) = pila.pop() {
        let Ok(l) = std::fs::read_dir(&c) else { continue };
        for e in l.flatten() {
            let p = e.path();
            let nome = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                pila.push(p);
                continue;
            }
            if nome.starts_with('.') || nome.ends_with(".part") {
                continue;
            }
            let Ok(st) = std::fs::metadata(&p) else { continue };
            totale += st.len();
            voci.push(json!({"p": hex(&p), "nome": nome, "dim": st.len(), "ts": secondi(st.modified()),
                "da": p.parent().and_then(|x| x.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                "mime": super::http::tipo_mime(&p)}));
        }
    }
    voci.sort_by_key(|v| std::cmp::Reverse(v["dim"].as_u64().unwrap_or(0)));
    json!({"voci": voci, "totale": totale, "libero": spazio_libero(if app.p.dati_chat.exists() { &app.p.dati_chat } else { &app.p.home })})
}

/// Spazio libero (con `df`, senza codice non sicuro); 0 se non si sa.
fn spazio_libero(p: &Path) -> u64 {
    std::process::Command::new("df").args(["-k", "-P"]).arg(p).output().ok()
        .and_then(|o| String::from_utf8_lossy(&o.stdout).lines().nth(1).and_then(|r| r.split_whitespace().nth(3)?.parse::<u64>().ok()))
        .map(|k| k * 1024).unwrap_or(0)
}

/// Un file dell'archivio privato (solo lì si può eliminare o condividere).
pub fn nell_archivio(app: &App, p_hex: &str) -> Option<PathBuf> {
    let p = da_hex_percorso(p_hex)?;
    dentro(&p, &[app.p.dati_chat.join("allegati")]).filter(|f| f.is_file() && !f.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
}

pub fn e_ricevuto(app: &App, f: &Path) -> bool {
    match (f.canonicalize(), app.p.dati_chat.join("allegati").canonicalize()) {
        (Ok(a), Ok(b)) => a.starts_with(b),
        _ => false,
    }
}

fn sigillato(f: &Path) -> bool {
    use std::io::Read;
    let mut b = [0u8; 5];
    std::fs::File::open(f).and_then(|mut h| h.read_exact(&mut b)).is_ok() && b == SIGILLO
}

/// Da dove LEGGERE un file: lui stesso se è in chiaro; se la cassaforte lo tiene cifrato
/// (comincia con NYCS1) si chiede al motore «APRI», che ne mette una copia decifrata nella
/// vista temporanea e ne dà il percorso. None se non si può aprire.
pub async fn in_chiaro(app: &App, f: Option<PathBuf>) -> Option<PathBuf> {
    let f = f?;
    if !e_ricevuto(app, &f) || !sigillato(&f) {
        return Some(f);
    }
    {
        let mut r = app.miniature_ripulite.lock().unwrap();
        if !*r {
            // la cassaforte è accesa: le miniature fatte PRIMA (in chiaro sul disco) si buttano
            *r = true;
            let _ = std::fs::remove_dir_all(app.p.cache.join("ricevuti"));
        }
    }
    let m = tokio::time::timeout(Duration::from_secs(5), tokio::net::TcpStream::connect(("127.0.0.1", app.porte.controllo))).await.ok()?.ok()?;
    let (lett, mut scr) = m.into_split();
    scr.write_all(format!("AUTH {}\nAPRI {}\n", app.segreto_motore, hex(&f)).as_bytes()).await.ok()?;
    let mut lett = BufReader::with_capacity(64 * 1024, lett);
    let fine = Instant::now() + Duration::from_secs(180);
    while Instant::now() < fine {
        let mut riga = String::new();
        let n = tokio::time::timeout(fine - Instant::now(), lett.read_line(&mut riga)).await.ok()?.ok()?;
        if n == 0 {
            return None;
        }
        let p: Vec<&str> = riga.split_whitespace().collect();
        match p.first().copied() {
            Some("APERTO") if p.len() >= 3 => return da_hex_percorso(p[2]).filter(|c| c.is_file()),
            Some("NONAPERTO" | "CHIUSA" | "ERR") => return None,
            _ => {}
        }
    }
    None
}

/// La miniatura che le fotocamere mettono già dentro il JPEG (EXIF, ~160 px), con
/// l'orientamento della foto, così il browser la gira come l'originale.
pub fn miniatura_exif(percorso: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut testa = Vec::new();
    std::fs::File::open(percorso).ok()?.take(256 * 1024).read_to_end(&mut testa).ok()?;
    if testa.get(..2)? != b"\xff\xd8" {
        return None;
    }
    let mut k = 2usize;
    while k + 4 <= testa.len() && testa[k] == 0xff {
        let tipo = testa[k + 1];
        let lung = u16::from_be_bytes([testa[k + 2], testa[k + 3]]) as usize;
        if tipo == 0xe1 && testa.get(k + 4..k + 10)? == b"Exif\x00\x00" {
            let t = testa.get(k + 10..(k + 2 + lung).min(testa.len()))?;
            let le = t.get(..2)? == b"II";
            let u16_ = |pos: usize| -> Option<u16> { let b = t.get(pos..pos + 2)?; Some(if le { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) }) };
            let u32_ = |pos: usize| -> Option<u32> { let b: [u8; 4] = t.get(pos..pos + 4)?.try_into().ok()?; Some(if le { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) }) };
            let ifd = |pos: usize| -> Option<(std::collections::HashMap<u16, usize>, u32)> {
                let n = u16_(pos)? as usize;
                let mut voci = std::collections::HashMap::new();
                for e in 0..n {
                    let base = pos + 2 + 12 * e;
                    voci.insert(u16_(base)?, base + 8); // posizione del valore (4 byte)
                }
                Some((voci, u32_(pos + 2 + 12 * n)?))
            };
            let (ifd0, prossimo) = ifd(u32_(4)? as usize)?;
            let orient = ifd0.get(&0x0112).and_then(|&v| u16_(v)).unwrap_or(1);
            if prossimo == 0 {
                return None;
            }
            let (ifd1, _) = ifd(prossimo as usize)?;
            let ini = u32_(*ifd1.get(&0x0201)?)? as usize;
            let dim = u32_(*ifd1.get(&0x0202)?)? as usize;
            let mini = t.get(ini..ini + dim)?;
            if mini.get(..2)? != b"\xff\xd8" {
                return None;
            }
            let mut out = mini.to_vec();
            if (2..=8).contains(&orient) {
                let mut carico = b"Exif\x00\x00MM\x00\x2a\x00\x00\x00\x08\x00\x01".to_vec();
                carico.extend_from_slice(&0x0112u16.to_be_bytes());
                carico.extend_from_slice(&3u16.to_be_bytes());
                carico.extend_from_slice(&1u32.to_be_bytes());
                carico.extend_from_slice(&orient.to_be_bytes());
                carico.extend_from_slice(&0u16.to_be_bytes());
                carico.extend_from_slice(b"\x00\x00\x00\x00");
                let mut segmento = b"\xff\xe1".to_vec();
                segmento.extend_from_slice(&((carico.len() + 2) as u16).to_be_bytes());
                segmento.extend_from_slice(&carico);
                out.splice(2..2, segmento);
            }
            return Some(out);
        }
        if tipo == 0xda {
            return None; // inizio dell'immagine: niente EXIF
        }
        k += 2 + lung;
    }
    None
}

/// Miniatura JPEG di una foto o di un video, in cache: EXIF → ffmpeg (se c'è) → la foto stessa
/// se è già piccola. None: la web-app mostra un'icona invece della foto intera.
pub fn miniatura(app: &App, percorso: &Path, cache: Option<PathBuf>) -> Option<PathBuf> {
    let cache = cache.unwrap_or_else(|| app.p.cache.clone());
    let st = std::fs::metadata(percorso).ok()?;
    let chiave = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY,
        format!("{}|{}|{}", percorso.display(), st.len(), secondi(st.modified())).as_bytes());
    let chiave: String = chiave.as_ref().iter().map(|b| format!("{b:02x}")).collect();
    std::fs::create_dir_all(&cache).ok()?;
    let dest = cache.join(format!("{chiave}.jpg"));
    if dest.exists() {
        return Some(dest);
    }
    let video = super::http::tipo_mime(percorso).starts_with("video/");
    if !video {
        if let Some(m) = miniatura_exif(percorso) {
            std::fs::write(&dest, m).ok()?;
            return Some(dest);
        }
    }
    if std::process::Command::new("ffmpeg").arg("-version").output().is_ok() {
        let mut c = std::process::Command::new("ffmpeg");
        c.args(["-v", "error", "-y"]);
        if video {
            c.args(["-ss", "0.5"]);
        }
        c.arg("-i").arg(percorso).args(["-frames:v", "1", "-vf", "scale=360:-2"]).arg(&dest);
        let _ = c.output();
        if dest.exists() {
            return Some(dest);
        }
    }
    (!video && st.len() <= 400 * 1024).then(|| percorso.to_path_buf())
}

/// Apre o condivide un file con le app del sistema. Telefono senza Termux: lo fa l'app Android
/// (la risposta porta il percorso); con Termux termux-open; sul PC xdg-open.
pub fn apri_o_condividi(f: &Path, condividi: bool) -> Value {
    if std::env::var("NYCTALUS_ANDROID").is_ok_and(|v| v == "1") {
        return json!({"ok": true, "android": if condividi { "condividi" } else { "apri" }, "percorso": f.to_string_lossy()});
    }
    let lancia = |cmd: &str, args: Vec<std::ffi::OsString>| {
        std::process::Command::new(cmd).args(args).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().is_ok()
    };
    let ok = if condividi {
        lancia("termux-open", vec!["--send".into(), "--chooser".into(), f.into()])
            || lancia("xdg-open", vec![f.parent().unwrap_or(f).into()])
    } else {
        lancia("termux-open", vec![f.into()]) || lancia("xdg-open", vec![f.into()])
    };
    json!({"ok": ok})
}

#[cfg(test)]
mod prove {
    use super::*;

    /// Un JPEG con l'EXIF (big-endian): IFD0 con l'orientamento, IFD1 con la miniatura dentro.
    fn jpeg_con_miniatura(orient: u16, mini: &[u8]) -> Vec<u8> {
        let mut t = b"MM\x00\x2a\x00\x00\x00\x08".to_vec();
        // IFD0 a 8: una voce (0x0112 SHORT 1 = orient), poi prossimo IFD a 26
        t.extend_from_slice(&1u16.to_be_bytes());
        t.extend_from_slice(&[0x01, 0x12, 0x00, 0x03, 0, 0, 0, 1]);
        t.extend_from_slice(&orient.to_be_bytes());
        t.extend_from_slice(&[0, 0]);
        t.extend_from_slice(&26u32.to_be_bytes());
        // IFD1 a 26: 0x0201 (inizio) e 0x0202 (lunghezza), poi 0; la miniatura dopo (a 26+2+24+4 = 56)
        t.extend_from_slice(&2u16.to_be_bytes());
        t.extend_from_slice(&[0x02, 0x01, 0x00, 0x04, 0, 0, 0, 1]);
        t.extend_from_slice(&56u32.to_be_bytes());
        t.extend_from_slice(&[0x02, 0x02, 0x00, 0x04, 0, 0, 0, 1]);
        t.extend_from_slice(&(mini.len() as u32).to_be_bytes());
        t.extend_from_slice(&0u32.to_be_bytes());
        assert_eq!(t.len(), 56);
        t.extend_from_slice(mini);
        let mut app1 = b"Exif\x00\x00".to_vec();
        app1.extend_from_slice(&t);
        let mut j = b"\xff\xd8\xff\xe1".to_vec();
        j.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
        j.extend_from_slice(&app1);
        j.extend_from_slice(b"\xff\xda\x00\x02\xff\xd9");
        j
    }

    #[test]
    fn miniatura_dentro_la_foto() {
        let d = std::env::temp_dir().join(format!("nyct-mini-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let mini = b"\xff\xd8MINIATURA\xff\xd9";
        let f = d.join("foto.jpg");
        std::fs::write(&f, jpeg_con_miniatura(6, mini)).unwrap();
        let m = miniatura_exif(&f).expect("miniatura EXIF");
        assert_eq!(&m[..2], b"\xff\xd8");
        assert!(m.windows(9).any(|w| w == b"MINIATURA"));
        // orientamento 6 riportato in un segmento APP1 della miniatura (il browser la gira)
        assert!(m.windows(6).any(|w| w == b"Exif\x00\x00"));
        assert!(m.windows(2).any(|w| w == [0x00, 0x06]));
        // senza orientamento particolare: la miniatura così com'è
        std::fs::write(&f, jpeg_con_miniatura(1, mini)).unwrap();
        assert_eq!(miniatura_exif(&f).unwrap(), mini.to_vec());
        // file rovinato o non JPEG: nessun errore, nessuna miniatura
        std::fs::write(&f, b"\xff\xd8\xff\xe1\x00").unwrap();
        assert!(miniatura_exif(&f).is_none());
        std::fs::write(&f, b"non sono una foto").unwrap();
        assert!(miniatura_exif(&f).is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn nomi_sicuri() {
        assert_eq!(nome_sicuro("../../.bashrc"), "bashrc");
        assert_eq!(nome_sicuro("a:b*c?.txt"), "a_b_c_.txt");
        assert_eq!(nome_sicuro(""), "file");
        assert_eq!(nome_sicuro("C:\\x\\foto.jpg"), "foto.jpg");
    }
}
