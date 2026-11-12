// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Trasferimenti di file grandi fra due contatti (il programma «File»).
//!
//! **Controllo** (dentro la sessione cifrata e autenticata della chat, testo):
//! `XO <id> <ts> <dim> <pezzo> <mime> <nome hex> <radice>` proposta · `XA <id>` accettata ·
//! `XN <id>` rifiutata · `XP <id>` pausa · `XR <id>` riprendi · `XC <id>` annullata ·
//! `XF <id>` arrivato tutto (chi riceve → chi invia).
//!
//! **Dati** (canali paralleli, ognuno un collegamento `.nyct` = un circuito diverso, con la
//! STESSA stretta della chat: saluto `T<indirizzo>` + Noise NK). Chi riceve apre i canali verso
//! chi invia e chiede (binario, sigillato): `X<id>` (di quale trasferimento) · `M` (le impronte
//! dei pezzi) · `G<n u64>` (il pezzo n). Risposte: `H<impronte…>` … `h` · `B<n u64><off u32><dati>`
//! … `Z<n u64>` · `E<messaggio>`.
//!
//! Il file è diviso in pezzi da [`PEZZO`]; ogni pezzo ha la sua impronta BLAKE3 e la **radice**
//! (BLAKE3 di tutte le impronte) viaggia nella proposta: un pezzo sbagliato o manomesso si
//! scarta e si richiede. Chi riceve tiene la mappa dei pezzi verificati su disco: dopo una
//! caduta (rete, spegnimento) riparte chiedendo solo quelli che mancano.
//!
//! **Interfaccia:** comandi `XSEND`, `XOK`, `XNO`, `XPAUSA`, `XRIPRENDI`, `XANNULLA`, `XLIST`,
//! `XCONF`; eventi `XNEW <id> <in|out> <addr> <dim> <mime> <nome hex> <stato> <fatti>`,
//! `XSTATO <id> <stato> [dettaglio hex]`, `XPROG <id> <fatti> <totale> <byte/s>`,
//! `XFINE <id> <percorso hex>`. Stati: `prepara`, `attesa`, `chiede`, `corre`, `pausa`,
//! `fatto`, `errore`, `rifiutato`, `annullato`.

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nyctalus_core::stretta::{self, Mittente, SegretiSessione};
use nyctalus_core::indirizzo;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

use super::{
    cartella_contatto, con_suffisso, da_hex, id_valido, leggi_frame, mime_sicuro, nome_sicuro, nuovo_id, ora,
    percorso_libero, scrivi_frame, socks5_connetti, Stato,
};
use crate::incontro::Record;
use crate::{in_esadecimale, Risultato};

/// Dimensione di un pezzo (l'unità che si verifica e si richiede).
pub(super) const PEZZO: u64 = 256 * 1024;
/// Dati di un pezzo per frame (sta comodo nel frame da 64 KB).
const BLOCCO_DATI: usize = 32 * 1024;
/// Impronte per frame nella distinta (32 KB).
const IMPRONTE_PER_FRAME: usize = 1024;
/// Al massimo 1 Mi pezzi (256 GB): limite anti-abuso sulla distinta.
const MAX_PEZZI: u64 = 1 << 20;
/// Pezzi chiesti in anticipo su ogni canale (la latenza di più salti si copre così).
const IN_VOLO_PER_CANALE: usize = 4;
/// Tentativi su un pezzo che arriva sbagliato prima di dichiarare il trasferimento rotto.
const TENTATIVI_PEZZO: u32 = 4;
/// Riprovi di un canale caduto prima di arrendersi (poi si riparte quando il contatto torna).
const RIPROVI_CANALE: u32 = 3;
/// Ogni quanto avvisare l'interfaccia sull'avanzamento.
const OGNI_AVVISO: Duration = Duration::from_millis(500);
/// Trasferimenti attivi contemporanei verso/da uno stesso contatto.
const MAX_PER_CONTATTO: usize = 16;

/// Solo per le PROVE: attesa (ms) dopo ogni pezzo, per poter mettere in pausa o spegnere a
/// metà anche sulla rete di prova dentro il PC (troppo veloce). Mai impostata nell'uso normale.
fn rallenta_prova() -> Option<Duration> {
    std::env::var("NYCTALUS_PROVA_RALLENTA_MS").ok()?.parse().ok().map(Duration::from_millis)
}

const SOGLIA_PRED: u64 = 100 * 1024 * 1024;
const CANALI_PRED: usize = 3;
const CANALI_MAX: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Verso {
    In,
    Out,
}

/// Un trasferimento, come sta su disco (`trasferimenti/<id>.txt`).
#[derive(Clone, Debug)]
pub(super) struct Trasf {
    pub id: String,
    pub verso: Verso,
    pub peer: String,
    pub nome: String,
    pub mime: String,
    pub dim: u64,
    pub npezzi: u64,
    pub radice: String,
    pub ts: u64,
    pub stato: String,
    pub accettato: bool,
    /// out: il file da mandare; in: dove è finito (vuoto finché non è completo)
    pub percorso: PathBuf,
    /// in: il file in costruzione (`.part`)
    pub parziale: PathBuf,
    pub byte_fatti: u64,
}

impl Trasf {
    fn chiuso(&self) -> bool {
        matches!(self.stato.as_str(), "fatto" | "rifiutato" | "annullato")
    }
}

/// Impostazioni del programma File (`trasferimenti/impostazioni`).
#[derive(Clone, Copy, Debug)]
pub(super) struct Impostazioni {
    pub soglia: u64,
    pub canali: usize,
    /// i file dei trasferimenti compaiono anche nella conversazione della chat (scelta dell'utente)
    pub in_chat: bool,
}

/// Tutto lo stato dei trasferimenti del motore.
pub(super) struct Registro {
    pub elenco: Mutex<HashMap<String, Trasf>>,
    /// trasferimenti in arrivo che stanno scaricando adesso (uno solo per id)
    pub attivi: Mutex<std::collections::HashSet<String>>,
    pub imp: Mutex<Impostazioni>,
}

impl Registro {
    pub fn carica(dati: &std::path::Path) -> Self {
        let dir = dati.join("trasferimenti");
        let _ = std::fs::create_dir_all(&dir);
        let mut elenco = HashMap::new();
        if let Ok(voci) = std::fs::read_dir(&dir) {
            for v in voci.flatten() {
                let p = v.path();
                if p.extension().and_then(|e| e.to_str()) != Some("txt") {
                    continue;
                }
                if let Some(mut t) = std::fs::read_to_string(&p).ok().and_then(|s| leggi_trasf(&s)) {
                    // quello che era in corsa riparte quando il contatto torna in linea
                    if matches!(t.stato.as_str(), "corre" | "prepara") {
                        t.stato = "attesa".into();
                    }
                    // i byte arrivati davvero sono quelli dei pezzi verificati nella mappa
                    if t.verso == Verso::In && !t.chiuso() {
                        let m = std::fs::read(dir.join(format!("{}.mappa", t.id))).unwrap_or_default();
                        t.byte_fatti = (0..t.npezzi).filter(|n| m.get(*n as usize) == Some(&1)).map(|n| PEZZO.min(t.dim - n * PEZZO)).sum();
                    }
                    elenco.insert(t.id.clone(), t);
                }
            }
        }
        let mut imp = Impostazioni { soglia: SOGLIA_PRED, canali: CANALI_PRED, in_chat: true };
        if let Ok(s) = std::fs::read_to_string(dir.join("impostazioni")) {
            for riga in s.lines() {
                match riga.split_once(' ') {
                    Some(("soglia", v)) => imp.soglia = v.trim().parse().unwrap_or(imp.soglia),
                    Some(("canali", v)) => imp.canali = v.trim().parse::<usize>().unwrap_or(imp.canali).clamp(1, CANALI_MAX),
                    Some(("inchat", v)) => imp.in_chat = v.trim() != "0",
                    _ => {}
                }
            }
        }
        Self { elenco: Mutex::new(elenco), attivi: Mutex::new(Default::default()), imp: Mutex::new(imp) }
    }
}

fn scrivi_trasf(t: &Trasf) -> String {
    format!(
        "id {}\nverso {}\npeer {}\nnome {}\nmime {}\ndim {}\nnpezzi {}\nradice {}\nts {}\nstato {}\naccettato {}\npercorso {}\nparziale {}\nfatti {}\n",
        t.id,
        if t.verso == Verso::In { "in" } else { "out" },
        t.peer,
        in_esadecimale(t.nome.as_bytes()),
        t.mime,
        t.dim,
        t.npezzi,
        t.radice,
        t.ts,
        t.stato,
        if t.accettato { 1 } else { 0 },
        in_esadecimale(t.percorso.to_string_lossy().as_bytes()),
        in_esadecimale(t.parziale.to_string_lossy().as_bytes()),
        t.byte_fatti
    )
}

fn leggi_trasf(s: &str) -> Option<Trasf> {
    let m: HashMap<&str, &str> = s.lines().filter_map(|l| l.split_once(' ')).collect();
    let hex = |k: &str| String::from_utf8_lossy(&da_hex(m.get(k).copied().unwrap_or("")).unwrap_or_default()).to_string();
    let t = Trasf {
        id: m.get("id")?.to_string(),
        verso: if *m.get("verso")? == "in" { Verso::In } else { Verso::Out },
        peer: m.get("peer")?.to_string(),
        nome: hex("nome"),
        mime: m.get("mime")?.to_string(),
        dim: m.get("dim")?.parse().ok()?,
        npezzi: m.get("npezzi")?.parse().ok()?,
        radice: m.get("radice")?.to_string(),
        ts: m.get("ts")?.parse().ok()?,
        stato: m.get("stato")?.to_string(),
        accettato: m.get("accettato") == Some(&"1"),
        percorso: PathBuf::from(hex("percorso")),
        parziale: PathBuf::from(hex("parziale")),
        byte_fatti: m.get("fatti").and_then(|v| v.parse().ok()).unwrap_or(0),
    };
    (id_valido(&t.id) && t.npezzi <= MAX_PEZZI).then_some(t)
}

fn npezzi_di(dim: u64) -> u64 {
    dim.div_ceil(PEZZO).max(1)
}

/// La radice: BLAKE3 di tutte le impronte dei pezzi, in fila.
fn radice_di(impronte: &[[u8; 32]]) -> String {
    let mut h = blake3::Hasher::new();
    for i in impronte {
        h.update(i);
    }
    h.finalize().to_hex().to_string()
}

/// Impronte dei pezzi di un file (lettura a blocchi: niente file intero in memoria).
fn calcola_impronte(percorso: &std::path::Path, dim: u64) -> Risultato<Vec<[u8; 32]>> {
    // file ricevuto e cifrato sul disco (cassaforte): le impronte sono dei pezzi in chiaro
    if crate::cassaforte::e_sigillato(percorso) {
        let s = crate::cassaforte::Sigillo::da_file(percorso)?;
        if s.lunghezza != dim || s.pezzo != PEZZO {
            return Err("il file è cambiato mentre lo preparavo".into());
        }
        let mut f = std::fs::File::open(percorso)?;
        return (0..s.npezzi()).map(|n| Ok(*blake3::hash(&s.leggi_pezzo(&mut f, n)?).as_bytes())).collect();
    }
    let mut f = std::fs::File::open(percorso)?;
    let mut out = Vec::with_capacity(npezzi_di(dim) as usize);
    let mut buf = vec![0u8; PEZZO as usize];
    let mut letti = 0u64;
    loop {
        let mut n = 0;
        while n < buf.len() {
            let k = f.read(&mut buf[n..])?;
            if k == 0 {
                break;
            }
            n += k;
        }
        if n == 0 && !out.is_empty() {
            break;
        }
        out.push(*blake3::hash(&buf[..n]).as_bytes());
        letti += n as u64;
        if n < buf.len() {
            break;
        }
    }
    if letti != dim {
        return Err("il file è cambiato mentre lo preparavo".into());
    }
    Ok(out)
}

impl Stato {
    fn x_dir(&self) -> PathBuf {
        self.dati.join("trasferimenti")
    }

    fn x_salva(&self, t: &Trasf) {
        let _ = std::fs::write(self.x_dir().join(format!("{}.txt", t.id)), scrivi_trasf(t));
    }

    fn x_impronte_salva(&self, id: &str, impronte: &[[u8; 32]]) {
        let _ = std::fs::write(self.x_dir().join(format!("{id}.impronte")), impronte.concat());
    }

    fn x_impronte(&self, id: &str) -> Option<Vec<[u8; 32]>> {
        let b = std::fs::read(self.x_dir().join(format!("{id}.impronte"))).ok()?;
        (b.len() % 32 == 0).then(|| b.chunks(32).map(|c| c.try_into().unwrap()).collect())
    }

    /// Mappa dei pezzi verificati (in): un byte per pezzo, 1 = arrivato e controllato.
    fn x_mappa(&self, id: &str, npezzi: u64) -> Vec<bool> {
        let b = std::fs::read(self.x_dir().join(format!("{id}.mappa"))).unwrap_or_default();
        (0..npezzi as usize).map(|i| b.get(i) == Some(&1)).collect()
    }

    fn x_mappa_segna(&self, id: &str, npezzi: u64, n: u64) {
        let p = self.x_dir().join(format!("{id}.mappa"));
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).open(&p) {
            if f.metadata().map(|m| m.len()).unwrap_or(0) < npezzi {
                let _ = f.set_len(npezzi);
            }
            if f.seek(SeekFrom::Start(n)).is_ok() {
                let _ = f.write_all(&[1]);
            }
        }
    }

    /// `XNEW <id> <in|out> <addr> <dim> <mime> <nome hex> <stato> <fatti> <percorso hex|->`.
    /// Un file arrivato e poi cancellato dalla cartella risulta `eliminato` (lo stato vero su
    /// disco resta `fatto`: è solo come lo si mostra).
    fn x_evento_nuovo(&self, t: &Trasf) -> String {
        let stato = if t.verso == Verso::In && t.stato == "fatto" && !t.percorso.as_os_str().is_empty() && !t.percorso.exists() {
            "eliminato"
        } else {
            t.stato.as_str()
        };
        let percorso = if t.percorso.as_os_str().is_empty() { "-".to_string() } else { in_esadecimale(t.percorso.to_string_lossy().as_bytes()) };
        format!(
            "XNEW {} {} {} {} {} {} {} {} {}",
            t.id,
            if t.verso == Verso::In { "in" } else { "out" },
            t.peer,
            t.dim,
            t.mime,
            in_esadecimale(t.nome.as_bytes()),
            stato,
            t.byte_fatti,
            percorso
        )
    }

    /// `XVIA <id>`: toglie dall'ELENCO un trasferimento concluso (non il file arrivato). Con un
    /// trasferimento in errore si butta anche il pezzo già scaricato.
    pub(super) fn x_togli(&self, id: &str) -> Risultato<()> {
        let t = self.x_prendi(id).ok_or("trasferimento sconosciuto")?;
        if !(t.chiuso() || t.stato == "errore") {
            return Err("è ancora in corso: prima annullalo".into());
        }
        if t.verso == Verso::In && t.stato == "errore" {
            let _ = std::fs::remove_file(&t.parziale);
        }
        self.trasf.elenco.lock().unwrap().remove(id);
        for est in ["txt", "impronte", "mappa"] {
            let _ = std::fs::remove_file(self.x_dir().join(format!("{id}.{est}")));
        }
        self.notifica(format!("XVIA {id}"));
        Ok(())
    }

    /// `XPULISCI`: toglie dall'elenco tutti i trasferimenti conclusi. Restituisce quanti.
    pub(super) fn x_pulisci(&self) -> usize {
        let chiusi: Vec<String> = self.trasf.elenco.lock().unwrap().values().filter(|t| t.chiuso()).map(|t| t.id.clone()).collect();
        chiusi.iter().filter(|id| self.x_togli(id).is_ok()).count()
    }

    /// Cambia lo stato (su disco e alle interfacce).
    fn x_stato(&self, id: &str, stato: &str, dettaglio: &str) {
        let copia = {
            let mut g = self.trasf.elenco.lock().unwrap();
            let Some(t) = g.get_mut(id) else { return };
            t.stato = stato.to_string();
            t.clone()
        };
        self.x_salva(&copia);
        if dettaglio.is_empty() {
            self.notifica(format!("XSTATO {id} {stato}"));
        } else {
            self.notifica(format!("XSTATO {id} {stato} {}", in_esadecimale(dettaglio.as_bytes())));
        }
    }

    fn x_prendi(&self, id: &str) -> Option<Trasf> {
        self.trasf.elenco.lock().unwrap().get(id).cloned()
    }

    /// Manda un segnale di controllo sulla sessione col contatto (aprendola se serve).
    async fn x_segnale(self: &Arc<Self>, peer: &str, testo: String) -> Risultato<()> {
        let s = self.sessione(peer).await?;
        s.send(testo.into_bytes()).await.map_err(|_| "sessione chiusa".into())
    }

    // --- invio ---------------------------------------------------------------------------
    /// `XSEND`: prepara la distinta (in sottofondo) e propone il file al contatto.
    pub(super) async fn x_invia(self: &Arc<Self>, addr: &str, percorso: PathBuf, mime: &str) -> Risultato<String> {
        let peer = con_suffisso(addr);
        indirizzo::in_chiave(&peer).map_err(|_| "indirizzo .nyct non valido")?;
        // un file ricevuto e cifrato sul disco si manda dalla sua copia in chiaro (stesso nome)
        let nome_originale = percorso.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let percorso = crate::cassaforte::in_chiaro(&percorso).map_err(|_| "file non trovato")?;
        let meta = tokio::fs::metadata(&percorso).await.map_err(|_| "file non trovato")?;
        if !meta.is_file() {
            return Err("non è un file".into());
        }
        let dim = meta.len();
        if npezzi_di(dim) > MAX_PEZZI {
            return Err("file troppo grande (massimo 256 GB)".into());
        }
        let attivi = self.trasf.elenco.lock().unwrap().values().filter(|t| t.peer == peer && !t.chiuso()).count();
        if attivi >= MAX_PER_CONTATTO {
            return Err("troppi trasferimenti aperti con questo contatto".into());
        }
        let nome = nome_sicuro(&nome_originale);
        let t = Trasf {
            id: nuovo_id(),
            verso: Verso::Out,
            peer,
            nome,
            mime: mime_sicuro(mime),
            dim,
            npezzi: npezzi_di(dim),
            radice: String::new(),
            ts: ora(),
            stato: "prepara".into(),
            accettato: false,
            percorso: std::fs::canonicalize(&percorso).unwrap_or(percorso),
            parziale: PathBuf::new(),
            byte_fatti: 0,
        };
        let id = t.id.clone();
        self.trasf.elenco.lock().unwrap().insert(id.clone(), t.clone());
        self.x_salva(&t);
        self.notifica(self.x_evento_nuovo(&t));
        let st = self.clone();
        let idc = id.clone();
        tokio::spawn(async move {
            let (p, d) = (t.percorso.clone(), t.dim);
            match tokio::task::spawn_blocking(move || calcola_impronte(&p, d)).await {
                Ok(Ok(impronte)) => {
                    st.x_impronte_salva(&idc, &impronte);
                    if let Some(t) = st.trasf.elenco.lock().unwrap().get_mut(&idc) {
                        t.radice = radice_di(&impronte);
                    }
                    st.x_stato(&idc, "attesa", "");
                    st.x_proponi(&idc).await;
                }
                Ok(Err(e)) => st.x_stato(&idc, "errore", &e.to_string()),
                Err(_) => st.x_stato(&idc, "errore", "preparazione interrotta"),
            }
        });
        Ok(id)
    }

    /// Propone (o ripropone) un file al contatto: se non è in linea, si riproverà quando torna.
    async fn x_proponi(self: &Arc<Self>, id: &str) {
        let Some(t) = self.x_prendi(id) else { return };
        if t.verso != Verso::Out || t.chiuso() || t.radice.is_empty() {
            return;
        }
        let msg = format!(
            "XO {} {} {} {} {} {} {}",
            t.id,
            t.ts,
            t.dim,
            PEZZO,
            t.mime,
            in_esadecimale(t.nome.as_bytes()),
            t.radice
        );
        if self.x_segnale(&t.peer, msg).await.is_err() {
            self.x_stato(id, "attesa", "il contatto non è in linea: parte appena si collega");
        }
    }

    // --- segnali dalla sessione ------------------------------------------------------------
    /// I segnali `X?` arrivati sulla sessione col contatto `peer` (autenticato).
    pub(super) fn x_ricevi(self: &Arc<Self>, peer: &str, chiaro: &[u8]) {
        let Ok(testo) = std::str::from_utf8(chiaro) else { return };
        let mut p = testo.split(' ');
        let tipo = p.next().unwrap_or("");
        let id = p.next().unwrap_or("").to_string();
        if !id_valido(&id) {
            return;
        }
        // un segnale vale solo per un trasferimento CON quel contatto
        let nostro = self.x_prendi(&id).filter(|t| t.peer == peer);
        match tipo {
            "XO" => {
                let campi: Vec<&str> = p.collect();
                let [ts, dim, pezzo, mime, nome_hex, radice] = campi.as_slice() else { return };
                self.x_proposta(peer, &id, ts, dim, pezzo, mime, nome_hex, radice, nostro);
            }
            "XA" => {
                if let Some(t) = nostro.filter(|t| t.verso == Verso::Out && !t.chiuso()) {
                    if let Some(x) = self.trasf.elenco.lock().unwrap().get_mut(&t.id) {
                        x.accettato = true;
                    }
                    self.x_stato(&id, "corre", "");
                }
            }
            "XN" => {
                if nostro.is_some_and(|t| t.verso == Verso::Out && !t.chiuso()) {
                    self.x_stato(&id, "rifiutato", "");
                }
            }
            "XP" => {
                if nostro.is_some_and(|t| !t.chiuso()) {
                    self.x_stato(&id, "pausa", "messo in pausa dal contatto");
                }
            }
            "XR" => {
                if let Some(t) = nostro.filter(|t| !t.chiuso() && t.accettato) {
                    self.x_stato(&id, "corre", "");
                    if t.verso == Verso::In {
                        tokio::spawn(self.clone().x_scarica(id));
                    }
                }
            }
            "XC" => {
                if let Some(t) = nostro.filter(|t| !t.chiuso()) {
                    self.x_stato(&id, "annullato", "annullato dal contatto");
                    if t.verso == Verso::In {
                        let _ = std::fs::remove_file(&t.parziale);
                    }
                }
            }
            "XF" => {
                // «XF <id> [ts]»: ts = l'ora del file in chat scelta da chi riceve (ora d'arrivo);
                // si accetta solo se plausibile (dopo la proposta, non nel futuro)
                let ts_chat = p
                    .next()
                    .and_then(|x| x.parse::<u64>().ok())
                    .filter(|x| nostro.as_ref().is_some_and(|t| *x >= t.ts) && *x <= ora() + 300);
                if let Some(t) = nostro.filter(|t| t.verso == Verso::Out && !t.chiuso()) {
                    let ts_chat = ts_chat.unwrap_or(t.ts);
                    if let Some(x) = self.trasf.elenco.lock().unwrap().get_mut(&t.id) {
                        x.byte_fatti = x.dim;
                    }
                    self.x_stato(&id, "fatto", "");
                    // nella conversazione compare il file, come un allegato (se l'utente lo vuole)
                    let nome_hex = in_esadecimale(t.nome.as_bytes());
                    let percorso_hex = in_esadecimale(t.percorso.to_string_lossy().as_bytes());
                    if self.trasf.imp.lock().unwrap().in_chat {
                        self.registra_riga(peer, &format!("fout {ts_chat} {} {} {} {nome_hex} {percorso_hex}", t.id, t.dim, t.mime));
                        self.notifica(format!("FILE {peer} out {ts_chat} {} {} {} {nome_hex} {percorso_hex}", t.id, t.dim, t.mime));
                        self.segna(peer, ts_chat, "2");
                    }
                    self.notifica(format!("XFINE {} {percorso_hex}", t.id));
                }
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn x_proposta(
        self: &Arc<Self>,
        peer: &str,
        id: &str,
        ts: &str,
        dim: &str,
        pezzo: &str,
        mime: &str,
        nome_hex: &str,
        radice: &str,
        nostro: Option<Trasf>,
    ) {
        // proposta ripetuta (il contatto è tornato in linea): si riprende da dove eravamo
        if let Some(t) = nostro {
            if t.verso == Verso::In && t.accettato && !t.chiuso() && t.stato != "pausa" {
                if t.stato != "corre" {
                    self.x_stato(&t.id, "corre", "");
                }
                let st = self.clone();
                let (p, i) = (peer.to_string(), id.to_string());
                tokio::spawn(async move {
                    let _ = st.x_segnale(&p, format!("XA {i}")).await;
                    st.x_scarica(i).await;
                });
            }
            return;
        }
        let (Ok(ts), Ok(dim), Ok(pezzo)) = (ts.parse::<u64>(), dim.parse::<u64>(), pezzo.parse::<u64>()) else { return };
        if pezzo != PEZZO || npezzi_di(dim) > MAX_PEZZI || radice.len() != 64 || !radice.bytes().all(|b| b.is_ascii_hexdigit()) {
            return;
        }
        let st = self.clone();
        let peer = peer.to_string();
        // solo dai CONTATTI: dagli sconosciuti si rifiuta senza chiedere
        let contatto = self.contatti.lock().unwrap().iter().any(|(c, _)| *c == peer);
        let aperti = self.trasf.elenco.lock().unwrap().values().filter(|t| t.peer == peer && !t.chiuso()).count();
        if !contatto || aperti >= MAX_PER_CONTATTO {
            let i = id.to_string();
            tokio::spawn(async move {
                let _ = st.x_segnale(&peer, format!("XN {i}")).await;
            });
            return;
        }
        let nome = nome_sicuro(&String::from_utf8_lossy(&da_hex(nome_hex).unwrap_or_default()));
        let cartella = self.dati.join("allegati").join(cartella_contatto(&peer));
        let _ = std::fs::create_dir_all(&cartella);
        let parziale = cartella.join(format!(".{id}.part"));
        let soglia = self.trasf.imp.lock().unwrap().soglia;
        let automatico = dim <= soglia;
        let t = Trasf {
            id: id.to_string(),
            verso: Verso::In,
            peer: peer.clone(),
            nome,
            mime: mime_sicuro(mime),
            dim,
            npezzi: npezzi_di(dim),
            radice: radice.to_lowercase(),
            ts,
            stato: if automatico { "corre".into() } else { "chiede".into() },
            accettato: automatico,
            percorso: PathBuf::new(),
            parziale,
            byte_fatti: 0,
        };
        self.trasf.elenco.lock().unwrap().insert(t.id.clone(), t.clone());
        self.x_salva(&t);
        self.notifica(self.x_evento_nuovo(&t));
        if automatico {
            let i = t.id.clone();
            tokio::spawn(async move {
                let _ = st.x_segnale(&peer, format!("XA {i}")).await;
                st.x_scarica(i).await;
            });
        }
    }

    /// Il contatto è tornato in linea: si riprende quello che era rimasto in sospeso con lui.
    pub(super) fn x_ricollega(self: &Arc<Self>, peer: &str) {
        let da_fare: Vec<Trasf> = self
            .trasf
            .elenco
            .lock()
            .unwrap()
            .values()
            .filter(|t| t.peer == peer && t.stato == "attesa")
            .cloned()
            .collect();
        for t in da_fare {
            let st = self.clone();
            tokio::spawn(async move {
                match t.verso {
                    Verso::Out => st.x_proponi(&t.id).await,
                    Verso::In if t.accettato => {
                        st.x_stato(&t.id, "corre", "");
                        let _ = st.x_segnale(&t.peer, format!("XR {}", t.id)).await;
                        st.x_scarica(t.id.clone()).await;
                    }
                    Verso::In => {}
                }
            });
        }
    }

    // --- comandi dell'interfaccia ------------------------------------------------------------
    pub(super) async fn x_comando(self: &Arc<Self>, cmd: &str, id: &str) -> Risultato<()> {
        let t = self.x_prendi(id).ok_or("trasferimento sconosciuto")?;
        match cmd {
            "XOK" => {
                if t.verso != Verso::In || t.stato != "chiede" {
                    return Err("non c'è niente da accettare".into());
                }
                if let Some(x) = self.trasf.elenco.lock().unwrap().get_mut(id) {
                    x.accettato = true;
                }
                self.x_stato(id, "corre", "");
                let _ = self.x_segnale(&t.peer, format!("XA {id}")).await;
                tokio::spawn(self.clone().x_scarica(id.to_string()));
            }
            "XNO" => {
                if t.verso != Verso::In || t.stato != "chiede" {
                    return Err("non c'è niente da rifiutare".into());
                }
                self.x_stato(id, "rifiutato", "");
                let _ = self.x_segnale(&t.peer, format!("XN {id}")).await;
            }
            "XPAUSA" => {
                if t.chiuso() || t.stato == "pausa" {
                    return Err("non è in corso".into());
                }
                self.x_stato(id, "pausa", "");
                let _ = self.x_segnale(&t.peer, format!("XP {id}")).await;
            }
            "XRIPRENDI" => {
                if t.stato != "pausa" && t.stato != "errore" && t.stato != "attesa" {
                    return Err("non è in pausa".into());
                }
                if !t.accettato {
                    // non ancora accettato: si ripropone (chi invia) o si torna a chiedere
                    let nuovo = if t.verso == Verso::Out { "attesa" } else { "chiede" };
                    self.x_stato(id, nuovo, "");
                    if t.verso == Verso::Out {
                        self.x_proponi(id).await;
                    }
                    return Ok(());
                }
                self.x_stato(id, "corre", "");
                let _ = self.x_segnale(&t.peer, format!("XR {id}")).await;
                if t.verso == Verso::In {
                    tokio::spawn(self.clone().x_scarica(id.to_string()));
                }
            }
            "XANNULLA" => {
                if t.chiuso() {
                    return Err("già concluso".into());
                }
                self.x_stato(id, "annullato", "");
                if t.verso == Verso::In {
                    let _ = std::fs::remove_file(&t.parziale);
                }
                let _ = self.x_segnale(&t.peer, format!("XC {id}")).await;
            }
            _ => return Err("comando sconosciuto".into()),
        }
        Ok(())
    }

    /// `XLIST`: tutti i trasferimenti (i più recenti per ultimi).
    pub(super) fn x_elenco(&self) -> Vec<String> {
        let mut v: Vec<Trasf> = self.trasf.elenco.lock().unwrap().values().cloned().collect();
        v.sort_by_key(|t| t.ts);
        v.iter().map(|t| self.x_evento_nuovo(t)).collect()
    }

    /// `XCONF [soglia N] [canali N]`: legge (e cambia) le impostazioni.
    pub(super) fn x_conf(&self, args: &str) -> String {
        let mut imp = self.trasf.imp.lock().unwrap();
        let mut p = args.split_whitespace();
        while let (Some(k), Some(v)) = (p.next(), p.next()) {
            match k {
                "soglia" => imp.soglia = v.parse().unwrap_or(imp.soglia),
                "canali" => imp.canali = v.parse::<usize>().unwrap_or(imp.canali).clamp(1, CANALI_MAX),
                "inchat" => imp.in_chat = v != "0",
                _ => {}
            }
        }
        let _ = std::fs::write(
            self.x_dir().join("impostazioni"),
            format!("soglia {}\ncanali {}\ninchat {}\n", imp.soglia, imp.canali, u8::from(imp.in_chat)),
        );
        format!("XCONF soglia {} canali {} inchat {}", imp.soglia, imp.canali, u8::from(imp.in_chat))
    }

    // --- canali dati ---------------------------------------------------------------------------
    /// Chi riceve apre un canale verso chi invia: stessa stretta della chat, saluto `T`.
    async fn x_apri_canale(&self, peer: &str, id: &str) -> Risultato<Canale> {
        let mut flusso = socks5_connetti(self.socks, peer, 1).await?;
        let mut saluto = vec![b'T'];
        saluto.extend_from_slice(self.indirizzo.as_bytes());
        scrivi_frame(&mut flusso, &saluto).await?;
        let msg1 = leggi_frame(&mut flusso).await?.ok_or("nessuna sfida dal contatto")?;
        let (msg2, segreti) = stretta::rispondi(&self.identita, &msg1)?;
        scrivi_frame(&mut flusso, &msg2).await?;
        let mut c = Canale::nuovo(flusso, &segreti, false);
        let mut x = vec![b'X'];
        x.extend_from_slice(id.as_bytes());
        c.manda(&x).await?;
        Ok(c)
    }

    /// Lato di chi invia: un canale dati in arrivo (già autenticato: `peer` è provato).
    pub(super) async fn x_servi(self: Arc<Self>, flusso: TcpStream, peer: String, segreti: SegretiSessione) -> Risultato<()> {
        let mut c = Canale::nuovo(flusso, &segreti, true);
        let primo = c.ricevi().await?.ok_or("canale vuoto")?;
        let id = std::str::from_utf8(primo.strip_prefix(b"X").ok_or("canale senza trasferimento")?)?.to_string();
        let t = self.x_prendi(&id).filter(|t| t.peer == peer && t.verso == Verso::Out).ok_or("trasferimento sconosciuto")?;
        let mut file = std::fs::File::open(&t.percorso).map_err(|_| "il file da mandare non c'è più")?;
        if file.metadata()?.len() != t.dim {
            c.manda(b"Eil file da mandare e' cambiato").await?;
            self.x_stato(&id, "errore", "il file è cambiato dopo l'invio della proposta");
            return Ok(());
        }
        loop {
            let Some(rich) = c.ricevi().await? else { return Ok(()) };
            // si serve solo un trasferimento accettato e in corso
            let ok = self.x_prendi(&id).is_some_and(|t| t.accettato && t.stato == "corre");
            if !ok {
                c.manda(b"Efermo").await?;
                return Ok(());
            }
            match rich.first() {
                Some(b'M') => {
                    let impronte = self.x_impronte(&id).ok_or("distinta mancante")?;
                    for blocco in impronte.chunks(IMPRONTE_PER_FRAME) {
                        let mut f = vec![b'H'];
                        f.extend_from_slice(&blocco.concat());
                        c.manda(&f).await?;
                    }
                    c.manda(b"h").await?;
                }
                Some(b'G') if rich.len() == 9 => {
                    let n = u64::from_be_bytes(rich[1..9].try_into().unwrap());
                    if n >= t.npezzi {
                        return Err("pezzo inesistente".into());
                    }
                    let inizio = n * PEZZO;
                    let lung = PEZZO.min(t.dim - inizio) as usize;
                    let mut buf = vec![0u8; lung];
                    file.seek(SeekFrom::Start(inizio))?;
                    file.read_exact(&mut buf)?;
                    for (k, parte) in buf.chunks(BLOCCO_DATI).enumerate() {
                        let mut f = Vec::with_capacity(13 + parte.len());
                        f.push(b'B');
                        f.extend_from_slice(&n.to_be_bytes());
                        f.extend_from_slice(&((k * BLOCCO_DATI) as u32).to_be_bytes());
                        f.extend_from_slice(parte);
                        c.manda(&f).await?;
                    }
                    let mut z = vec![b'Z'];
                    z.extend_from_slice(&n.to_be_bytes());
                    c.manda(&z).await?;
                }
                _ => return Err("richiesta non valida".into()),
            }
        }
    }

    // --- scaricamento (chi riceve) ------------------------------------------------------------
    /// Scarica i pezzi mancanti su più canali paralleli; alla fine verifica e consegna il file.
    pub(super) async fn x_scarica(self: Arc<Self>, id: String) {
        if !self.trasf.attivi.lock().unwrap().insert(id.clone()) {
            return; // sta già scaricando
        }
        let esito = self.x_scarica_dentro(&id).await;
        self.trasf.attivi.lock().unwrap().remove(&id);
        if let Err(e) = esito {
            // fermato da noi o dal contatto: lo stato è già quello giusto
            if self.x_prendi(&id).is_some_and(|t| t.stato == "corre") {
                self.x_stato(&id, "attesa", &format!("interrotto ({e}): riprende quando il contatto è in linea"));
            }
        }
    }

    async fn x_scarica_dentro(self: &Arc<Self>, id: &str) -> Risultato<()> {
        let t = self.x_prendi(id).ok_or("sconosciuto")?;
        if t.verso != Verso::In || !t.accettato || t.stato != "corre" {
            return Ok(());
        }
        // 1) la distinta: impronte dei pezzi, controllate con la radice
        let impronte = match self.x_impronte(id) {
            Some(i) if i.len() as u64 == t.npezzi => i,
            _ => {
                let mut c = self.x_apri_canale(&t.peer, id).await?;
                c.manda(b"M").await?;
                let mut tutte: Vec<[u8; 32]> = Vec::new();
                loop {
                    let f = c.ricevi().await?.ok_or("distinta interrotta")?;
                    match f.first() {
                        Some(b'H') if (f.len() - 1) % 32 == 0 => {
                            tutte.extend(f[1..].chunks(32).map(|x| <[u8; 32]>::try_from(x).unwrap()));
                            if tutte.len() as u64 > t.npezzi {
                                return Err("distinta troppo lunga".into());
                            }
                        }
                        Some(b'h') => break,
                        Some(b'E') => return Err(String::from_utf8_lossy(&f[1..]).to_string().into()),
                        _ => return Err("distinta malformata".into()),
                    }
                }
                if tutte.len() as u64 != t.npezzi || radice_di(&tutte) != t.radice {
                    self.x_stato(id, "errore", "la distinta del file non torna con la proposta");
                    return Ok(());
                }
                self.x_impronte_salva(id, &tutte);
                tutte
            }
        };
        let impronte = Arc::new(impronte);

        // 2) il file in costruzione, già della misura giusta
        {
            let mut f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&t.parziale)?;
            let nuovo = f.metadata()?.len() == 0;
            match crate::cassaforte::Sigillo::nuovo(PEZZO, t.dim).filter(|_| nuovo) {
                // cassaforte aperta: intestazione + pezzi sigillati, mai in chiaro sul disco
                Some((s, testa)) => {
                    f.write_all(&testa)?;
                    f.set_len(s.dimensione_file())?;
                }
                // in chiaro (o un file già cominciato: resta com'è, sigillato o no)
                None if !crate::cassaforte::e_sigillato(&t.parziale) => {
                    if f.metadata()?.len() != t.dim {
                        f.set_len(t.dim)?;
                    }
                }
                None => {}
            }
        }
        let mappa = self.x_mappa(id, t.npezzi);
        let mancanti: VecDeque<u64> = (0..t.npezzi).filter(|n| !mappa[*n as usize]).collect();
        let fatti_byte: u64 = (0..t.npezzi).filter(|n| mappa[*n as usize]).map(|n| PEZZO.min(t.dim - n * PEZZO)).sum();
        let comune = Arc::new(Comune {
            coda: Mutex::new(mancanti),
            in_corso: Mutex::new(0),
            sbagli: Mutex::new(HashMap::new()),
            fatti: Mutex::new(fatti_byte),
            rotto: Mutex::new(None),
            avviso: Mutex::new((Instant::now(), fatti_byte)),
        });

        // 3) i canali in parallelo; se uno cade quando gli altri hanno finito, i suoi pezzi
        //    tornano in coda e si fa un altro giro (al massimo GIRI)
        const GIRI: usize = 3;
        let canali = self.trasf.imp.lock().unwrap().canali;
        let mut errori = Vec::new();
        for _ in 0..GIRI {
            let mut lavori = Vec::new();
            for _ in 0..canali {
                let (st, c, imp, id, t) = (self.clone(), comune.clone(), impronte.clone(), id.to_string(), t.clone());
                lavori.push(tokio::spawn(async move { st.x_canale_lavoro(&t, &id, &c, &imp).await }));
            }
            for l in lavori {
                if let Ok(Err(e)) = l.await {
                    errori.push(e.to_string());
                }
            }
            let fermo = comune.rotto.lock().unwrap().is_some() || self.x_prendi(id).is_none_or(|t| t.stato != "corre");
            if fermo || comune.finito() {
                break;
            }
        }
        if let Some(e) = comune.rotto.lock().unwrap().take() {
            self.x_stato(id, "errore", &e);
            return Ok(());
        }
        if self.x_prendi(id).is_none_or(|t| t.stato != "corre") {
            return Ok(()); // pausa o annullato nel frattempo
        }
        let mappa = self.x_mappa(id, t.npezzi);
        if !mappa.iter().all(|x| *x) {
            return Err(errori.first().cloned().unwrap_or_else(|| "pezzi mancanti".into()).into());
        }

        // 4) completo: controllo finale dei pezzi su disco, poi il nome vero
        let (p, imp2, dim) = (t.parziale.clone(), impronte.clone(), t.dim);
        let integro = tokio::task::spawn_blocking(move || calcola_impronte(&p, dim).map(|v| v == *imp2)).await;
        if !matches!(integro, Ok(Ok(true))) {
            let _ = std::fs::remove_file(self.x_dir().join(format!("{id}.mappa")));
            self.x_stato(id, "errore", "il file ricevuto non torna con le impronte: riprova");
            return Ok(());
        }
        let cartella = t.parziale.parent().map(|x| x.to_path_buf()).unwrap_or_else(|| self.dati.clone());
        let finale = percorso_libero(&cartella, &t.nome);
        std::fs::rename(&t.parziale, &finale)?;
        // cominciato in chiaro (prima della cassaforte) e finito ora: si sigilla
        let _ = crate::cassaforte::sigilla_esistente(&finale);
        let _ = std::fs::remove_file(self.x_dir().join(format!("{id}.mappa")));
        if let Some(x) = self.trasf.elenco.lock().unwrap().get_mut(id) {
            x.percorso = finale.clone();
            x.byte_fatti = x.dim;
        }
        self.x_stato(id, "fatto", "");
        // nella chat il file sta all'ORA D'ARRIVO (uguale per tutti e due: va nell'XF), non a
        // quella della proposta: un file lungo comparirebbe in alto fra i messaggi vecchi e la
        // spunta risulterebbe già «letta» per letture di messaggi successivi
        let ts_chat = ora().max(t.ts);
        let _ = self.x_segnale(&t.peer, format!("XF {id} {ts_chat}")).await;
        // nella conversazione compare il file ricevuto, come un allegato (se l'utente lo vuole)
        let nome_hex = in_esadecimale(t.nome.as_bytes());
        let percorso_hex = in_esadecimale(finale.to_string_lossy().as_bytes());
        if self.trasf.imp.lock().unwrap().in_chat {
            self.registra_riga(&t.peer, &format!("fin {ts_chat} {id} {} {} {nome_hex} {percorso_hex}", t.dim, t.mime));
            self.notifica(format!("FILE {} in {ts_chat} {id} {} {} {nome_hex} {percorso_hex}", t.peer, t.dim, t.mime));
            self.conferma(&t.peer, format!("K {ts_chat}").into_bytes());     // ✓✓ consegnato anche in chat
        }
        self.notifica(format!("XFINE {id} {percorso_hex}"));
        Ok(())
    }

    /// Un canale: prende pezzi dalla coda, li chiede (alcuni in anticipo), li verifica e scrive.
    async fn x_canale_lavoro(self: &Arc<Self>, t: &Trasf, id: &str, c: &Arc<Comune>, impronte: &Arc<Vec<[u8; 32]>>) -> Risultato<()> {
        let mut riprovi = 0;
        loop {
            if c.finito() || self.x_prendi(id).is_none_or(|x| x.stato != "corre") || c.rotto.lock().unwrap().is_some() {
                return Ok(());
            }
            match self.x_canale_giro(t, id, c, impronte).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    riprovi += 1;
                    if riprovi > RIPROVI_CANALE {
                        return Err(e);
                    }
                    tokio::time::sleep(Duration::from_secs(2 * riprovi as u64)).await;
                }
            }
        }
    }

    async fn x_canale_giro(&self, t: &Trasf, id: &str, c: &Arc<Comune>, impronte: &Arc<Vec<[u8; 32]>>) -> Risultato<()> {
        let mut canale = self.x_apri_canale(&t.peer, id).await?;
        let mut chiesti: VecDeque<u64> = VecDeque::new();
        let mut buf: HashMap<u64, Vec<u8>> = HashMap::new();
        let esito: Risultato<()> = async {
            loop {
                // richieste in anticipo
                while chiesti.len() < IN_VOLO_PER_CANALE {
                    let Some(n) = c.prendi() else { break };
                    let mut g = vec![b'G'];
                    g.extend_from_slice(&n.to_be_bytes());
                    canale.manda(&g).await?;
                    chiesti.push_back(n);
                }
                if chiesti.is_empty() {
                    return Ok(()); // niente più da chiedere
                }
                if self.x_prendi(id).is_none_or(|x| x.stato != "corre") {
                    return Err("fermato".into());
                }
                let f = canale.ricevi().await?.ok_or("canale chiuso dal contatto")?;
                match f.first() {
                    Some(b'B') if f.len() > 13 => {
                        let n = u64::from_be_bytes(f[1..9].try_into().unwrap());
                        let off = u32::from_be_bytes(f[9..13].try_into().unwrap()) as usize;
                        if !chiesti.contains(&n) {
                            return Err("pezzo non chiesto".into());
                        }
                        let b = buf.entry(n).or_default();
                        if off != b.len() || b.len() + f.len() - 13 > PEZZO as usize {
                            return Err("pezzo fuori ordine".into());
                        }
                        b.extend_from_slice(&f[13..]);
                    }
                    Some(b'Z') if f.len() == 9 => {
                        let n = u64::from_be_bytes(f[1..9].try_into().unwrap());
                        chiesti.retain(|x| *x != n);
                        let dati = buf.remove(&n).unwrap_or_default();
                        let atteso = PEZZO.min(t.dim - n * PEZZO) as usize;
                        if dati.len() == atteso && blake3::hash(&dati).as_bytes() == &impronte[n as usize] {
                            let mut f = std::fs::OpenOptions::new().write(true).open(&t.parziale)?;
                            if crate::cassaforte::e_sigillato(&t.parziale) {
                                let s = crate::cassaforte::Sigillo::da_file(&t.parziale)?;
                                f.seek(SeekFrom::Start(s.posizione(n)))?;
                                f.write_all(&s.sigilla(n, &dati))?;
                            } else {
                                f.seek(SeekFrom::Start(n * PEZZO))?;
                                f.write_all(&dati)?;
                            }
                            self.x_mappa_segna(id, t.npezzi, n);
                            c.fatto(dati.len() as u64);
                            self.x_avanzamento(id, t, c);
                            if let Some(d) = rallenta_prova() {
                                tokio::time::sleep(d).await;
                            }
                        } else {
                            // pezzo sbagliato (o manomesso): si scarta e si richiede
                            if c.sbagliato(n) > TENTATIVI_PEZZO {
                                *c.rotto.lock().unwrap() = Some("un pezzo continua ad arrivare sbagliato: trasferimento fermato".into());
                                return Ok(());
                            }
                            c.rimetti(n);
                        }
                    }
                    Some(b'E') => return Err(String::from_utf8_lossy(&f[1..]).to_string().into()),
                    _ => return Err("risposta non valida".into()),
                }
            }
        }
        .await;
        // quello che era chiesto e non è arrivato torna in coda per gli altri canali
        for n in chiesti {
            c.rimetti(n);
        }
        c.chiuso_uno();
        esito
    }

    fn x_avanzamento(&self, id: &str, t: &Trasf, c: &Comune) {
        let fatti = *c.fatti.lock().unwrap();
        let mut a = c.avviso.lock().unwrap();
        let dt = a.0.elapsed();
        if dt < OGNI_AVVISO && fatti < t.dim {
            return;
        }
        let bps = ((fatti.saturating_sub(a.1)) as f64 / dt.as_secs_f64().max(0.001)) as u64;
        *a = (Instant::now(), fatti);
        drop(a);
        if let Some(x) = self.trasf.elenco.lock().unwrap().get_mut(id) {
            x.byte_fatti = fatti;
        }
        self.notifica(format!("XPROG {id} {fatti} {} {bps}", t.dim));
    }
}

/// Stato condiviso fra i canali di uno scaricamento.
struct Comune {
    coda: Mutex<VecDeque<u64>>,
    in_corso: Mutex<usize>,
    sbagli: Mutex<HashMap<u64, u32>>,
    fatti: Mutex<u64>,
    rotto: Mutex<Option<String>>,
    avviso: Mutex<(Instant, u64)>,
}

impl Comune {
    fn prendi(&self) -> Option<u64> {
        let n = self.coda.lock().unwrap().pop_front()?;
        *self.in_corso.lock().unwrap() += 1;
        Some(n)
    }
    fn rimetti(&self, n: u64) {
        self.coda.lock().unwrap().push_back(n);
        let mut k = self.in_corso.lock().unwrap();
        *k = k.saturating_sub(1);
    }
    fn fatto(&self, byte: u64) {
        *self.fatti.lock().unwrap() += byte;
        let mut k = self.in_corso.lock().unwrap();
        *k = k.saturating_sub(1);
    }
    fn sbagliato(&self, n: u64) -> u32 {
        let mut g = self.sbagli.lock().unwrap();
        let v = g.entry(n).or_insert(0);
        *v += 1;
        *v
    }
    fn chiuso_uno(&self) {}
    fn finito(&self) -> bool {
        self.coda.lock().unwrap().is_empty() && *self.in_corso.lock().unwrap() == 0
    }
}

/// Un canale dati: frame sigillati con le chiavi della stretta (come la sessione della chat).
struct Canale {
    lettura: OwnedReadHalf,
    scrittura: OwnedWriteHalf,
    tx: Record,
    rx: Record,
}

impl Canale {
    fn nuovo(flusso: TcpStream, segreti: &SegretiSessione, sono_iniziatore: bool) -> Self {
        let (lettura, scrittura) = flusso.into_split();
        let (tx, rx) = if sono_iniziatore {
            (Record::nuovo(&segreti.verso_destinatario), Record::nuovo(&segreti.verso_mittente))
        } else {
            (Record::nuovo(&segreti.verso_mittente), Record::nuovo(&segreti.verso_destinatario))
        };
        Self { lettura, scrittura, tx, rx }
    }
    async fn manda(&mut self, chiaro: &[u8]) -> Risultato<()> {
        let f = self.tx.sigilla(chiaro);
        scrivi_frame(&mut self.scrittura, &f).await
    }
    async fn ricevi(&mut self) -> Risultato<Option<Vec<u8>>> {
        match leggi_frame(&mut self.lettura).await? {
            Some(f) => Ok(Some(self.rx.apri(&f)?)),
            None => Ok(None),
        }
    }
}

/// Lato servizio di un canale dati: la stessa sfida Noise della chat (`gestisci_entrante`).
pub(super) async fn sfida(flusso: &mut TcpStream, peer: &str) -> Risultato<SegretiSessione> {
    let chiave = indirizzo::in_chiave(peer).map_err(|_| "indirizzo del contatto non valido")?;
    let (mittente, msg1) = Mittente::inizia(&chiave)?;
    scrivi_frame(flusso, &msg1).await?;
    let msg2 = leggi_frame(flusso).await?.ok_or("nessuna prova d'identità")?;
    Ok(mittente.completa(&msg2)?)
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn stato_andata_e_ritorno() {
        let t = Trasf {
            id: "abcdef0123456789".into(),
            verso: Verso::In,
            peer: "x.nyct".into(),
            nome: "foto vacanze.jpg".into(),
            mime: "image/jpeg".into(),
            dim: 1_000_000,
            npezzi: npezzi_di(1_000_000),
            radice: "a".repeat(64),
            ts: 42,
            stato: "corre".into(),
            accettato: true,
            percorso: PathBuf::from("/tmp/x y"),
            parziale: PathBuf::from("/tmp/.p.part"),
            byte_fatti: 12345,
        };
        let r = leggi_trasf(&scrivi_trasf(&t)).unwrap();
        assert_eq!((r.nome.as_str(), r.npezzi, r.verso, r.byte_fatti, r.accettato), ("foto vacanze.jpg", 4, Verso::In, 12345, true));
        assert_eq!(r.percorso, PathBuf::from("/tmp/x y"));
    }

    #[test]
    fn impronte_dei_pezzi_e_radice() {
        let dir = std::env::temp_dir().join(format!("nyct-trasf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("f.bin");
        let dati: Vec<u8> = (0..(PEZZO as usize * 2 + 1000)).map(|i| (i % 251) as u8).collect();
        std::fs::write(&p, &dati).unwrap();
        let imp = calcola_impronte(&p, dati.len() as u64).unwrap();
        assert_eq!(imp.len(), 3);
        assert_eq!(&imp[2], blake3::hash(&dati[PEZZO as usize * 2..]).as_bytes());
        assert_eq!(radice_di(&imp).len(), 64);
        assert!(calcola_impronte(&p, dati.len() as u64 + 1).is_err()); // file diverso da quanto annunciato
        let vuoto = dir.join("vuoto");
        std::fs::write(&vuoto, b"").unwrap();
        assert_eq!(calcola_impronte(&vuoto, 0).unwrap().len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn misure() {
        assert_eq!(npezzi_di(0), 1);
        assert_eq!(npezzi_di(PEZZO), 1);
        assert_eq!(npezzi_di(PEZZO + 1), 2);
    }
}
