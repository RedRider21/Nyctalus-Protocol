// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Scelta dei nodi dei percorsi (PIANO-ANONIMATO, Tappa 2).
//!
//! - **Guardia**: il nodo d'entrata è scelto a caso **una volta** per rete e tenuto 30-60
//!   giorni (salvato su disco), come le guardie di Tor. Cambiare entrata a ogni circuito
//!   vorrebbe dire, prima o poi, passare da un'entrata malevola che vede il tuo IP.
//! - **Uscita e intermedi** a caso.
//! - **Regola di parentela**: in un percorso mai due nodi «parenti», cioè della stessa
//!   **famiglia** dichiarata dal gestore nell'elenco firmato, oppure con indirizzo pubblico
//!   nella stessa **sottorete** (/16 IPv4, /32 IPv6), che di solito vuol dire stesso
//!   fornitore. Gli indirizzi privati e locali (reti di casa, prove) non hanno sottorete.
//! - Se la regola non si può rispettare (es. tutti i nodi su un PC), il percorso si fa
//!   lo stesso con un **avviso**; con `NYCTALUS_PARENTELA=rigida` invece si rifiuta.
//! - **Ponti nascosti** (invisibilità, punto 1): se l'utente ne ha (`--bridge` o `bridge =`),
//!   si entra **sempre** da un ponte, mai da un nodo dell'elenco pubblico. Il ponte è un nodo
//!   che non è nell'elenco, con una sua chiave di camuffamento: chi osserva l'utente vede solo
//!   un indirizzo qualunque, e chi sonda il ponte senza quella chiave non riceve risposta.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::avvia_circuito::NodoRete;
use crate::Risultato;

/// Durata di una guardia: fra 30 e 60 giorni, a caso.
const GUARDIA_MIN_GIORNI: u64 = 30;
const GUARDIA_MAX_GIORNI: u64 = 60;
/// Guardie PRIMARIE ricordate per rete (come le «primary guards» di Tor): si usa sempre la
/// prima; la seguente solo se la prima non risponde. Mai nodi a caso al posto di una guardia
/// irraggiungibile: chi blocca l'entrata può spostare l'utente solo fra queste, sempre le stesse.
pub const GUARDIE_PRIMARIE: usize = 3;
/// Per quanto una guardia che non ha risposto resta scartata in questo processo (poi si riprova:
/// appena torna, si torna a lei).
const SCARTO_SECONDI: u64 = 10 * 60;
/// Tentativi di scelta casuale prima di dichiarare la regola impossibile.
const TENTATIVI: usize = 64;

#[derive(Default)]
struct Contesto {
    /// identità della rete (dalla chiave dell'autorità), per tenere una guardia per rete
    rete: String,
    /// famiglia dichiarata, per chiave X25519 del nodo
    famiglie: HashMap<[u8; 32], String>,
    /// guardia già scelta in questo processo (chiave X25519): l'ultima usata, per le stampe
    guardia: Option<[u8; 32]>,
    /// guardie primarie della rete (dal file, poi tenute qui): (rete, [(chiave, scadenza)])
    primarie: Option<(String, Vec<([u8; 32], u64)>)>,
    /// guardie che non hanno risposto (chiave → istante): scartate per SCARTO_SECONDI
    scartate: HashMap<[u8; 32], std::time::Instant>,
    /// avvisi già stampati (uno per tipo)
    avvisato: bool,
    /// solo per le prove: cartella delle guardie e modalità rigida senza toccare l'ambiente
    cartella_prova: Option<PathBuf>,
    rigida_prova: Option<bool>,
    /// ponti nascosti dell'utente: nodo e chiave di camuffamento per entrarci
    ponti: Vec<(NodoRete, [u8; 32])>,
    /// banda dichiarata nell'elenco firmato (Mbit/s), per chiave X25519
    bande: HashMap<[u8; 32], u32>,
    /// nodi solo intermedi (mai guardia, uscita, punto d'incontro)
    intermedi: std::collections::HashSet<[u8; 32]>,
    /// intermedi PROMOSSI a guardia dal gestore (stabili nel tempo): entrata sì, uscita mai
    guardie_promosse: std::collections::HashSet<[u8; 32]>,
    /// nodi SENZA USCITA verso Internet (`uscita=no`): guardia, intermedio e punto d'incontro sì
    senza_uscita: std::collections::HashSet<[u8; 32]>,
}

/// Imposta i nodi **solo intermedi** (dall'elenco firmato: `ruolo=intermedio`).
pub fn imposta_intermedi(nodi: Vec<[u8; 32]>) {
    contesto().lock().unwrap().intermedi = nodi.into_iter().collect();
}

/// Imposta i nodi **senza uscita** verso Internet (dall'elenco firmato: `uscita=no`).
pub fn imposta_senza_uscita(nodi: Vec<[u8; 32]>) {
    contesto().lock().unwrap().senza_uscita = nodi.into_iter().collect();
}

/// I nodi che possono fare da USCITA verso Internet: i pieni, tranne quelli «senza uscita».
/// Può essere vuoto: allora la rete serve solo chat, file e siti `.nyct`.
pub fn uscite(nodi: &[NodoRete]) -> Vec<NodoRete> {
    let senza = contesto().lock().unwrap().senza_uscita.clone();
    pieni(nodi).into_iter().filter(|n| !senza.contains(&n.1)).collect()
}

/// Imposta gli intermedi **promossi a guardia** (dall'elenco firmato: `guardia=si`).
pub fn imposta_guardie_promosse(nodi: Vec<[u8; 32]>) {
    contesto().lock().unwrap().guardie_promosse = nodi.into_iter().collect();
}

/// I candidati a guardia (nodo d'entrata): i nodi pieni più gli intermedi promossi dal gestore.
pub fn candidati_guardia(nodi: &[NodoRete]) -> Vec<NodoRete> {
    let promossi = contesto().lock().unwrap().guardie_promosse.clone();
    let mut v = pieni(nodi);
    let extra: Vec<NodoRete> = nodi.iter().copied().filter(|n| promossi.contains(&n.1) && !v.iter().any(|m| m.1 == n.1)).collect();
    v.extend(extra);
    v
}

/// I nodi che possono fare da guardia, uscita e punto d'incontro (tutti tranne i «solo intermedi»;
/// se fossero tutti intermedi, per non bloccare la rete, tutti).
pub fn pieni(nodi: &[NodoRete]) -> Vec<NodoRete> {
    let c = contesto().lock().unwrap();
    let v: Vec<NodoRete> = nodi.iter().copied().filter(|n| !c.intermedi.contains(&n.1)).collect();
    if v.is_empty() { nodi.to_vec() } else { v }
}

/// Imposta le bande dei nodi (dall'elenco firmato): i nodi più capaci si scelgono più spesso.
pub fn imposta_bande(bande: Vec<([u8; 32], u32)>) {
    contesto().lock().unwrap().bande = bande.into_iter().collect();
}

/// Peso di un nodo nella scelta: la sua banda; chi non la dichiara vale la mediana di chi la
/// dichiara (senza bande nell'elenco: tutti uguali, come prima). Come Tor, che pesa i relè per
/// banda misurata: scegliere a caso fra nodi di 5 e 50 Mbit è una lotteria (velocita.py: da 2,7 a
/// 9,3 Mbit/s a seconda di chi capita).
fn pesi(nodi: &[NodoRete]) -> Vec<f64> {
    let c = contesto().lock().unwrap();
    if c.bande.is_empty() {
        return vec![1.0; nodi.len()];
    }
    let mut note: Vec<u32> = c.bande.values().copied().collect();
    note.sort_unstable();
    let mediana = note[note.len() / 2] as f64;
    nodi.iter().map(|n| c.bande.get(&n.1).map(|b| *b as f64).unwrap_or(mediana).max(0.1)).collect()
}

fn casuale_01() -> f64 {
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).expect("generatore casuale di sistema");
    ((u64::from_le_bytes(b) >> 11) as f64 + 0.5) / (1u64 << 53) as f64
}

/// Indici di `nodi` in ordine casuale **pesato per banda** (Efraimidis-Spirakis: chiave u^(1/peso),
/// in ordine decrescente): chi ha più banda tende a venire prima.
fn ordine_pesato(nodi: &[NodoRete]) -> Vec<usize> {
    let p = pesi(nodi);
    let mut chiavi: Vec<(f64, usize)> = p.iter().enumerate().map(|(i, w)| (casuale_01().ln() / w, i)).collect();
    chiavi.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    chiavi.into_iter().map(|(_, i)| i).collect()
}

/// Imposta i ponti nascosti (vuoto = si entra dai nodi dell'elenco).
pub fn imposta_ponti(ponti: Vec<(NodoRete, [u8; 32])>) {
    let mut c = contesto().lock().unwrap();
    if c.ponti.iter().map(|p| p.0.1).ne(ponti.iter().map(|p| p.0.1)) {
        c.guardia = None;
        c.primarie = None;
        c.scartate.clear();
    }
    c.ponti = ponti;
}

/// I nodi-ponte impostati.
pub fn ponti() -> Vec<NodoRete> {
    contesto().lock().unwrap().ponti.iter().map(|p| p.0).collect()
}

/// Chiave di camuffamento per collegarsi al nodo d'entrata `n`: quella del ponte se `n` è un
/// ponte, altrimenti quella della rete.
pub fn chiave_entrata(n: &NodoRete, rete: Option<[u8; 32]>) -> Option<[u8; 32]> {
    contesto().lock().unwrap().ponti.iter().find(|p| p.0.1 == n.1).map(|p| p.1).or(rete)
}

fn contesto() -> &'static Mutex<Contesto> {
    static C: OnceLock<Mutex<Contesto>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(Contesto::default()))
}

/// Da chiamare al caricamento dell'elenco firmato: identità della rete e famiglie.
pub fn imposta_rete(autorita: &[u8; 32], famiglie: Vec<([u8; 32], String)>) {
    let rete: String = blake3::hash(autorita).as_bytes()[..8].iter().map(|b| format!("{b:02x}")).collect();
    let mut c = contesto().lock().unwrap();
    if c.rete != rete {
        c.guardia = None;
        c.primarie = None;
        c.scartate.clear();
    }
    c.rete = rete;
    c.famiglie = famiglie.into_iter().collect();
}

/// Modalità rigida: se la regola di parentela non si può rispettare, niente percorso.
fn rigida() -> bool {
    if let Some(r) = contesto().lock().unwrap().rigida_prova {
        return r;
    }
    std::env::var("NYCTALUS_PARENTELA").map(|v| v == "rigida" || v == "strict").unwrap_or(false)
}

/// Indirizzo pubblico (non locale, non privato, non CGNAT, non link-local).
pub fn pubblico(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => {
            let o = a.octets();
            !(a.is_loopback()
                || a.is_private()
                || a.is_link_local()
                || a.is_unspecified()
                || a.is_broadcast()
                || matches!(o[..3], [192, 0, 2] | [198, 51, 100] | [203, 0, 113])
                || (o[0] == 100 && (64..128).contains(&o[1])))
        }
        IpAddr::V6(a) => {
            let s = a.segments();
            !(a.is_loopback() || a.is_unspecified() || (s[0] & 0xfe00) == 0xfc00 || (s[0] & 0xffc0) == 0xfe80)
        }
    }
}

/// Sottorete «di fornitore» di un indirizzo pubblico: /16 IPv4, /32 IPv6.
fn sottorete(ip: IpAddr) -> Option<Vec<u8>> {
    if !pubblico(ip) {
        return None;
    }
    Some(match ip {
        IpAddr::V4(a) => a.octets()[..2].to_vec(),
        IpAddr::V6(a) => a.octets()[..4].to_vec(),
    })
}

/// Due nodi sono «parenti» se sono lo stesso nodo, se il gestore li ha messi nella stessa
/// famiglia, o se hanno indirizzi pubblici nella stessa sottorete.
pub fn parenti(a: &NodoRete, b: &NodoRete) -> bool {
    if a.1 == b.1 || a.0 == b.0 {
        return true;
    }
    let c = contesto().lock().unwrap();
    if let (Some(fa), Some(fb)) = (c.famiglie.get(&a.1), c.famiglie.get(&b.1)) {
        if fa == fb {
            return true;
        }
    }
    drop(c);
    matches!((sottorete(a.0.ip()), sottorete(b.0.ip())), (Some(x), Some(y)) if x == y)
}

fn casuale(n: usize) -> usize {
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).expect("generatore casuale di sistema");
    (u64::from_le_bytes(b) % n.max(1) as u64) as usize
}

/// Sceglie `quanti` nodi a caso fra `nodi`, diversi e non parenti fra loro né con `gia`.
/// `None` se non è possibile.
/// I `nodi` senza quelli appena scartati perché non rispondevano (spenti ma ancora nell'elenco),
/// se ne restano abbastanza per sceglierne `quanti` oltre a quelli in `gia`; altrimenti tutti.
fn senza_scartati(nodi: &[NodoRete], gia: &[NodoRete], quanti: usize) -> Vec<NodoRete> {
    let vivi: Vec<NodoRete> = nodi.iter().copied().filter(|n| !guardia_scartata(&n.1)).collect();
    let liberi = vivi.iter().filter(|n| gia.iter().all(|g| g.1 != n.1)).count();
    if liberi >= quanti { vivi } else { nodi.to_vec() }
}

pub fn scegli_non_parenti(nodi: &[NodoRete], gia: &[NodoRete], quanti: usize) -> Option<Vec<NodoRete>> {
    let vivi = senza_scartati(nodi, gia, quanti);
    let nodi = vivi.as_slice();
    for _ in 0..TENTATIVI {
        let indici = ordine_pesato(nodi);
        let mut scelti: Vec<NodoRete> = Vec::with_capacity(quanti);
        for &i in &indici {
            if scelti.len() == quanti {
                break;
            }
            let n = nodi[i];
            if gia.iter().chain(scelti.iter()).all(|x| !parenti(x, &n)) {
                scelti.push(n);
            }
        }
        if scelti.len() == quanti {
            return Some(scelti);
        }
    }
    None
}

/// Ripiego: `quanti` nodi a caso, solo diversi da quelli in `gia` (senza la regola).
fn scegli_diversi(nodi: &[NodoRete], gia: &[NodoRete], quanti: usize) -> Vec<NodoRete> {
    let vivi = senza_scartati(nodi, gia, quanti);
    let nodi = vivi.as_slice();
    let liberi: Vec<NodoRete> = nodi.iter().copied().filter(|n| gia.iter().all(|g| g.1 != n.1)).collect();
    ordine_pesato(&liberi).into_iter().take(quanti).map(|i| liberi[i]).collect()
}

/// Come `scegli_non_parenti`, ma se la regola è impossibile ripiega (con avviso) o, in
/// modalità rigida, dà errore. Il booleano dice se la regola è stata rispettata.
pub fn scegli(nodi: &[NodoRete], gia: &[NodoRete], quanti: usize) -> Risultato<(Vec<NodoRete>, bool)> {
    if let Some(s) = scegli_non_parenti(nodi, gia, quanti) {
        return Ok((s, true));
    }
    if rigida() {
        return Err("no path without related nodes (same family or provider subnet); NYCTALUS_PARENTELA=rigida".into());
    }
    avvisa();
    Ok((scegli_diversi(nodi, gia, quanti), false))
}

fn avvisa() {
    let mut c = contesto().lock().unwrap();
    if !c.avvisato {
        c.avvisato = true;
        eprintln!(
            "WARNING: the path uses RELATED nodes (same family or same provider subnet, e.g. all \
             nodes on one PC): traffic is encrypted but NOT anonymous. See PIANO-ANONIMATO.md."
        );
    }
}

// --- guardia persistente ------------------------------------------------------------------

fn cartella_guardie() -> Option<PathBuf> {
    if let Some(d) = contesto().lock().unwrap().cartella_prova.clone() {
        return Some(d);
    }
    // durante i test del motore MAI nella cartella personale dell'utente
    if cfg!(test) {
        return Some(std::env::temp_dir().join(format!("nyct-guardie-test-{}", std::process::id())));
    }
    if let Ok(d) = std::env::var("NYCTALUS_GUARDIE") {
        return Some(PathBuf::from(d));
    }
    let casa = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(PathBuf::from(casa).join(".nyctalus").join("guardie"))
}

fn adesso() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn esadecimale(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Guardie salvate per questa rete, nell'ordine: (chiave X25519, scadenza). Una per riga
/// (i file di prima, con una sola riga, si leggono uguali).
fn leggi_guardie(rete: &str) -> Vec<([u8; 32], u64)> {
    let Some(dir) = cartella_guardie() else { return Vec::new() };
    let Ok(testo) = std::fs::read_to_string(dir.join(rete)) else { return Vec::new() };
    testo.lines().filter_map(leggi_riga_guardia).take(GUARDIE_PRIMARIE).collect()
}

fn leggi_riga_guardia(riga: &str) -> Option<([u8; 32], u64)> {
    let mut campi = riga.split_whitespace();
    let chiave = campi.next()?;
    let scadenza: u64 = campi.next()?.parse().ok()?;
    if chiave.len() != 64 || !chiave.is_ascii() {
        return None;
    }
    let mut k = [0u8; 32];
    for (i, byte) in k.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&chiave[2 * i..2 * i + 2], 16).ok()?;
    }
    Some((k, scadenza))
}

/// La prima guardia salvata (per le prove).
#[cfg(test)]
fn leggi_guardia(rete: &str) -> Option<([u8; 32], u64)> {
    leggi_guardie(rete).into_iter().next()
}

/// Salva una sola guardia (per le prove).
#[cfg(test)]
fn salva_guardia(rete: &str, chiave: &[u8; 32], scadenza: u64) {
    contesto().lock().unwrap().primarie = None;
    salva_guardie(rete, &[(*chiave, scadenza)]);
}

fn salva_guardie(rete: &str, guardie: &[([u8; 32], u64)]) {
    let Some(dir) = cartella_guardie() else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    let testo: String = guardie.iter().map(|(k, s)| format!("{} {s}\n", esadecimale(k))).collect();
    let tmp = dir.join(format!(".{rete}.{}", std::process::id()));
    if std::fs::write(&tmp, testo).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        let _ = std::fs::rename(&tmp, dir.join(rete));
    }
}

fn durata_guardia() -> u64 {
    (GUARDIA_MIN_GIORNI + casuale((GUARDIA_MAX_GIORNI - GUARDIA_MIN_GIORNI + 1) as usize) as u64) * 86_400
}

/// Le guardie primarie della rete fra i `nodi` candidati, nell'ordine: quelle salvate ancora
/// valide (nell'elenco e non scadute), completate fino a [`GUARDIE_PRIMARIE`] con nodi nuovi
/// (scelti pesati per banda, possibilmente di famiglie diverse da quelle già in lista).
fn primarie(nodi: &[NodoRete], rete: &str) -> Vec<NodoRete> {
    let ora = adesso();
    let in_memoria = {
        let c = contesto().lock().unwrap();
        c.primarie.as_ref().filter(|(r, _)| r == rete).map(|(_, v)| v.clone())
    };
    let mut lista: Vec<([u8; 32], u64)> = in_memoria.unwrap_or_else(|| leggi_guardie(rete));
    let prima = lista.clone();
    lista.retain(|(k, scadenza)| *scadenza > ora && nodi.iter().any(|n| n.1 == *k));
    while lista.len() < GUARDIE_PRIMARIE.min(nodi.len()) {
        let liberi: Vec<NodoRete> = nodi.iter().copied().filter(|n| !lista.iter().any(|(k, _)| *k == n.1)).collect();
        if liberi.is_empty() {
            break;
        }
        // di famiglia diversa da quelle già in lista, se si può
        let famiglie: Vec<Option<String>> = lista.iter().map(|(k, _)| famiglia_di(k)).collect();
        let diversi: Vec<NodoRete> = liberi
            .iter()
            .copied()
            .filter(|n| famiglia_di(&n.1).is_none_or(|f| !famiglie.contains(&Some(f))))
            .collect();
        let pool = if diversi.is_empty() { liberi } else { diversi };
        let n = pool[ordine_pesato(&pool)[0]];
        lista.push((n.1, ora + durata_guardia()));
    }
    if lista != prima {
        salva_guardie(rete, &lista);
    }
    contesto().lock().unwrap().primarie = Some((rete.to_string(), lista.clone()));
    lista.iter().filter_map(|(k, _)| nodi.iter().find(|n| n.1 == *k).copied()).collect()
}

fn rete_e_candidati(nodi: &[NodoRete]) -> (Vec<NodoRete>, String) {
    let (rete, ponti) = {
        let c = contesto().lock().unwrap();
        let rete = if c.rete.is_empty() { "manuale".to_string() } else { c.rete.clone() };
        (rete, c.ponti.iter().map(|p| p.0).collect::<Vec<_>>())
    };
    // con i ponti le guardie sono i ponti (ricordati a parte), mai nodi dell'elenco
    if ponti.is_empty() {
        (candidati_guardia(nodi), rete)
    } else {
        (ponti, rete + "-ponte")
    }
}

/// La guardia (nodo d'entrata) per questa rete: la PRIMA delle guardie primarie che non ha
/// appena fallito; se hanno fallito tutte, comunque la prima (non si ripiega mai su nodi a caso).
/// Le primarie restano 30-60 giorni ciascuna (ricordate in `~/.nyctalus/guardie`).
pub fn guardia(nodi: &[NodoRete]) -> NodoRete {
    let (candidati, rete) = rete_e_candidati(nodi);
    let lista = primarie(&candidati, &rete);
    let mut c = contesto().lock().unwrap();
    let ora = std::time::Instant::now();
    c.scartate.retain(|_, quando| ora.duration_since(*quando).as_secs() < SCARTO_SECONDI);
    let scelta = lista
        .iter()
        .find(|n| !c.scartate.contains_key(&n.1))
        .or(lista.first())
        .copied()
        .unwrap_or_else(|| candidati[0]);
    c.guardia = Some(scelta.1);
    scelta
}

/// L'entrata `chiave` è fra quelle appena scartate perché non rispondevano?
pub fn guardia_scartata(chiave: &[u8; 32]) -> bool {
    let c = contesto().lock().unwrap();
    c.scartate.get(chiave).is_some_and(|quando| quando.elapsed().as_secs() < SCARTO_SECONDI)
}

/// L'entrata `chiave` non ha risposto: per un po' si usa la guardia primaria seguente. Vero se
/// ne resta almeno un'altra da provare.
pub fn segna_guardia_irraggiungibile(chiave: &[u8; 32]) -> bool {
    let mut c = contesto().lock().unwrap();
    let Some((_, lista)) = c.primarie.clone() else { return false };
    if !lista.iter().any(|(k, _)| k == chiave) {
        return false;
    }
    c.scartate.insert(*chiave, std::time::Instant::now());
    if c.guardia == Some(*chiave) {
        c.guardia = None;
    }
    lista.iter().any(|(k, _)| !c.scartate.contains_key(k))
}

/// Percorso di `salti` nodi che finisce in `ultimo` (punto d'incontro): la guardia come
/// entrata (se non è parente di `ultimo`, altrimenti un'altra entrata per questo percorso),
/// poi intermedi a caso, tutti non parenti. Con pochi nodi il percorso si accorcia.
pub fn percorso_verso(nodi: &[NodoRete], ultimo: NodoRete, salti: usize) -> Risultato<Vec<NodoRete>> {
    let prima = salti.saturating_sub(1).min(nodi.len().saturating_sub(1));
    if prima == 0 {
        return Ok(vec![ultimo]);
    }
    let g = guardia(nodi);
    let ponti = ponti();
    // se la guardia è parente del punto d'incontro si entra da un'altra (fra i ponti, se ci sono)
    // (mai un'entrata appena scartata perché non rispondeva, se ce ne sono altre)
    let tutti: Vec<NodoRete> = if ponti.is_empty() { candidati_guardia(nodi) } else { ponti };
    let vivi: Vec<NodoRete> = tutti.iter().copied().filter(|n| !guardia_scartata(&n.1)).collect();
    let candidati: &[NodoRete] = if vivi.len() > 1 { &vivi } else { &tutti };
    let entrata = if parenti(&g, &ultimo) {
        scegli(candidati, &[ultimo], 1)?.0.into_iter().next().or(Some(g))
    } else {
        Some(g)
    };
    let Some(entrata) = entrata else { return Ok(vec![ultimo]) };
    let (intermedi, _) = scegli(nodi, &[entrata, ultimo], prima - 1)?;
    let mut p = vec![entrata];
    p.extend(intermedi);
    p.push(ultimo);
    Ok(p)
}

// --- simulazione (PIANO-ANONIMATO, punto K): quanto spesso un avversario sta ai due capi ---

/// La famiglia dichiarata di un nodo (dall'elenco firmato), se c'è.
pub fn famiglia_di(chiave: &[u8; 32]) -> Option<String> {
    contesto().lock().unwrap().famiglie.get(chiave).cloned()
}

/// Come si sceglievano / si scelgono entrata e uscita.
#[derive(Clone, Copy, Debug)]
pub enum Strategia {
    /// prima della Tappa 2: entrata = 1° nodo dell'elenco, uscita = 2°, per tutti
    Vecchia,
    /// entrata e uscita a caso a ogni circuito, senza guardia né regola di parentela
    SenzaGuardia,
    /// adesso: guardia persistente 30-60 giorni, uscita a caso, regola di parentela
    Attuale,
}

/// Risultato di una simulazione.
#[derive(Debug, Default)]
pub struct EsitoSimulazione {
    pub circuiti: u64,
    /// circuiti con l'avversario sia all'entrata sia all'uscita (può collegare chi con chi)
    pub compromessi: u64,
    pub utenti: usize,
    /// utenti con almeno un circuito compromesso nel periodo
    pub utenti_colpiti: usize,
    /// utenti che almeno una volta sono entrati da un nodo dell'avversario (ne vede l'IP)
    pub utenti_entrata_vista: usize,
}

/// Simula `utenti` che per `giorni` aprono `circuiti_al_giorno` circuiti ciascuno, con la
/// strategia data, e conta quando l'avversario (i nodi per cui `avversario` è vero) sta ai due
/// capi. Usa le stesse funzioni di scelta del motore vero (`scegli`, regola di parentela).
pub fn simula(
    nodi: &[NodoRete],
    avversario: &dyn Fn(&NodoRete) -> bool,
    strategia: Strategia,
    utenti: usize,
    giorni: u64,
    circuiti_al_giorno: u64,
) -> Risultato<EsitoSimulazione> {
    if nodi.len() < 2 {
        return Err("servono almeno 2 nodi".into());
    }
    let mut e = EsitoSimulazione { utenti, ..Default::default() };
    for _ in 0..utenti {
        let (mut colpito, mut visto) = (false, false);
        let mut guardia: Option<(NodoRete, u64)> = None; // (nodo, ultimo giorno)
        for giorno in 0..giorni {
            if matches!(strategia, Strategia::Attuale) && guardia.is_none_or(|(_, fine)| giorno > fine) {
                let durata = GUARDIA_MIN_GIORNI + casuale((GUARDIA_MAX_GIORNI - GUARDIA_MIN_GIORNI + 1) as usize) as u64;
                guardia = Some((nodi[casuale(nodi.len())], giorno + durata - 1));
            }
            for _ in 0..circuiti_al_giorno {
                let (entrata, uscita) = match strategia {
                    Strategia::Vecchia => (nodi[0], nodi[1]),
                    Strategia::SenzaGuardia => {
                        let a = nodi[casuale(nodi.len())];
                        let altri: Vec<NodoRete> = nodi.iter().copied().filter(|n| n.1 != a.1).collect();
                        (a, altri[casuale(altri.len())])
                    }
                    Strategia::Attuale => {
                        let g = guardia.expect("guardia scelta").0;
                        let u = scegli_non_parenti(nodi, &[g], 1)
                            .or_else(|| Some(scegli_diversi(nodi, &[g], 1)))
                            .and_then(|v| v.first().copied())
                            .ok_or("nessuna uscita possibile")?;
                        (g, u)
                    }
                };
                e.circuiti += 1;
                visto |= avversario(&entrata);
                if avversario(&entrata) && avversario(&uscita) {
                    e.compromessi += 1;
                    colpito = true;
                }
            }
        }
        e.utenti_colpiti += usize::from(colpito);
        e.utenti_entrata_vista += usize::from(visto);
    }
    Ok(e)
}

#[cfg(test)]
pub(crate) mod test {
    use super::*;
    use std::net::SocketAddr;

    fn nodo(ip: &str, k: u8) -> NodoRete {
        (format!("{ip}:4600").parse::<SocketAddr>().unwrap(), [k; 32], None)
    }

    // le prove condividono il contesto globale: si eseguono una alla volta
    /// Le prove che toccano la guardia (anche di altri moduli) passano una alla volta: il
    /// contesto e il file della guardia sono condivisi.
    pub(crate) fn blocco() -> std::sync::MutexGuard<'static, ()> {
        static B: Mutex<()> = Mutex::new(());
        B.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn reimposta(famiglie: Vec<([u8; 32], String)>, cartella: &std::path::Path) {
        imposta_rete(&[7; 32], famiglie);
        let mut c = contesto().lock().unwrap();
        c.guardia = None;
        c.primarie = None;
        c.scartate.clear();
        c.avvisato = false;
        c.cartella_prova = Some(cartella.to_path_buf());
        c.rigida_prova = Some(false);
        c.ponti.clear();
        c.bande.clear();
        c.intermedi.clear();
        c.guardie_promosse.clear();
        c.senza_uscita.clear();
    }

    fn cartella(nome: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("nyct-guardie-{nome}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn intermedi_promossi_fanno_da_guardia_mai_da_uscita() {
        let _b = blocco();
        let d = cartella("promossi");
        reimposta(vec![], &d);
        let nodi = vec![nodo("81.10.0.1", 1), nodo("95.20.0.1", 2), nodo("62.30.0.1", 3), nodo("70.40.0.1", 4)];
        // 3 e 4 aiutano la rete (solo intermedi); 3 è stabile e il gestore l'ha promosso a guardia
        imposta_intermedi(vec![[3; 32], [4; 32]]);
        imposta_guardie_promosse(vec![[3; 32]]);
        let p: Vec<u8> = pieni(&nodi).iter().map(|n| n.1[0]).collect();
        assert_eq!(p, vec![1, 2], "uscite e punti d'incontro: solo i nodi pieni");
        let mut g: Vec<u8> = candidati_guardia(&nodi).iter().map(|n| n.1[0]).collect();
        g.sort();
        assert_eq!(g, vec![1, 2, 3], "guardia: i pieni più il promosso, mai l'intermedio non promosso");
        // scegliendo molte volte la guardia (nuova ogni volta) il promosso esce, il 4 mai
        let mut visti = std::collections::HashSet::new();
        for i in 0..200 {
            let _ = std::fs::remove_dir_all(&d);
            reimposta(vec![], &cartella(&format!("promossi{}", i % 2)));
            imposta_intermedi(vec![[3; 32], [4; 32]]);
            imposta_guardie_promosse(vec![[3; 32]]);
            let _ = std::fs::remove_dir_all(cartella(&format!("promossi{}", i % 2)));
            visti.insert(guardia(&nodi).1[0]);
        }
        assert!(visti.contains(&3), "il promosso viene scelto come guardia");
        assert!(!visti.contains(&4), "l'intermedio non promosso non è mai guardia");
        imposta_intermedi(vec![]);
        imposta_guardie_promosse(vec![]);
    }

    #[test]
    fn parentela_per_sottorete_e_famiglia() {
        let _b = blocco();
        reimposta(vec![([5; 32], "mario".into()), ([6; 32], "mario".into())], &cartella("p"));
        assert!(parenti(&nodo("203.0.113.1", 1), &nodo("203.0.200.9", 2)) == false); // documentazione = non pubblico
        assert!(parenti(&nodo("81.10.1.1", 1), &nodo("81.10.200.9", 2))); // stessa /16 pubblica
        assert!(!parenti(&nodo("81.10.1.1", 1), &nodo("81.11.1.1", 2)));
        assert!(!parenti(&nodo("192.168.1.2", 1), &nodo("192.168.1.3", 2))); // privati: niente sottorete
        assert!(parenti(&nodo("81.10.1.1", 5), &nodo("95.1.1.1", 6))); // stessa famiglia
        assert!(parenti(&nodo("81.10.1.1", 1), &nodo("95.1.1.1", 1))); // stessa chiave = stesso nodo
    }

    #[test]
    fn mai_due_parenti_nello_stesso_percorso() {
        let _b = blocco();
        reimposta(vec![], &cartella("r"));
        // 3 coppie di nodi nella stessa /16: un percorso di 3 deve prenderne uno per coppia
        let nodi = vec![
            nodo("81.10.0.1", 1), nodo("81.10.0.2", 2),
            nodo("95.20.0.1", 3), nodo("95.20.0.2", 4),
            nodo("62.30.0.1", 5), nodo("62.30.0.2", 6),
        ];
        for _ in 0..300 {
            let p = scegli_non_parenti(&nodi, &[], 3).expect("possibile");
            let reti: std::collections::HashSet<_> = p.iter().map(|n| n.0.ip().to_string()[..5].to_string()).collect();
            assert_eq!(reti.len(), 3, "percorso con parenti: {p:?}");
        }
        assert!(scegli_non_parenti(&nodi, &[], 4).is_none()); // 4 senza parenti: impossibile
    }

    #[test]
    fn impossibile_ripiega_con_avviso_o_rifiuta_se_rigida() {
        let _b = blocco();
        reimposta(vec![], &cartella("i"));
        let nodi = vec![nodo("81.10.0.1", 1), nodo("81.10.0.2", 2), nodo("81.10.0.3", 3)];
        let (p, rispettata) = scegli(&nodi, &[], 3).unwrap();
        assert_eq!(p.len(), 3);
        assert!(!rispettata);
        contesto().lock().unwrap().rigida_prova = Some(true);
        assert!(scegli(&nodi, &[], 3).is_err());
        contesto().lock().unwrap().rigida_prova = Some(false);
    }

    #[test]
    fn scelte_casuali_non_sempre_le_stesse() {
        let _b = blocco();
        reimposta(vec![], &cartella("c"));
        let nodi: Vec<NodoRete> = (1..=6).map(|i| nodo(&format!("{}.1.0.1", 40 + i), i)).collect();
        let mut viste = std::collections::HashSet::new();
        for _ in 0..400 {
            viste.insert(scegli_non_parenti(&nodi, &[nodi[0]], 1).unwrap()[0].1);
        }
        assert_eq!(viste.len(), 5, "ogni nodo idoneo deve poter uscire"); // tutti tranne il già scelto
    }

    #[test]
    fn guardia_ricordata_e_rinnovata() {
        let _b = blocco();
        let dir = cartella("g");
        reimposta(vec![], &dir);
        let nodi: Vec<NodoRete> = (1..=5).map(|i| nodo(&format!("{}.1.0.1", 40 + i), i)).collect();
        let g = guardia(&nodi);
        // un altro processo (contesto vuoto) ritrova la stessa guardia dal file
        contesto().lock().unwrap().guardia = None;
        assert_eq!(guardia(&nodi).1, g.1);
        let rete = contesto().lock().unwrap().rete.clone();
        let (_, scadenza) = leggi_guardia(&rete).unwrap();
        let giorni = (scadenza - adesso() + 60) / 86_400;
        assert!((GUARDIA_MIN_GIORNI..=GUARDIA_MAX_GIORNI).contains(&giorni), "durata {giorni} giorni");
        // guardia tolta dall'elenco: se ne sceglie un'altra
        contesto().lock().unwrap().guardia = None;
        let senza: Vec<NodoRete> = nodi.iter().copied().filter(|n| n.1 != g.1).collect();
        assert_ne!(guardia(&senza).1, g.1);
        // scaduta: se ne sceglie una nuova (non necessariamente diversa, ma il file si rinnova)
        salva_guardia(&rete, &nodi[0].1, adesso() - 1);
        contesto().lock().unwrap().guardia = None;
        let _ = guardia(&nodi);
        assert!(leggi_guardia(&rete).unwrap().1 > adesso());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn guardie_primarie_in_ordine_e_di_riserva() {
        let _b = blocco();
        let dir = cartella("gp");
        reimposta(vec![], &dir);
        let nodi: Vec<NodoRete> = (1..=6).map(|i| nodo(&format!("{}.1.0.1", 40 + i), i)).collect();
        let prima = guardia(&nodi);
        let rete = contesto().lock().unwrap().rete.clone();
        let salvate = leggi_guardie(&rete);
        assert_eq!(salvate.len(), GUARDIE_PRIMARIE, "si ricordano {GUARDIE_PRIMARIE} guardie primarie");
        assert_eq!(salvate[0].0, prima.1, "si usa la prima");
        // un altro processo ritrova le stesse, nello stesso ordine
        contesto().lock().unwrap().primarie = None;
        assert_eq!(guardia(&nodi).1, prima.1);
        assert_eq!(leggi_guardie(&rete), salvate);
        // la prima non risponde → la seconda (mai un nodo fuori dalle primarie)
        assert!(segna_guardia_irraggiungibile(&prima.1), "restano altre primarie");
        assert_eq!(guardia(&nodi).1, salvate[1].0);
        for _ in 0..20 {
            assert_eq!(percorso_verso(&nodi, nodi[5], 3).unwrap()[0].1 == salvate[1].0 || nodi[5].1 == salvate[1].0, true);
        }
        // fallisce anche la seconda → la terza; poi tutte → di nuovo la prima, mai un'altra
        assert!(segna_guardia_irraggiungibile(&salvate[1].0));
        assert_eq!(guardia(&nodi).1, salvate[2].0);
        assert!(!segna_guardia_irraggiungibile(&salvate[2].0), "finite le primarie");
        assert_eq!(guardia(&nodi).1, prima.1);
        // un nodo che non è fra le primarie non si segna
        let estranea = nodi.iter().find(|n| !salvate.iter().any(|(k, _)| *k == n.1)).unwrap();
        assert!(!segna_guardia_irraggiungibile(&estranea.1));
        // passato lo scarto si torna alla prima
        {
            let mut c = contesto().lock().unwrap();
            let vecchio = std::time::Instant::now() - std::time::Duration::from_secs(SCARTO_SECONDI + 1);
            for v in c.scartate.values_mut() {
                *v = vecchio;
            }
        }
        assert_eq!(guardia(&nodi).1, prima.1);
        // il file d'una volta (una sola guardia) si legge e si completa tenendola per prima
        salva_guardie(&rete, &[(nodi[4].1, adesso() + 86_400 * 40)]);
        contesto().lock().unwrap().primarie = None;
        assert_eq!(guardia(&nodi).1, nodi[4].1);
        let nuove = leggi_guardie(&rete);
        assert_eq!(nuove.len(), GUARDIE_PRIMARIE);
        assert_eq!(nuove[0].0, nodi[4].1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn guardie_primarie_di_famiglie_diverse() {
        let _b = blocco();
        let dir = cartella("gf");
        let nodi: Vec<NodoRete> = (1..=6).map(|i| nodo(&format!("{}.1.0.1", 40 + i), i)).collect();
        // tre famiglie da due nodi: le tre primarie devono essere una per famiglia
        let fam: Vec<([u8; 32], String)> = nodi.iter().enumerate().map(|(i, n)| (n.1, format!("f{}", i / 2))).collect();
        for _ in 0..10 {
            reimposta(fam.clone(), &dir);
            let _ = std::fs::remove_dir_all(&dir);
            let _ = guardia(&nodi);
            let rete = contesto().lock().unwrap().rete.clone();
            let f: std::collections::HashSet<_> = leggi_guardie(&rete).iter().map(|(k, _)| famiglia_di(k)).collect();
            assert_eq!(f.len(), GUARDIE_PRIMARIE, "una guardia per famiglia");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nodi_senza_uscita_mai_scelti_come_uscita() {
        let _b = blocco();
        reimposta(vec![], &cartella("su"));
        let nodi: Vec<NodoRete> = (1..=5).map(|i| nodo(&format!("{}.1.0.1", 40 + i), i)).collect();
        imposta_senza_uscita(vec![nodi[0].1, nodi[1].1, nodi[2].1]);
        let u = uscite(&nodi);
        assert_eq!(u.len(), 2, "uscite: solo i nodi senza «uscita=no»");
        assert!(u.iter().all(|n| n.1 != nodi[0].1 && n.1 != nodi[1].1 && n.1 != nodi[2].1));
        // possono comunque fare da guardia e da punto d'incontro (chat, siti .nyct)
        assert_eq!(pieni(&nodi).len(), 5);
        assert!(candidati_guardia(&nodi).iter().any(|n| n.1 == nodi[0].1));
        // tutti senza uscita: nessuna uscita (la rete serve solo i siti .nyct)
        imposta_senza_uscita(nodi.iter().map(|n| n.1).collect());
        assert!(uscite(&nodi).is_empty());
        imposta_senza_uscita(vec![]);
    }

    #[test]
    fn con_i_ponti_si_entra_sempre_da_un_ponte() {
        let _b = blocco();
        reimposta(vec![], &cartella("p"));
        let nodi: Vec<NodoRete> = (1..=5).map(|i| nodo(&format!("{}.1.0.1", 40 + i), i)).collect();
        let ponte = nodo("60.1.0.1", 99);
        imposta_ponti(vec![(ponte, [5; 32])]);
        assert_eq!(guardia(&nodi).1, ponte.1, "la guardia è il ponte, non un nodo dell'elenco");
        for _ in 0..30 {
            let p = percorso_verso(&nodi, nodi[2], 3).unwrap();
            assert_eq!(p[0].1, ponte.1);
            assert_eq!(p.len(), 3);
        }
        // chiave d'entrata: quella del ponte per il ponte, quella della rete per gli altri
        assert_eq!(chiave_entrata(&ponte, Some([1; 32])), Some([5; 32]));
        assert_eq!(chiave_entrata(&nodi[0], Some([1; 32])), Some([1; 32]));
        imposta_ponti(vec![]);
        assert_ne!(guardia(&nodi).1, ponte.1, "senza ponti si torna ai nodi dell'elenco");
    }

    #[test]
    fn nodi_piu_capaci_scelti_piu_spesso() {
        let _b = blocco();
        reimposta(vec![], &cartella("b"));
        let nodi: Vec<NodoRete> = (1..=4).map(|i| nodo(&format!("{}.1.0.1", 40 + i), i)).collect();
        imposta_bande(vec![(nodi[0].1, 100), (nodi[1].1, 10), (nodi[2].1, 10), (nodi[3].1, 10)]);
        let mut primo = 0;
        for _ in 0..2000 {
            if nodi[ordine_pesato(&nodi)[0]].1 == nodi[0].1 {
                primo += 1;
            }
        }
        // atteso 100/130 ≈ 77%
        assert!((1400..1700).contains(&primo), "il nodo da 100 Mbit scelto per primo {primo} volte su 2000");
        imposta_bande(vec![]);
        let mut primo = 0;
        for _ in 0..2000 {
            if nodi[ordine_pesato(&nodi)[0]].1 == nodi[0].1 {
                primo += 1;
            }
        }
        assert!((380..620).contains(&primo), "senza bande tutti uguali: {primo} su 2000");
    }

    #[test]
    fn percorso_verso_il_punto_d_incontro() {
        let _b = blocco();
        reimposta(vec![], &cartella("v"));
        let nodi: Vec<NodoRete> = (1..=5).map(|i| nodo(&format!("{}.1.0.1", 40 + i), i)).collect();
        let g = guardia(&nodi);
        let ultimo = *nodi.iter().find(|n| n.1 != g.1).unwrap();
        for _ in 0..50 {
            let p = percorso_verso(&nodi, ultimo, 3).unwrap();
            assert_eq!(p.len(), 3);
            assert_eq!(p[0].1, g.1, "si entra sempre dalla guardia");
            assert_eq!(p[2].1, ultimo.1);
            assert!(!parenti(&p[0], &p[1]) && !parenti(&p[1], &p[2]));
        }
        // se il punto d'incontro è la guardia, per quel percorso si entra da un altro nodo
        let p = percorso_verso(&nodi, g, 3).unwrap();
        assert_ne!(p[0].1, g.1);
        assert_eq!(p[2].1, g.1);
    }

    #[test]
    fn simulazione_un_gestore_solo_non_sta_mai_ai_due_capi() {
        let _b = blocco();
        // 4 gestori × 3 nodi; l'avversario è UN gestore (un quarto dei nodi)
        let mut fam = vec![];
        let mut nodi = vec![];
        for g in 0..4u8 {
            for n in 0..3u8 {
                let k = 10 * g + n + 1;
                nodi.push(nodo(&format!("{}.{}.0.1", 40 + g, n + 1), k));
                fam.push(([k; 32], format!("g{g}")));
            }
        }
        reimposta(fam, &cartella("s"));
        let avv = |n: &NodoRete| famiglia_di(&n.1).as_deref() == Some("g1");
        let att = simula(&nodi, &avv, Strategia::Attuale, 200, 90, 10).unwrap();
        assert_eq!(att.compromessi, 0, "con le famiglie un solo gestore non può stare ai due capi");
        let senza = simula(&nodi, &avv, Strategia::SenzaGuardia, 200, 90, 10).unwrap();
        assert!(senza.compromessi > 0 && senza.utenti_colpiti > 150, "senza regole sì, e colpisce quasi tutti: {senza:?}");
        assert!(att.utenti_entrata_vista < senza.utenti_entrata_vista, "con la guardia molti meno utenti mostrano l'IP all'avversario");
    }
}
