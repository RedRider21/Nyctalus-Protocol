// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Trasloco dall'app con Termux all'app «con tutto»: Termux impacchetta (tar) i dati di Nyctalus
//! — `.nyctalus-chat` (identità, contatti, messaggi, allegati) e la configurazione della rete —
//! e li consegna QUI, sul telefono stesso (127.0.0.1, con il segreto dell'app). Si accettano solo
//! quei percorsi: niente nomi assoluti né `..`, niente collegamenti; file 600, cartelle 700.
//! Se qui c'è già un'identità non si sovrascrive nulla (a meno di `forza=1`). I dati in Termux
//! restano dove sono.

use std::path::{Component, Path, PathBuf};

use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader};

use super::{leggi_config, log, scrivi_config, servizi, App};

/// Al massimo 16 GB (gli allegati possono essere tanti).
const MAX_TRASLOCO: u64 = 16 * 1024 * 1024 * 1024;

fn ottale(b: &[u8]) -> u64 {
    let s: String = b.iter().take_while(|&&c| c != 0).map(|&c| c as char).collect();
    u64::from_str_radix(s.trim(), 8).unwrap_or(0)
}

fn testo(b: &[u8]) -> String {
    String::from_utf8_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(b.len())]).into_owned()
}

/// Il percorso è ammesso? (solo i dati di Nyctalus, nessuna uscita dalla cartella)
pub fn ammesso(nome: &str) -> Option<PathBuf> {
    let n = nome.trim_start_matches("./");
    let p = Path::new(n);
    if p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    let ok = n == ".nyctalus-chat" || n.starts_with(".nyctalus-chat/") || n == ".nyctalus-mobile/config.json";
    ok.then(|| p.to_path_buf())
}

pub fn identita_presente(app: &App) -> bool {
    app.p.dati_chat.join("identita").exists()
}

async fn leggi_esatti<R: AsyncRead + Unpin>(r: &mut R, b: &mut [u8], resto: &mut u64) -> std::io::Result<()> {
    if *resto < b.len() as u64 {
        return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "pacchetto troncato"));
    }
    r.read_exact(b).await?;
    *resto -= b.len() as u64;
    Ok(())
}

async fn salta<R: AsyncRead + Unpin>(r: &mut R, mut n: u64, resto: &mut u64) -> std::io::Result<()> {
    let mut buf = vec![0u8; 65536];
    while n > 0 {
        let k = n.min(65536) as usize;
        leggi_esatti(r, &mut buf[..k], resto).await?;
        n -= k as u64;
    }
    Ok(())
}

/// Estrae il tar (ustar, GNU nomi lunghi, pax) in `dest`, solo i percorsi ammessi.
/// → (file scritti, byte scritti).
pub async fn estrai<R: AsyncRead + Unpin>(r: &mut R, lunghezza: u64, dest: &Path) -> Result<(u64, u64), String> {
    let mut resto = lunghezza;
    let (mut file, mut byte) = (0u64, 0u64);
    let mut nome_lungo: Option<String> = None;
    let mut testa = [0u8; 512];
    loop {
        if resto < 512 {
            break;
        }
        leggi_esatti(r, &mut testa, &mut resto).await.map_err(|e| e.to_string())?;
        if testa.iter().all(|&b| b == 0) {
            break; // fine dell'archivio
        }
        let tipo = testa[156];
        let dim = ottale(&testa[124..136]);
        let blocchi = dim.div_ceil(512) * 512;
        let mut nome = nome_lungo.take().unwrap_or_else(|| {
            let base = testo(&testa[0..100]);
            let prefisso = if &testa[257..262] == b"ustar" { testo(&testa[345..500]) } else { String::new() };
            if prefisso.is_empty() { base } else { format!("{prefisso}/{base}") }
        });
        match tipo {
            b'L' | b'x' => {
                // nome lungo (GNU) o intestazione pax: il nome vale per la voce seguente
                if dim > 64 * 1024 {
                    return Err("intestazione troppo grande".into());
                }
                let mut b = vec![0u8; blocchi as usize];
                leggi_esatti(r, &mut b, &mut resto).await.map_err(|e| e.to_string())?;
                let b = &b[..dim as usize];
                nome_lungo = if tipo == b'L' {
                    Some(testo(b))
                } else {
                    String::from_utf8_lossy(b).lines().find_map(|riga| riga.split_once(" path=").map(|(_, p)| p.to_string()))
                };
                continue;
            }
            b'g' => {
                salta(r, blocchi, &mut resto).await.map_err(|e| e.to_string())?;
                continue;
            }
            _ => {}
        }
        if nome.ends_with('/') {
            nome.pop();
        }
        let Some(rel) = ammesso(&nome) else {
            salta(r, blocchi, &mut resto).await.map_err(|e| e.to_string())?;
            continue;
        };
        let p = dest.join(&rel);
        match tipo {
            b'5' => {
                tokio::fs::create_dir_all(&p).await.map_err(|e| e.to_string())?;
                imposta_modo(&p, 0o700);
                salta(r, blocchi, &mut resto).await.map_err(|e| e.to_string())?;
            }
            b'0' | 0 | b'7' => {
                if let Some(c) = p.parent() {
                    tokio::fs::create_dir_all(c).await.map_err(|e| e.to_string())?;
                }
                let mut f = tokio::fs::File::create(&p).await.map_err(|e| e.to_string())?;
                imposta_modo(&p, 0o600);
                let mut da_scrivere = dim;
                let mut buf = vec![0u8; 65536];
                while da_scrivere > 0 {
                    let k = da_scrivere.min(65536) as usize;
                    leggi_esatti(r, &mut buf[..k], &mut resto).await.map_err(|e| e.to_string())?;
                    f.write_all(&buf[..k]).await.map_err(|e| e.to_string())?;
                    da_scrivere -= k as u64;
                }
                f.flush().await.map_err(|e| e.to_string())?;
                salta(r, blocchi - dim, &mut resto).await.map_err(|e| e.to_string())?;
                file += 1;
                byte += dim;
            }
            _ => {
                // collegamenti, dispositivi, …: mai
                salta(r, blocchi, &mut resto).await.map_err(|e| e.to_string())?;
            }
        }
    }
    Ok((file, byte))
}

fn imposta_modo(p: &Path, modo: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(modo));
    }
    #[cfg(not(unix))]
    let _ = (p, modo);
}

/// POST /api/trasloco: riceve il pacchetto da Termux, lo sistema e riaccende i servizi.
pub async fn ricevi<R: AsyncRead + Unpin>(app: &std::sync::Arc<App>, lunghezza: i64, forza: bool, lettore: &mut BufReader<R>) -> (&'static str, Value) {
    if lunghezza <= 0 || lunghezza as u64 > MAX_TRASLOCO {
        return ("413 Payload Too Large", json!({"errore": "dimensione non valida"}));
    }
    if identita_presente(app) && !forza {
        return ("409 Conflict", json!({"errore": "qui c'è già un'identità Nyctalus: non sovrascrivo nulla"}));
    }
    // i servizi si fermano (la chat tiene aperti i suoi file), si estrae a parte, poi si sposta
    let accesi = matches!(app.servizi.lock().unwrap().stato.as_str(), "pronto" | "avvio");
    let a = app.clone();
    let _ = tokio::task::spawn_blocking(move || servizi::ferma(&a)).await;
    // dopo un rifiuto si torna com'era: i servizi si riaccendono solo se lo erano
    let riaccendi = |app: &std::sync::Arc<App>| if accesi { servizi::avvia(app) };
    let tappa = app.p.home.join(".nyctalus-trasloco");
    let _ = tokio::fs::remove_dir_all(&tappa).await;
    if let Err(e) = tokio::fs::create_dir_all(&tappa).await {
        return ("500 Internal Server Error", json!({"errore": e.to_string()}));
    }
    imposta_modo(&tappa, 0o700);
    let esito = estrai(lettore, lunghezza as u64, &tappa).await;
    let (file, byte) = match esito {
        Ok(x) if tappa.join(".nyctalus-chat").join("identita").exists() => x,
        Ok(_) => {
            let _ = tokio::fs::remove_dir_all(&tappa).await;
            riaccendi(app);
            return ("400 Bad Request", json!({"errore": "nel pacchetto non c'è un'identità Nyctalus"}));
        }
        Err(e) => {
            let _ = tokio::fs::remove_dir_all(&tappa).await;
            riaccendi(app);
            return ("400 Bad Request", json!({"errore": format!("pacchetto non valido: {e}")}));
        }
    };
    // al posto dei dati di qui (se si forza, quelli di prima restano da parte in .nyctalus-chat.prima)
    if app.p.dati_chat.exists() {
        let prima = app.p.home.join(".nyctalus-chat.prima");
        let _ = tokio::fs::remove_dir_all(&prima).await;
        let _ = tokio::fs::rename(&app.p.dati_chat, &prima).await;
    }
    if let Err(e) = tokio::fs::rename(tappa.join(".nyctalus-chat"), &app.p.dati_chat).await {
        return ("500 Internal Server Error", json!({"errore": e.to_string()}));
    }
    // la configurazione della rete (codice, reti salvate, ponti, impostazioni); le porte restano quelle di qui
    if let Ok(t) = tokio::fs::read_to_string(tappa.join(".nyctalus-mobile").join("config.json")).await {
        if let Ok(Value::Object(mut nuova)) = serde_json::from_str::<Value>(&t) {
            nuova.remove("porte");
            if let Some(p) = leggi_config(&app.p).get("porte") {
                nuova.insert("porte".into(), p.clone());
            }
            let _ = scrivi_config(&app.p, &Value::Object(nuova));
        }
    }
    let _ = tokio::fs::remove_dir_all(&tappa).await;
    let risultato = json!({"ok": true, "file": file, "byte": byte});
    let _ = super::scrivi_privato(&app.p.dati.join("trasloco.json"), super::http::json_come_python(&risultato).as_bytes());
    log(&format!("trasloco da Termux: {file} file, {byte} byte"));
    servizi::avvia(app);
    ("200 OK", risultato)
}

/// GET /api/trasloco: c'è già un'identità qui? Com'è andato l'ultimo trasloco?
pub fn stato(app: &App) -> Value {
    let ultimo = std::fs::read_to_string(app.p.dati.join("trasloco.json")).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    json!({"identita": identita_presente(app), "ultimo": ultimo.unwrap_or(Value::Null)})
}

#[cfg(test)]
mod prove {
    use super::ammesso;

    #[test]
    fn percorsi_ammessi() {
        assert!(ammesso(".nyctalus-chat/identita").is_some());
        assert!(ammesso("./.nyctalus-chat/allegati/x/foto.jpg").is_some());
        assert!(ammesso(".nyctalus-mobile/config.json").is_some());
        assert!(ammesso(".nyctalus-mobile/token").is_none());
        assert!(ammesso(".nyctalus-chat/../.bashrc").is_none());
        assert!(ammesso("/etc/passwd").is_none());
        assert!(ammesso(".bashrc").is_none());
        assert!(ammesso(".nyctalus-chatX/a").is_none());
    }
}
