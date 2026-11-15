// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! I servizi dell'app: motore della chat, rete (pubblica dal codice di rete, oppure di prova
//! locale) e servizio `.nyct`. Stesso ordine e stessi registri del Python (`Servizi._avvia`).
//! Ogni programma parte in un suo gruppo di processi; fermando si spengono i gruppi interi.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::{leggi_config, log, rete, App};

/// Il motore da lanciare: `NYCTALUS_BIN`, altrimenti questo stesso programma.
pub fn binario() -> Option<PathBuf> {
    if let Some(b) = std::env::var_os("NYCTALUS_BIN").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(b);
    }
    std::env::current_exe().ok()
}

pub fn porta_libera(porta: u16) -> bool {
    std::net::TcpStream::connect_timeout(&([127, 0, 0, 1], porta).into(), Duration::from_millis(300)).is_err()
}

/// I processi lanciati (per spegnerli tutti).
#[derive(Default)]
pub struct Processi(Mutex<Vec<Child>>);

impl Processi {
    fn lancia(&self, argomenti: &[String], registro: &Path, ambiente: &[(String, String)]) -> Result<u32, String> {
        let f = std::fs::OpenOptions::new().create(true).append(true).open(registro).map_err(|e| e.to_string())?;
        let mut c = Command::new(&argomenti[0]);
        c.args(&argomenti[1..]).stdout(Stdio::from(f.try_clone().map_err(|e| e.to_string())?)).stderr(Stdio::from(f)).stdin(Stdio::null());
        for (k, v) in ambiente {
            c.env(k, v);
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            c.process_group(0);
        }
        let figlio = c.spawn().map_err(|e| format!("{}: {e}", argomenti[0]))?;
        let pid = figlio.id();
        self.0.lock().unwrap().push(figlio);
        Ok(pid)
    }

    /// Spegne ogni programma con il suo gruppo (il processo e i suoi figli).
    pub fn termina(&self) {
        let mut v = self.0.lock().unwrap();
        for c in v.iter_mut().rev() {
            let pid = c.id();
            #[cfg(unix)]
            {
                // il gruppo intero (process_group(0): il gruppo ha il numero del processo)
                let gruppo = format!("-{pid}");
                let ok = Command::new("kill").args(["-TERM", "--", &gruppo]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
                if !ok {      // kill di toybox (Android) senza «--»
                    let _ = Command::new("kill").args(["-TERM", &gruppo]).stdout(Stdio::null()).stderr(Stdio::null()).status();
                }
            }
        }
        // si lascia il tempo di chiudersi bene (la chat svuota la vista in chiaro della cassaforte),
        // poi chi è ancora vivo si ferma per forza
        let fine = Instant::now() + Duration::from_secs(5);
        while Instant::now() < fine && v.iter_mut().any(|c| matches!(c.try_wait(), Ok(None))) {
            std::thread::sleep(Duration::from_millis(100));
        }
        for c in v.iter_mut() {
            if matches!(c.try_wait(), Ok(None)) {
                let _ = c.kill();
            }
            let _ = c.wait();
        }
        v.clear();
    }
}

fn attendi(registro: &Path, cerca: impl Fn(&str) -> Option<String>, secondi: u64) -> Option<String> {
    let fine = Instant::now() + Duration::from_secs(secondi);
    while Instant::now() < fine {
        if let Ok(t) = std::fs::read(registro) {
            if let Some(x) = cerca(&String::from_utf8_lossy(&t)) {
                return Some(x);
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    None
}

/// Primo indirizzo `.nyct` nel testo.
pub fn trova_nyct(t: &str) -> Option<String> {
    t.match_indices(".nyct").find_map(|(i, _)| {
        let inizio = i.checked_sub(56)?;
        let c = &t[inizio..i + 5];
        rete::indirizzo_nyct(c).then(|| c.to_string())
    })
}

/// La riga di un nodo di prova: `127.0.0.1:PORTA,hex,hex`.
fn riga_nodo(t: &str, porta: u16) -> Option<String> {
    let inizio = format!("127.0.0.1:{porta},");
    let i = t.find(&inizio)?;
    let resto: String = t[i..].chars().take_while(|c| !c.is_whitespace()).collect();
    let parti: Vec<&str> = resto.split(',').collect();
    (parti.len() >= 3 && parti[1..3].iter().all(|p| !p.is_empty() && p.bytes().all(|c| c.is_ascii_hexdigit()))).then(|| parti[..3].join(","))
}

fn campo_esadecimale(t: &str, etichetta: &str) -> Option<String> {
    let i = t.find(etichetta)? + etichetta.len();
    let v: String = t[i..].trim_start().chars().take_while(char::is_ascii_hexdigit).collect();
    (!v.is_empty()).then_some(v)
}

/// «porta segreto» del motore acceso (lo legge nyctalus-cli), privato 600.
fn scrivi_collegamento(app: &App, segreto: &str) {
    let _ = std::fs::create_dir_all(&app.p.dati_chat);
    if let Err(e) = super::scrivi_privato(&app.p.dati_chat.join("controllo"), format!("{} {segreto}\n", app.porte.controllo).as_bytes()) {
        log(&format!("collegamento per nyctalus-cli non scritto: {e}"));
    }
}

fn togli_collegamento(app: &App) {
    let _ = std::fs::remove_file(app.p.dati_chat.join("controllo"));
}

pub fn avvia(app: &Arc<App>) {
    {
        let mut s = app.servizi.lock().unwrap();
        if s.stato == "avvio" || s.stato == "pronto" {
            return;
        }
        s.stato = "avvio".into();
        s.dettaglio = "motore".into();
    }
    let app = app.clone();
    std::thread::spawn(move || {
        if let Err(e) = avvia_davvero(&app) {
            {
                let mut s = app.servizi.lock().unwrap();
                s.stato = "errore".into();
                s.dettaglio = e.clone();
            }
            log(&format!("errore avvio servizi: {e}"));
            app.processi.termina();
        }
    });
}

pub fn ferma(app: &App) {
    app.processi.termina();
    togli_collegamento(app);
    let mut s = app.servizi.lock().unwrap();
    s.stato = "fermo".into();
    s.dettaglio.clear();
    s.indirizzo.clear();
    s.modalita.clear();
}

fn dettaglio(app: &App, d: &str) {
    app.servizi.lock().unwrap().dettaglio = d.into();
}

fn avvia_davvero(app: &Arc<App>) -> Result<(), String> {
    let b = binario().ok_or("binario 'nyctalus' non trovato (vedi l'installatore)")?.to_string_lossy().to_string();
    let lavoro = app.p.dati.join("servizi");
    let _ = std::fs::remove_dir_all(&lavoro);
    std::fs::create_dir_all(&lavoro).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&app.p.dati_chat).map_err(|e| e.to_string())?;
    let cfg = leggi_config(&app.p);
    let s = |k: &str| cfg.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let pubblica = !s("directory_url").is_empty() && !s("authority").is_empty();
    // rete ospitata nella casa in cui siamo: nodi ed elenco con l'indirizzo interno
    let (regola, url_elenco) = if pubblica { rete::vicini(&cfg) } else { (String::new(), String::new()) };
    let mut base: Vec<(String, String)> = Vec::new();
    if !regola.is_empty() {
        base.push(("NYCTALUS_VICINI".into(), regola.clone()));
        log(&format!("rete ospitata in questa casa: uso l'indirizzo interno ({regola})"));
    }
    let porte = &app.porte;
    let porta_socks = if pubblica { porte.socks } else { porte.socks_locale };
    let socks = format!("127.0.0.1:{porta_socks}");
    let occupate: Vec<String> = [porte.controllo, porte.backend, porta_socks].iter().filter(|p| !porta_libera(**p)).map(u16::to_string).collect();
    if !occupate.is_empty() {
        return Err(format!("porte già usate da un altro programma: {} (cambiale in {})", occupate.join(", "), app.p.config.display()));
    }
    let manopole = rete::manopole_motore(&cfg);
    let ponti = rete::ponti_di(&cfg).join(";");
    let segreto_motore = app.segreto_motore.clone();

    // 1) il motore della chat (la web-app vede subito il Contact ID)
    let mut amb = base.clone();
    amb.push(("NYCTALUS_CHAT_TOKEN".into(), segreto_motore.clone()));
    amb.extend(manopole.iter().cloned());
    let mut argomenti = vec![b.clone(), "chat".into(), "--data".into(), app.p.dati_chat.to_string_lossy().into(),
        "--backend".into(), format!("127.0.0.1:{}", porte.backend), "--control".into(), format!("127.0.0.1:{}", porte.controllo),
        "--socks".into(), socks.clone()];
    if pubblica {
        for k in ["rubrica", "casella"] {
            if rete::indirizzo_nyct(&s(k)) {
                argomenti.extend([format!("--{k}"), s(k)]);
            }
        }
    }
    let reg_chat = lavoro.join("chat.log");
    app.processi.lancia(&argomenti, &reg_chat, &amb)?;
    let primo = attendi(&reg_chat, |t| trova_nyct(t).or_else(|| t.contains("aspetto la passphrase").then(|| "chiusa".to_string())), 10)
        .ok_or("il motore della chat non è partito")?;
    scrivi_collegamento(app, &segreto_motore);
    if primo.ends_with(".nyct") {
        let mut st = app.servizi.lock().unwrap();
        st.indirizzo = primo;
        st.chiusa = false;
    } else {
        log("dati cifrati: la chat aspetta la passphrase");
        app.servizi.lock().unwrap().chiusa = true;
        let (app2, reg) = (app.clone(), reg_chat.clone());
        std::thread::spawn(move || {
            if let Some(a) = attendi(&reg, trova_nyct, 24 * 3600) {
                app2.servizi.lock().unwrap().indirizzo = a;
            }
        });
    }

    // 2) la rete
    let dirf = lavoro.join("dir.txt");
    let autorita;
    let mut amb_rete = base.clone();
    amb_rete.push(("NYCTALUS_PONTI".into(), if pubblica { ponti.clone() } else { String::new() }));
    amb_rete.extend(manopole.iter().cloned());
    if pubblica {
        {
            let mut st = app.servizi.lock().unwrap();
            st.modalita = "pubblica".into();
            st.dettaglio = "elenco dei nodi".into();
        }
        let url = if url_elenco.is_empty() { s("directory_url") } else { url_elenco };
        let corpo = rete::scarica(&url).map_err(|e| {
            let nome = s("nome_rete");
            format!("la rete «{}» non risponde: l'elenco dei nodi non si scarica da {url} ({e}). È spenta, oppure da qui non \
                     si raggiunge: riaccendila, oppure scegli un'altra rete o la rete di prova", if nome.is_empty() { "condivisa".into() } else { nome })
        })?;
        std::fs::write(&dirf, corpo).map_err(|e| e.to_string())?;
        autorita = s("authority");
        let mut a = vec![b.clone(), "start".into(), "--directory".into(), dirf.to_string_lossy().into(), "--authority".into(), autorita.clone(),
            "--socks".into(), socks.clone(), "--control".into(), format!("127.0.0.1:{}", porte.monitor)];
        a.extend(rete::opz_prot(&cfg, true));
        app.processi.lancia(&a, &lavoro.join("demone.log"), &amb_rete)?;
    } else {
        {
            let mut st = app.servizi.lock().unwrap();
            st.modalita = "locale".into();
            st.dettaglio = "rete di prova".into();
        }
        let out = Command::new(&b).arg("authority").output().map_err(|e| e.to_string())?;
        let testo = String::from_utf8_lossy(&out.stdout).to_string();
        let sec = campo_esadecimale(&testo, "secret:").ok_or("autorità di prova non creata")?;
        autorita = campo_esadecimale(&testo, "public:").ok_or("autorità di prova non creata")?;
        let mut righe = Vec::new();
        for &porta in &porte.nodi {
            if !porta_libera(porta) {
                return Err(format!("porta {porta} già usata da un altro programma"));
            }
            let lf = lavoro.join(format!("n{porta}.log"));
            app.processi.lancia(&[b.clone(), "node".into(), "--listen".into(), format!("127.0.0.1:{porta}")], &lf, &base)?;
            righe.push(attendi(&lf, |t| riga_nodo(t, porta), 15).ok_or(format!("nodo di prova {porta} non partito"))?);
        }
        let out = Command::new(&b).args(["directory", "--authority-secret", &sec, "--entries", &righe.join(";")]).output().map_err(|e| e.to_string())?;
        std::fs::write(&dirf, &out.stdout).map_err(|e| e.to_string())?;
        app.processi.lancia(&[b.clone(), "circuit".into(), "--directory".into(), dirf.to_string_lossy().into(), "--authority".into(), autorita.clone(),
            "--socks".into(), socks.clone(), "--hops".into(), "3".into(), "--control".into(), format!("127.0.0.1:{}", porte.monitor)],
            &lavoro.join("demone.log"), &base)?;
    }

    // 3) il nostro servizio .nyct (entrata), quando il SOCKS risponde
    dettaglio(app, "servizio .nyct");
    for _ in 0..60 {
        if !porta_libera(porta_socks) {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let mut amb_sito = if pubblica { amb_rete.clone() } else { base.clone() };
    amb_sito.push(("NYCTALUS_CHAT_TOKEN".into(), segreto_motore));
    let mut a = vec![b.clone(), "site".into(), "--backend".into(), format!("127.0.0.1:{}", porte.backend), "--identity-chat".into(),
        format!("127.0.0.1:{}", porte.controllo), "--directory".into(), dirf.to_string_lossy().into(), "--authority".into(), autorita,
        "--hops".into(), "3".into()];
    if pubblica {
        a.extend(rete::opz_prot(&cfg, false));
    }
    app.processi.lancia(&a, &lavoro.join("site.log"), &amb_sito)?;
    let (modalita, indirizzo) = {
        let mut st = app.servizi.lock().unwrap();
        st.stato = "pronto".into();
        st.dettaglio.clear();
        (st.modalita.clone(), st.indirizzo.clone())
    };
    log(&format!("servizi pronti ({modalita}) — {indirizzo}"));
    Ok(())
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn letture_dai_registri() {
        let a = "a".repeat(56);
        assert_eq!(trova_nyct(&format!("indirizzo: {a}.nyct pronto")), Some(format!("{a}.nyct")));
        assert_eq!(trova_nyct("x.nyct"), None);
        assert_eq!(riga_nodo("  directory entry:   127.0.0.1:19440,ab12,cd34\n", 19440), Some("127.0.0.1:19440,ab12,cd34".into()));
        assert_eq!(riga_nodo("127.0.0.1:19441,ab", 19441), None);
        assert_eq!(campo_esadecimale("secret:  00ff\npublic: 11", "secret:"), Some("00ff".into()));
    }
}
