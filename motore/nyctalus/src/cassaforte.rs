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
    scrivi_atomico(&file_cassaforte(dati), descrittore.as_bytes())?;
    Ok(file.len())
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
    *aperta().write().unwrap() = None;
    for (f, c) in file.iter().zip(contenuti) {
        scrivi_atomico(f, &c)?;
    }
    std::fs::remove_file(file_cassaforte(dati))?;
    Ok(file.len())
}

/// I file della cartella dei dati da proteggere (tutti, ricorsivamente, tranne il descrittore e i
/// collegamenti/segnali che non contengono dati personali).
fn file_da_proteggere(dati: &Path) -> Vec<PathBuf> {
    fn giro(d: &Path, v: &mut Vec<PathBuf>) {
        let Ok(e) = std::fs::read_dir(d) else { return };
        for x in e.flatten() {
            let p = x.path();
            let nome = x.file_name().to_string_lossy().to_string();
            if nome == FILE_CASSAFORTE || nome == "controllo" || nome == "finestre" || nome.ends_with(".tmp") || nome.starts_with(".") {
                continue;
            }
            match x.file_type() {
                Ok(t) if t.is_dir() => giro(&p, v),
                Ok(t) if t.is_file() => v.push(p),
                _ => {}
            }
        }
    }
    let mut v = Vec::new();
    giro(dati, &mut v);
    v
}

/// Legge un file dei dati: lo decifra se è cifrato (serve la cassaforte aperta), altrimenti lo
/// restituisce com'è (file di prima della cassaforte).
pub fn leggi(f: &Path) -> Risultato<Vec<u8>> {
    let d = std::fs::read(f)?;
    if !d.starts_with(MAGIA) {
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
        assert_eq!(leggi(&d.join("cronologia").join("abc")).unwrap(), b"ciao Mario");
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
        assert!(!esiste(&d));
        let _ = std::fs::remove_dir_all(&d);
    }
}
