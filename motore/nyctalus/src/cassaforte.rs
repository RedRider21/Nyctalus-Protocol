// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! **Cassaforte** dei dati locali (piano di sicurezza, punto 7): identità, contatti, cronologia
//! della chat, file ricevuti… cifrati sul disco con una chiave che nasce dalla **passphrase**
//! dell'utente, chiesta a ogni avvio. Chi copia il disco (o ruba il PC/telefono spento) non legge
//! niente.
//!
//! - Chiave: **Argon2id** (passphrase + sale casuale; 64 MiB, 3 passaggi) → 32 byte. Mai salvata.
//! - File: **XChaCha20-Poly1305** con nonce casuale per ogni scrittura; formato
//!   `NYCF1` ‖ nonce (24) ‖ testo cifrato con etichetta. Un file alterato non si apre.
//! - `cassaforte.json` nella cartella dei dati: versione, sale, parametri e un «campione» cifrato
//!   per riconoscere subito una passphrase sbagliata.
//! - I file non cifrati (scritti prima di attivare la cassaforte) si leggono ancora: la prima
//!   riscrittura li cifra; all'attivazione si cifrano subito tutti.
//!
//! Se la passphrase si dimentica i dati sono persi: è il prezzo della protezione.

use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroize;

use crate::Risultato;

const MAGIA: &[u8; 5] = b"NYCF1";
const FILE_CASSAFORTE: &str = "cassaforte.json";
const CAMPIONE: &[u8] = b"Nyctalus: cassaforte aperta";
/// Parametri di Argon2id (memoria in KiB, passaggi, corsie). Abbastanza pesanti da rallentare chi
/// prova passphrase a raffica, leggeri per un telefono (~0,5 s).
const MEMORIA_KIB: u32 = 64 * 1024;
const PASSAGGI: u32 = 3;
const CORSIE: u32 = 1;

/// La chiave della cassaforte aperta (azzerata quando si chiude o si cambia).
pub struct Chiave([u8; 32]);

impl Drop for Chiave {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Cassaforte aperta in questo processo (al più una: la cartella dei dati della chat).
fn aperta() -> &'static RwLock<Option<Chiave>> {
    static A: OnceLock<RwLock<Option<Chiave>>> = OnceLock::new();
    A.get_or_init(|| RwLock::new(None))
}

pub fn file_cassaforte(dati: &Path) -> PathBuf {
    dati.join(FILE_CASSAFORTE)
}

/// C'è una cassaforte in questa cartella (i dati sono cifrati)?
pub fn esiste(dati: &Path) -> bool {
    file_cassaforte(dati).exists()
}

pub fn e_aperta() -> bool {
    aperta().read().unwrap().is_some()
}

fn deriva(passphrase: &[u8], sale: &[u8], m: u32, t: u32, p: u32) -> Risultato<[u8; 32]> {
    let parametri = argon2::Params::new(m, t, p, Some(32)).map_err(|e| format!("Argon2: {e}"))?;
    let a = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, parametri);
    let mut k = [0u8; 32];
    a.hash_password_into(passphrase, sale, &mut k).map_err(|e| format!("Argon2: {e}"))?;
    Ok(k)
}

fn sigilla(k: &[u8; 32], dati: &[u8]) -> Vec<u8> {
    let mut nonce = [0u8; 24];
    getrandom::fill(&mut nonce).expect("generatore casuale di sistema");
    let c = XChaCha20Poly1305::new(k.into()).encrypt(&XNonce::from(nonce), dati).expect("cifratura");
    let mut v = Vec::with_capacity(MAGIA.len() + 24 + c.len());
    v.extend_from_slice(MAGIA);
    v.extend_from_slice(&nonce);
    v.extend_from_slice(&c);
    v
}

fn apri_con(k: &[u8; 32], d: &[u8]) -> Option<Vec<u8>> {
    let resto = d.strip_prefix(MAGIA.as_slice())?;
    if resto.len() < 24 + 16 {
        return None;
    }
    let (nonce, c) = resto.split_at(24);
    XChaCha20Poly1305::new(k.into()).decrypt(&XNonce::try_from(nonce).ok()?, c).ok()
}

fn esa(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn da_esa(t: &str) -> Option<Vec<u8>> {
    if !t.len().is_multiple_of(2) || !t.is_ascii() {
        return None;
    }
    (0..t.len()).step_by(2).map(|i| u8::from_str_radix(&t[i..i + 2], 16).ok()).collect()
}

fn leggi_descrittore(dati: &Path) -> Risultato<(Vec<u8>, u32, u32, u32, Vec<u8>)> {
    let testo = std::fs::read_to_string(file_cassaforte(dati))?;
    let campo = |k: &str| -> Option<String> {
        let i = testo.find(&format!("\"{k}\""))?;
        let resto = &testo[i + k.len() + 2..];
        let resto = resto[resto.find(':')? + 1..].trim_start();
        let fine = resto.find([',', '}', '\n']).unwrap_or(resto.len());
        Some(resto[..fine].trim().trim_matches('"').to_string())
    };
    let num = |k: &str| campo(k).and_then(|v| v.parse::<u32>().ok()).ok_or_else(|| format!("cassaforte.json: manca {k}"));
    let sale = campo("sale").and_then(|s| da_esa(&s)).ok_or("cassaforte.json: sale non valido")?;
    let campione = campo("campione").and_then(|s| da_esa(&s)).ok_or("cassaforte.json: campione non valido")?;
    Ok((sale, num("memoria_kib")?, num("passaggi")?, num("corsie")?, campione))
}

/// Apre la cassaforte con la passphrase (errore se sbagliata). Dopo, `leggi`/`scrivi` cifrano.
pub fn apri(dati: &Path, passphrase: &[u8]) -> Risultato<()> {
    let (sale, m, t, p, campione) = leggi_descrittore(dati)?;
    let k = deriva(passphrase, &sale, m, t, p)?;
    if apri_con(&k, &campione).as_deref() != Some(CAMPIONE) {
        let mut k = k;
        k.zeroize();
        return Err("passphrase sbagliata".into());
    }
    *aperta().write().unwrap() = Some(Chiave(k));
    Ok(())
}

/// Crea la cassaforte (o ne cambia la passphrase) e RICIFRA tutti i file della cartella con la
/// chiave nuova. Se c'era una cassaforte, deve essere già aperta (la vecchia chiave serve a leggere).
pub fn imposta(dati: &Path, passphrase_nuova: &[u8]) -> Risultato<usize> {
    if passphrase_nuova.len() < 8 {
        return Err("passphrase troppo corta (almeno 8 caratteri)".into());
    }
    if esiste(dati) && !e_aperta() {
        return Err("la cassaforte è chiusa: prima sbloccala".into());
    }
    // tutti i file da proteggere, letti con la chiave attuale (o in chiaro)
    let file = file_da_proteggere(dati);
    let mut contenuti = Vec::with_capacity(file.len());
    for f in &file {
        contenuti.push(leggi(f)?);
    }
    // file ricevuti già sigillati (cambio della passphrase): le loro chiavi, lette con quella vecchia
    let ricevuti = file_ricevuti(dati);
    let mut sigilli = Vec::new();
    for f in ricevuti.iter().filter(|f| e_sigillato(f)) {
        sigilli.push((f.clone(), Sigillo::da_file(f)?));
    }
    let mut sale = [0u8; 16];
    getrandom::fill(&mut sale).expect("generatore casuale di sistema");
    let k = deriva(passphrase_nuova, &sale, MEMORIA_KIB, PASSAGGI, CORSIE)?;
    let descrittore = format!(
        "{{\n \"v\": 1,\n \"sale\": \"{}\",\n \"memoria_kib\": {MEMORIA_KIB},\n \"passaggi\": {PASSAGGI},\n \"corsie\": {CORSIE},\n \"campione\": \"{}\"\n}}\n",
        esa(&sale),
        esa(&sigilla(&k, CAMPIONE))
    );
    *aperta().write().unwrap() = Some(Chiave(k));
    for (f, c) in file.iter().zip(contenuti) {
        scrivi(f, &c)?;
    }
    // intestazioni dei file già sigillati: stessa chiave del file, protetta dalla chiave nuova
    for (f, s) in &sigilli {
        use std::io::{Seek, SeekFrom, Write};
        let mut w = std::fs::OpenOptions::new().write(true).open(f)?;
        w.seek(SeekFrom::Start(0))?;
        w.write_all(&s.testa(&k))?;
        w.sync_all()?;
    }
    // file ricevuti ancora in chiaro (prima attivazione): si sigillano
    let mut quanti = file.len() + sigilli.len();
    for f in ricevuti.iter().filter(|f| !e_sigillato(f)) {
        sigilla_esistente(f)?;
        quanti += 1;
    }
    scrivi_atomico(&file_cassaforte(dati), descrittore.as_bytes())?;
    Ok(quanti)
}

/// Toglie la cassaforte: tutti i file tornano in chiaro (serve la cassaforte aperta).
pub fn togli(dati: &Path) -> Risultato<usize> {
    if !e_aperta() {
        return Err("la cassaforte è chiusa: prima sbloccala".into());
    }
    let file = file_da_proteggere(dati);
    let mut contenuti = Vec::with_capacity(file.len());
    for f in &file {
        contenuti.push(leggi(f)?);
    }
    let ricevuti = file_ricevuti(dati);
    for f in &ricevuti {
        dissigilla(f)?;
    }
    *aperta().write().unwrap() = None;
    for (f, c) in file.iter().zip(contenuti) {
        scrivi_atomico(f, &c)?;
    }
    std::fs::remove_file(file_cassaforte(dati))?;
    Ok(file.len() + ricevuti.len())
}

/// I file del MOTORE da proteggere: un elenco preciso (nella stessa cartella ci sono anche file
/// delle interfacce, come `ui.json`, che devono restare leggibili da loro). Allegati e trasferimenti
/// hanno una gestione a parte.
const FILE_MOTORE: &[&str] = &["identita", "contatti", "visti", "coda-riservata", "casella-viste", "riservata"];
const CARTELLE_MOTORE: &[&str] = &["cronologia", "stati"];

fn file_da_proteggere(dati: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = FILE_MOTORE.iter().map(|f| dati.join(f)).filter(|p| p.is_file()).collect();
    for c in CARTELLE_MOTORE {
        if let Ok(e) = std::fs::read_dir(dati.join(c)) {
            for x in e.flatten() {
                let p = x.path();
                if p.is_file() && !p.to_string_lossy().ends_with(".tmp") {
                    v.push(p);
                }
            }
        }
    }
    v
}

/// File «a righe» (si aggiunge una riga per volta): cifrati riga per riga, così aggiungere un
/// messaggio non obbliga a riscrivere tutta la cronologia.
fn e_a_righe(f: &Path) -> bool {
    let cartella = f.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("");
    CARTELLE_MOTORE.contains(&cartella) || f.file_name().and_then(|n| n.to_str()) == Some("casella-viste")
}

const RIGA: &str = "NYCR1 ";

/// Aggiunge una riga a un file a righe: cifrata se la cassaforte è aperta.
pub fn aggiungi_riga(f: &Path, riga: &str) -> Risultato<()> {
    use std::io::Write;
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    let testo = {
        let g = aperta().read().unwrap();
        match g.as_ref() {
            Some(k) => format!("{RIGA}{}\n", esa(&sigilla(&k.0, riga.as_bytes()))),
            None => format!("{riga}\n"),
        }
    };
    let mut o = std::fs::OpenOptions::new();
    o.create(true).append(true);
    #[cfg(unix)]
    o.mode(0o600);
    o.open(f)?.write_all(testo.as_bytes())?;
    Ok(())
}

/// Legge un file dei dati: lo decifra se è cifrato (serve la cassaforte aperta), altrimenti lo
/// restituisce com'è (file di prima della cassaforte).
pub fn leggi(f: &Path) -> Risultato<Vec<u8>> {
    let d = std::fs::read(f)?;
    if !d.starts_with(MAGIA) {
        // file a righe: le righe cifrate si aprono una per una, quelle in chiaro (di prima) restano
        if d.starts_with(RIGA.as_bytes()) || d.windows(RIGA.len() + 1).any(|w| w[0] == b'\n' && &w[1..] == RIGA.as_bytes()) {
            let testo = String::from_utf8_lossy(&d).into_owned();
            let g = aperta().read().unwrap();
            let k = g.as_ref().ok_or("dati cifrati e cassaforte chiusa")?;
            let mut out = Vec::with_capacity(d.len() / 2);
            for r in testo.lines() {
                match r.strip_prefix(RIGA) {
                    Some(h) => {
                        let chiaro = da_esa(h).and_then(|b| apri_con(&k.0, &b))
                            .ok_or_else(|| format!("{}: riga cifrata rovinata", f.display()))?;
                        out.extend_from_slice(&chiaro);
                    }
                    None => out.extend_from_slice(r.as_bytes()),
                }
                out.push(b'\n');
            }
            return Ok(out);
        }
        return Ok(d);
    }
    let g = aperta().read().unwrap();
    let k = g.as_ref().ok_or("dati cifrati e cassaforte chiusa")?;
    apri_con(&k.0, &d).ok_or_else(|| format!("{}: file cifrato rovinato o di un'altra cassaforte", f.display()).into())
}

pub fn leggi_testo(f: &Path) -> Risultato<String> {
    Ok(String::from_utf8(leggi(f)?)?)
}

/// Scrive un file dei dati: cifrato se la cassaforte è aperta, in chiaro altrimenti. Sempre in
/// modo atomico (file temporaneo + rinomina), così un'interruzione non lascia file a metà.
pub fn scrivi(f: &Path, dati: &[u8]) -> Risultato<()> {
    let g = aperta().read().unwrap();
    match g.as_ref() {
        Some(k) if e_a_righe(f) => {
            let mut out = String::new();
            for r in String::from_utf8_lossy(dati).lines() {
                out.push_str(RIGA);
                out.push_str(&esa(&sigilla(&k.0, r.as_bytes())));
                out.push('\n');
            }
            scrivi_atomico(f, out.as_bytes())
        }
        Some(k) => scrivi_atomico(f, &sigilla(&k.0, dati)),
        None => scrivi_atomico(f, dati),
    }
}

fn scrivi_atomico(f: &Path, dati: &[u8]) -> Risultato<()> {
    let tmp = f.with_extension(format!("{}.tmp", f.extension().and_then(|e| e.to_str()).unwrap_or("")));
    {
        use std::io::Write;
        #[cfg(unix)]
        use std::os::unix::fs::OpenOptionsExt;
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create(true).truncate(true);
        #[cfg(unix)]
        o.mode(0o600);
        let mut w = o.open(&tmp)?;
        w.write_all(dati)?;
        w.sync_all()?;
    }
    std::fs::rename(&tmp, f)?;
    Ok(())
}

// --- file ricevuti (allegati e file grandi): cifrati a pezzi, aperti al volo --------------------
//
// Formato `NYCS1`: intestazione di 89 byte = magia (5) ‖ pezzo (u32) ‖ lunghezza in chiaro (u64) ‖
// nonce (24) ‖ chiave del FILE sigillata con la chiave della cassaforte (32 + 16). Poi i pezzi,
// ognuno sigillato con la chiave del file (nonce = numero del pezzo, che è anche autenticato):
// si possono scrivere in qualunque ordine e riprendere dopo una caduta, e il file non sta mai in
// chiaro sul disco. Ogni file ha la SUA chiave: cambiando la passphrase si riscrive solo
// l'intestazione. La lunghezza è autenticata: un file troncato non si apre.

const MAGIA_FILE: &[u8; 5] = b"NYCS1";
pub const TESTA_FILE: u64 = 5 + 4 + 8 + 24 + 48;
/// Pezzo degli allegati della chat (i file grandi usano il loro, 256 KiB).
pub const PEZZO_ALLEGATI: u64 = 64 * 1024;

pub struct Sigillo {
    k: [u8; 32],
    pub pezzo: u64,
    pub lunghezza: u64,
}

impl Drop for Sigillo {
    fn drop(&mut self) {
        self.k.zeroize();
    }
}

fn nonce_pezzo(n: u64) -> XNonce {
    let mut b = [0u8; 24];
    b[..8].copy_from_slice(&n.to_le_bytes());
    XNonce::from(b)
}

impl Sigillo {
    /// Un sigillo nuovo per un file di `lunghezza` byte (serve la cassaforte aperta): con la sua
    /// intestazione da scrivere in testa al file.
    pub fn nuovo(pezzo: u64, lunghezza: u64) -> Option<(Self, Vec<u8>)> {
        let g = aperta().read().unwrap();
        let maestra = g.as_ref()?;
        let mut k = [0u8; 32];
        getrandom::fill(&mut k).expect("generatore casuale di sistema");
        let s = Self { k, pezzo, lunghezza };
        let testa = s.testa(&maestra.0);
        Some((s, testa))
    }

    fn testa(&self, maestra: &[u8; 32]) -> Vec<u8> {
        let mut t = Vec::with_capacity(TESTA_FILE as usize);
        t.extend_from_slice(MAGIA_FILE);
        t.extend_from_slice(&(self.pezzo as u32).to_le_bytes());
        t.extend_from_slice(&self.lunghezza.to_le_bytes());
        let mut nonce = [0u8; 24];
        getrandom::fill(&mut nonce).expect("generatore casuale di sistema");
        let c = XChaCha20Poly1305::new(maestra.into())
            .encrypt(&XNonce::from(nonce), chacha20poly1305::aead::Payload { msg: &self.k, aad: &t[..17] })
            .expect("cifratura");
        t.extend_from_slice(&nonce);
        t.extend_from_slice(&c);
        t
    }

    /// Il sigillo di un file dalla sua intestazione (serve la cassaforte aperta).
    pub fn da_testa(t: &[u8]) -> Risultato<Self> {
        if t.len() < TESTA_FILE as usize || !t.starts_with(MAGIA_FILE) {
            return Err("non è un file sigillato".into());
        }
        let g = aperta().read().unwrap();
        let maestra = g.as_ref().ok_or("dati cifrati e cassaforte chiusa")?;
        let pezzo = u32::from_le_bytes(t[5..9].try_into().expect("4 byte")) as u64;
        let lunghezza = u64::from_le_bytes(t[9..17].try_into().expect("8 byte"));
        let nonce: [u8; 24] = t[17..41].try_into().expect("24 byte");
        let k = XChaCha20Poly1305::new((&maestra.0).into())
            .decrypt(&XNonce::from(nonce), chacha20poly1305::aead::Payload { msg: &t[41..89], aad: &t[..17] })
            .map_err(|_| "file sigillato con un'altra cassaforte, o rovinato")?;
        if pezzo == 0 {
            return Err("file sigillato non valido".into());
        }
        Ok(Self { k: k.try_into().map_err(|_| "chiave del file non valida")?, pezzo, lunghezza })
    }

    pub fn da_file(f: &Path) -> Risultato<Self> {
        use std::io::Read;
        let mut t = vec![0u8; TESTA_FILE as usize];
        std::fs::File::open(f)?.read_exact(&mut t)?;
        Self::da_testa(&t)
    }

    pub fn npezzi(&self) -> u64 {
        self.lunghezza.div_ceil(self.pezzo)
    }

    /// Dove comincia sul disco il pezzo `n`.
    pub fn posizione(&self, n: u64) -> u64 {
        TESTA_FILE + n * (self.pezzo + 16)
    }

    /// Quanto è lungo sul disco il file completo.
    pub fn dimensione_file(&self) -> u64 {
        TESTA_FILE + self.lunghezza + self.npezzi() * 16
    }

    pub fn sigilla(&self, n: u64, dati: &[u8]) -> Vec<u8> {
        XChaCha20Poly1305::new((&self.k).into())
            .encrypt(&nonce_pezzo(n), chacha20poly1305::aead::Payload { msg: dati, aad: &n.to_le_bytes() })
            .expect("cifratura")
    }

    pub fn apri(&self, n: u64, sigillato: &[u8]) -> Option<Vec<u8>> {
        XChaCha20Poly1305::new((&self.k).into())
            .decrypt(&nonce_pezzo(n), chacha20poly1305::aead::Payload { msg: sigillato, aad: &n.to_le_bytes() })
            .ok()
    }

    /// Legge e apre il pezzo `n` dal file.
    pub fn leggi_pezzo(&self, file: &mut std::fs::File, n: u64) -> Risultato<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let chiaro = self.pezzo.min(self.lunghezza - n * self.pezzo) as usize;
        let mut b = vec![0u8; chiaro + 16];
        file.seek(SeekFrom::Start(self.posizione(n)))?;
        file.read_exact(&mut b)?;
        self.apri(n, &b).ok_or_else(|| format!("pezzo {n} rovinato").into())
    }
}

/// Il file è sigillato (formato `NYCS1`)?
pub fn e_sigillato(f: &Path) -> bool {
    use std::io::Read;
    let mut m = [0u8; 5];
    std::fs::File::open(f).and_then(|mut x| x.read_exact(&mut m)).is_ok() && &m == MAGIA_FILE
}

/// Scrittore in sequenza di un file ricevuto: sigillato a pezzi se la cassaforte è aperta,
/// in chiaro altrimenti (come prima).
pub struct Scrittore {
    file: std::fs::File,
    sigillo: Option<Sigillo>,
    tampone: Vec<u8>,
    n: u64,
}

impl Scrittore {
    pub fn crea(f: &Path, lunghezza: u64) -> std::io::Result<Self> {
        use std::io::Write;
        let mut file = std::fs::File::create(f)?;
        let sigillo = match Sigillo::nuovo(PEZZO_ALLEGATI, lunghezza) {
            Some((s, testa)) => {
                file.write_all(&testa)?;
                Some(s)
            }
            None => None,
        };
        Ok(Self { file, sigillo, tampone: Vec::new(), n: 0 })
    }

    pub fn scrivi(&mut self, dati: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        let Some(s) = &self.sigillo else { return self.file.write_all(dati) };
        self.tampone.extend_from_slice(dati);
        while self.tampone.len() as u64 >= s.pezzo {
            let resto = self.tampone.split_off(s.pezzo as usize);
            self.file.write_all(&s.sigilla(self.n, &self.tampone))?;
            self.n += 1;
            self.tampone = resto;
        }
        Ok(())
    }

    /// Scrive l'ultimo pezzo (se c'è) e chiude.
    pub fn chiudi(mut self) -> std::io::Result<()> {
        use std::io::Write;
        if let Some(s) = &self.sigillo
            && !self.tampone.is_empty()
        {
            self.file.write_all(&s.sigilla(self.n, &self.tampone))?;
        }
        self.file.flush()
    }
}

/// Cartella della «vista in chiaro»: i file ricevuti decifrati al volo quando si aprono. In RAM
/// dove c'è (`$XDG_RUNTIME_DIR`, sparisce alla chiusura della sessione), altrimenti nella cartella
/// dei dati; svuotata a ogni avvio del motore.
fn vista() -> &'static RwLock<Option<PathBuf>> {
    static V: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();
    V.get_or_init(|| RwLock::new(None))
}

/// Prepara (e svuota) la vista in chiaro per questa cartella dei dati.
pub fn prepara_vista(dati: &Path) -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|d| d.is_dir())
        .map(|d| d.join(format!("nyctalus-aperti-{}", &blake3::hash(dati.to_string_lossy().as_bytes()).to_hex()[..12])))
        .unwrap_or_else(|| dati.join(".aperti"));
    let _ = std::fs::remove_dir_all(&base);
    let _ = std::fs::create_dir_all(&base);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700));
    }
    *vista().write().unwrap() = Some(base.clone());
    base
}

/// Svuota la vista in chiaro (alla chiusura).
pub fn svuota_vista() {
    if let Some(v) = vista().read().unwrap().as_ref() {
        let _ = std::fs::remove_dir_all(v);
    }
}

/// Il percorso da cui LEGGERE un file ricevuto: lui stesso se è in chiaro; se è sigillato, una
/// copia decifrata nella vista (fatta la prima volta, poi riusata).
pub fn in_chiaro(f: &Path) -> Risultato<PathBuf> {
    use std::io::Write;
    if !e_sigillato(f) {
        return Ok(f.to_path_buf());
    }
    let s = Sigillo::da_file(f)?;
    let base = vista().read().unwrap().clone().ok_or("vista in chiaro non pronta")?;
    let dir = base.join(&blake3::hash(f.to_string_lossy().as_bytes()).to_hex()[..16]);
    let dest = dir.join(f.file_name().unwrap_or_default());
    if dest.metadata().map(|m| m.len() == s.lunghezza).unwrap_or(false) {
        return Ok(dest);
    }
    std::fs::create_dir_all(&dir)?;
    let tmp = dir.join(".in-corso");
    {
        let mut origine = std::fs::File::open(f)?;
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            o.mode(0o600);
        }
        let mut w = o.open(&tmp)?;
        for n in 0..s.npezzi() {
            w.write_all(&s.leggi_pezzo(&mut origine, n)?)?;
        }
        w.flush()?;
    }
    std::fs::rename(&tmp, &dest)?;
    Ok(dest)
}

/// Sigilla un file ricevuto che è ancora in chiaro (attivazione della cassaforte, o un file
/// finito in chiaro perché cominciato prima).
pub fn sigilla_esistente(f: &Path) -> Risultato<()> {
    use std::io::{Read, Write};
    if e_sigillato(f) || !e_aperta() {
        return Ok(());
    }
    let lunghezza = f.metadata()?.len();
    let (s, testa) = Sigillo::nuovo(PEZZO_ALLEGATI, lunghezza).ok_or("cassaforte chiusa")?;
    let tmp = f.with_extension("sigillo.tmp");
    {
        let mut origine = std::fs::File::open(f)?;
        let mut w = std::fs::File::create(&tmp)?;
        w.write_all(&testa)?;
        let mut b = vec![0u8; s.pezzo as usize];
        for n in 0..s.npezzi() {
            let k = s.pezzo.min(lunghezza - n * s.pezzo) as usize;
            origine.read_exact(&mut b[..k])?;
            w.write_all(&s.sigilla(n, &b[..k]))?;
        }
        w.sync_all()?;
    }
    std::fs::rename(&tmp, f)?;
    Ok(())
}

/// Riporta in chiaro un file sigillato (quando si toglie la cassaforte).
fn dissigilla(f: &Path) -> Risultato<()> {
    use std::io::Write;
    if !e_sigillato(f) {
        return Ok(());
    }
    let s = Sigillo::da_file(f)?;
    let tmp = f.with_extension("chiaro.tmp");
    {
        let mut origine = std::fs::File::open(f)?;
        let mut w = std::fs::File::create(&tmp)?;
        for n in 0..s.npezzi() {
            w.write_all(&s.leggi_pezzo(&mut origine, n)?)?;
        }
        w.sync_all()?;
    }
    std::fs::rename(&tmp, f)?;
    Ok(())
}

/// I file ricevuti COMPLETI (non quelli a metà: `.part`) sotto `allegati/`.
fn file_ricevuti(dati: &Path) -> Vec<PathBuf> {
    fn giro(d: &Path, v: &mut Vec<PathBuf>) {
        let Ok(e) = std::fs::read_dir(d) else { return };
        for x in e.flatten() {
            let p = x.path();
            let nome = x.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                giro(&p, v);
            } else if p.is_file() && !nome.ends_with(".part") && !nome.ends_with(".tmp") {
                v.push(p);
            }
        }
    }
    let mut v = Vec::new();
    giro(&dati.join("allegati"), &mut v);
    v
}

#[cfg(test)]
pub(crate) mod test {
    use super::*;

    /// Le prove della cassaforte (stato globale del processo) passano una alla volta.
    pub(crate) fn blocco() -> std::sync::MutexGuard<'static, ()> {
        static B: std::sync::Mutex<()> = std::sync::Mutex::new(());
        B.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[test]
    fn attiva_cifra_tutto_apre_con_la_passphrase_giusta_e_si_toglie() {
        let _b = blocco();
        *aperta().write().unwrap() = None;
        let d = std::env::temp_dir().join(format!("nyct-cassaforte-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("cronologia")).unwrap();
        std::fs::write(d.join("contatti"), b"mario abc.nyct\n").unwrap();
        std::fs::write(d.join("cronologia").join("abc"), b"ciao Mario").unwrap();
        // attivazione: i file esistenti diventano cifrati
        assert!(imposta(&d, b"corta").is_err(), "passphrase troppo corta");
        assert_eq!(imposta(&d, b"una passphrase lunga").unwrap(), 2);
        let grezzo = std::fs::read(d.join("contatti")).unwrap();
        assert!(grezzo.starts_with(MAGIA) && !grezzo.windows(5).any(|w| w == b"mario"), "sul disco non c'è più il testo");
        assert_eq!(leggi(&d.join("cronologia").join("abc")).unwrap(), b"ciao Mario\n");
        // file a righe: una riga aggiunta resta cifrata e si legge insieme alle altre
        aggiungi_riga(&d.join("cronologia").join("abc"), "come stai?").unwrap();
        let grezzo = std::fs::read_to_string(d.join("cronologia").join("abc")).unwrap();
        assert!(grezzo.lines().all(|l| l.starts_with(RIGA)) && !grezzo.contains("come"), "righe tutte cifrate");
        assert_eq!(leggi(&d.join("cronologia").join("abc")).unwrap(), b"ciao Mario\ncome stai?\n");
        // file non del motore (es. ui.json delle interfacce): mai toccato
        std::fs::write(d.join("ui.json"), b"{}").unwrap();
        imposta(&d, b"una passphrase lunga").unwrap();
        assert_eq!(std::fs::read(d.join("ui.json")).unwrap(), b"{}");
        // chiusa: non si legge; passphrase sbagliata: errore; giusta: si legge
        *aperta().write().unwrap() = None;
        assert!(leggi(&d.join("contatti")).is_err(), "cassaforte chiusa");
        assert!(apri(&d, b"sbagliata, proprio sbagliata").is_err());
        apri(&d, b"una passphrase lunga").unwrap();
        assert_eq!(leggi(&d.join("contatti")).unwrap(), b"mario abc.nyct\n");
        // un file alterato non si apre
        let mut alterato = std::fs::read(d.join("contatti")).unwrap();
        let n = alterato.len();
        alterato[n - 1] ^= 1;
        std::fs::write(d.join("alterato"), &alterato).unwrap();
        assert!(leggi(&d.join("alterato")).is_err());
        std::fs::remove_file(d.join("alterato")).unwrap();
        // cambio della passphrase: la vecchia non apre più
        imposta(&d, b"la passphrase nuova").unwrap();
        *aperta().write().unwrap() = None;
        assert!(apri(&d, b"una passphrase lunga").is_err());
        apri(&d, b"la passphrase nuova").unwrap();
        // togli: tutto torna in chiaro
        togli(&d).unwrap();
        assert_eq!(std::fs::read(d.join("contatti")).unwrap(), b"mario abc.nyct\n");
        assert_eq!(std::fs::read(d.join("cronologia").join("abc")).unwrap(), b"ciao Mario\ncome stai?\n");
        assert!(!esiste(&d));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn file_ricevuti_cifrati_a_pezzi_e_aperti_al_volo() {
        use std::io::{Seek, SeekFrom, Write};
        let _b = blocco();
        *aperta().write().unwrap() = None;
        let d = std::env::temp_dir().join(format!("nyct-cassaforte-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("allegati").join("mario")).unwrap();
        let vista = d.join("vista");
        std::fs::create_dir_all(&vista).unwrap();
        *super::vista().write().unwrap() = Some(vista.clone());
        // un file ricevuto PRIMA della cassaforte: in chiaro
        let vecchio = d.join("allegati").join("mario").join("vecchio.bin");
        let dati_vecchi: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&vecchio, &dati_vecchi).unwrap();
        // attivazione: il file vecchio viene sigillato
        imposta(&d, b"passphrase dei file").unwrap();
        assert!(e_sigillato(&vecchio), "all'attivazione i file ricevuti si sigillano");
        assert!(!std::fs::read(&vecchio).unwrap().windows(64).any(|w| w == &dati_vecchi[..64]), "niente in chiaro sul disco");
        assert_eq!(std::fs::read(in_chiaro(&vecchio).unwrap()).unwrap(), dati_vecchi, "aperto al volo: identico");
        // un allegato che arriva ora: scritto a pezzi, già sigillato
        let nuovo = d.join("allegati").join("mario").join("foto.png");
        let dati: Vec<u8> = (0..150_000u32).map(|i| (i % 241) as u8).collect();
        let mut w = Scrittore::crea(&nuovo, dati.len() as u64).unwrap();
        for pezzo in dati.chunks(7000) {
            w.scrivi(pezzo).unwrap();
        }
        w.chiudi().unwrap();
        assert!(e_sigillato(&nuovo));
        let s = Sigillo::da_file(&nuovo).unwrap();
        assert_eq!(std::fs::metadata(&nuovo).unwrap().len(), s.dimensione_file());
        let chiaro = in_chiaro(&nuovo).unwrap();
        assert!(chiaro.starts_with(&vista) && std::fs::read(&chiaro).unwrap() == dati, "nella vista, identico");
        // file grande: pezzi scritti in disordine (come il programma File), poi letti
        let grande = d.join("allegati").join("mario").join(".id.part");
        let tot: Vec<u8> = (0..700_000u32).map(|i| (i % 239) as u8).collect();
        let (sg, testa) = Sigillo::nuovo(256 * 1024, tot.len() as u64).unwrap();
        let mut f = std::fs::OpenOptions::new().create(true).truncate(true).read(true).write(true).open(&grande).unwrap();
        f.write_all(&testa).unwrap();
        f.set_len(sg.dimensione_file()).unwrap();
        for n in [2u64, 0, 1] {
            let (da, a) = ((n * sg.pezzo) as usize, ((n + 1) * sg.pezzo).min(tot.len() as u64) as usize);
            f.seek(SeekFrom::Start(sg.posizione(n))).unwrap();
            f.write_all(&sg.sigilla(n, &tot[da..a])).unwrap();
        }
        let sg2 = Sigillo::da_file(&grande).unwrap();
        let mut letto = Vec::new();
        for n in 0..sg2.npezzi() {
            letto.extend(sg2.leggi_pezzo(&mut f, n).unwrap());
        }
        assert_eq!(letto, tot, "pezzi in disordine: il file torna intero");
        // un pezzo spostato o alterato non si apre
        assert!(sg2.apri(1, &sg2.sigilla(0, b"abc")).is_none(), "un pezzo non vale al posto di un altro");
        let mut rovinato = std::fs::read(&nuovo).unwrap();
        let ultimo = rovinato.len() - 1;
        rovinato[ultimo] ^= 1;
        let r = d.join("allegati").join("mario").join("rovinato.png");
        std::fs::write(&r, &rovinato).unwrap();
        assert!(in_chiaro(&r).is_err(), "file alterato: non si apre");
        rovinato.truncate(rovinato.len() - 5000);
        std::fs::write(&r, &rovinato).unwrap();
        assert!(in_chiaro(&r).is_err(), "file troncato: non si apre");
        std::fs::remove_file(&r).unwrap();
        // cambio della passphrase: si riscrivono solo le intestazioni, i file si aprono ancora
        let prima = std::fs::read(&nuovo).unwrap();
        imposta(&d, b"passphrase nuova dei file").unwrap();
        let dopo = std::fs::read(&nuovo).unwrap();
        assert_eq!(&prima[TESTA_FILE as usize..], &dopo[TESTA_FILE as usize..], "i pezzi non si toccano");
        assert_ne!(&prima[..TESTA_FILE as usize], &dopo[..TESTA_FILE as usize], "l'intestazione sì");
        let _ = std::fs::remove_dir_all(&vista);
        std::fs::create_dir_all(&vista).unwrap();
        assert_eq!(std::fs::read(in_chiaro(&nuovo).unwrap()).unwrap(), dati);
        *aperta().write().unwrap() = None;
        assert!(in_chiaro(&nuovo).is_err(), "cassaforte chiusa: i file non si aprono");
        apri(&d, b"passphrase nuova dei file").unwrap();
        // togli: tutto torna in chiaro (i .part a metà non si toccano)
        togli(&d).unwrap();
        assert_eq!(std::fs::read(&nuovo).unwrap(), dati);
        assert_eq!(std::fs::read(&vecchio).unwrap(), dati_vecchi);
        *super::vista().write().unwrap() = None;
        let _ = std::fs::remove_dir_all(&d);
    }
}
