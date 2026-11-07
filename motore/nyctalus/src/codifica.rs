// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! **Ragnatela codificata** (velocità e stealth): le celle di un gruppo conflux vengono spedite a
//! **gruppi di k**, seguite da **r celle di riserva**; chi riceve ricompone il gruppo con
//! **qualunque k** delle k+r. Così:
//!
//! - non si aspetta la strada lenta: le riserve viaggiano sulle strade più libere e coprono le
//!   celle rimaste indietro;
//! - nessuna strada porta il flusso «com'è»: una parte delle celle è miscelata;
//! - **rumore utile**: il rumore di fondo dell'uscita, invece di celle finte buttate via, manda
//!   riserve in più dell'ultimo gruppo.
//!
//! Il codice è **sistematico** (le k celle originali partono subito, senza ritardo) e **MDS**: le
//! riserve usano una matrice di **Cauchy** su GF(2^8), in cui ogni sottomatrice quadrata è
//! invertibile, quindi bastano davvero k celle qualsiasi.
//!
//! Intestazione di 8 byte (al posto del numero di sequenza conflux, stessa lunghezza dell'unità):
//! bit 63 = 1 (unità codificata), bit 62-24 = gruppo, bit 23-16 = m (celle vere del gruppo, solo
//! nelle riserve; 0 nelle originali), bit 15-0 = indice (originale: 0..k; riserva: 0x8000 | j).

use std::collections::BTreeMap;

/// Contrassegno nel riempimento del quadro LINK: il client chiede la ragnatela codificata.
pub const MARCA: &[u8; 8] = b"NYCTCOD1";
/// Bit delle unità codificate nell'intestazione.
pub const CODIFICATA: u64 = 1 << 63;
const RISERVA: u16 = 0x8000;
/// Gruppi tenuti in attesa al massimo (anti-DoS).
const GRUPPI_MAX: usize = 1024;
/// Limiti dei parametri.
pub const K_MAX: u8 = 64;
pub const R_MAX: u8 = 32;

// --- aritmetica in GF(2^8), polinomio 0x11d ---------------------------------------------------

struct Tabelle {
    exp: [u8; 512],
    log: [u8; 256],
}

fn tabelle() -> &'static Tabelle {
    static T: std::sync::OnceLock<Tabelle> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut t = Tabelle { exp: [0; 512], log: [0; 256] };
        let mut x: u16 = 1;
        for i in 0..255 {
            t.exp[i] = x as u8;
            t.log[x as usize] = i as u8;
            x <<= 1;
            if x & 0x100 != 0 {
                x ^= 0x11d;
            }
        }
        for i in 255..512 {
            t.exp[i] = t.exp[i - 255];
        }
        t
    })
}

fn mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    let t = tabelle();
    t.exp[t.log[a as usize] as usize + t.log[b as usize] as usize]
}

fn inv(a: u8) -> u8 {
    let t = tabelle();
    t.exp[255 - t.log[a as usize] as usize]
}

/// `dst ^= c · src` (byte per byte in GF(2^8)).
fn somma_per(dst: &mut [u8], src: &[u8], c: u8) {
    if c == 0 {
        return;
    }
    if c == 1 {
        for (d, s) in dst.iter_mut().zip(src) {
            *d ^= s;
        }
        return;
    }
    let t = tabelle();
    let lc = t.log[c as usize] as usize;
    for (d, s) in dst.iter_mut().zip(src) {
        if *s != 0 {
            *d ^= t.exp[lc + t.log[*s as usize] as usize];
        }
    }
}

/// Coefficiente della riserva j sulla cella i: Cauchy 1/(x_j + y_i), x_j = 128 + j, y_i = i.
fn coeff(j: u8, i: u8) -> u8 {
    inv((128u8 + j) ^ i)
}

/// Intestazione di un'unità codificata.
pub fn intestazione(gruppo: u64, m: u8, indice: u16) -> u64 {
    CODIFICATA | ((gruppo & ((1 << 39) - 1)) << 24) | ((m as u64) << 16) | indice as u64
}

fn leggi_intestazione(h: u64) -> (u64, u8, u16) {
    ((h >> 24) & ((1 << 39) - 1), (h >> 16) as u8, h as u16)
}

/// Il riempimento del LINK con la richiesta (k, r).
pub fn richiesta(k: u8, r: u8) -> [u8; 10] {
    let mut v = [0u8; 10];
    v[..8].copy_from_slice(MARCA);
    v[8] = k;
    v[9] = r;
    v
}

/// La ragnatela codificata chiesta da questo client: `NYCTALUS_CODIFICA=k,r` (es. 8,2), `0` spenta.
pub fn richiesta_configurata() -> Option<(u8, u8)> {
    let v = std::env::var("NYCTALUS_CODIFICA").ok()?;
    let (k, r) = v.split_once(',')?;
    let (k, r) = (k.trim().parse::<u8>().ok()?, r.trim().parse::<u8>().ok()?);
    ((1..=K_MAX).contains(&k) && (1..=R_MAX).contains(&r)).then_some((k, r))
}

/// Dal LINK (dopo i 16 byte del set_id): la richiesta (k, r), se c'è ed è valida.
pub fn leggi_richiesta(riempimento: &[u8]) -> Option<(u8, u8)> {
    if riempimento.len() < 10 || &riempimento[..8] != MARCA {
        return None;
    }
    let (k, r) = (riempimento[8], riempimento[9]);
    ((1..=K_MAX).contains(&k) && (1..=R_MAX).contains(&r)).then_some((k, r))
}

// --- chi spedisce -----------------------------------------------------------------------------

/// Codificatore: riceve celle e restituisce le unità da spedire (intestazione + cella).
pub struct Codificatore {
    k: u8,
    r: u8,
    gruppo: u64,
    celle: Vec<Vec<u8>>,
    /// l'ultimo gruppo chiuso (per le riserve in più del «rumore utile»)
    ultimo: Option<(u64, Vec<Vec<u8>>)>,
    riserve_extra: u8,
}

impl Codificatore {
    pub fn nuovo(k: u8, r: u8) -> Self {
        Self { k, r, gruppo: 0, celle: Vec::new(), ultimo: None, riserve_extra: 0 }
    }

    /// Una cella da spedire: l'originale subito; a gruppo pieno anche le r riserve.
    pub fn cella(&mut self, cella: Vec<u8>) -> Vec<(u64, Vec<u8>)> {
        let i = self.celle.len() as u16;
        let mut out = vec![(intestazione(self.gruppo, 0, i), cella.clone())];
        self.celle.push(cella);
        if self.celle.len() == self.k as usize {
            out.extend(self.chiudi());
        }
        out
    }

    /// C'è un gruppo incompleto in corso?
    pub fn in_corso(&self) -> bool {
        !self.celle.is_empty()
    }

    /// Chiude il gruppo in corso (anche incompleto, dopo una pausa): le sue r riserve.
    pub fn chiudi(&mut self) -> Vec<(u64, Vec<u8>)> {
        if self.celle.is_empty() {
            return Vec::new();
        }
        let celle = std::mem::take(&mut self.celle);
        let out = (0..self.r).map(|j| self.riserva(self.gruppo, &celle, j)).collect();
        self.ultimo = Some((self.gruppo, celle));
        self.riserve_extra = 0;
        self.gruppo += 1;
        out
    }

    /// «Rumore utile»: una riserva in più dell'ultimo gruppo chiuso (al posto di una cella finta).
    /// `None` se non c'è un gruppo da proteggere o se le riserve sono finite.
    pub fn riserva_in_piu(&mut self) -> Option<(u64, Vec<u8>)> {
        let (g, celle) = self.ultimo.take()?;
        let j = self.r.checked_add(self.riserve_extra).filter(|j| *j < 128)?;
        self.riserve_extra += 1;
        let u = self.riserva(g, &celle, j);
        self.ultimo = Some((g, celle));
        Some(u)
    }

    fn riserva(&self, gruppo: u64, celle: &[Vec<u8>], j: u8) -> (u64, Vec<u8>) {
        let mut v = vec![0u8; celle[0].len()];
        for (i, c) in celle.iter().enumerate() {
            somma_per(&mut v, c, coeff(j, i as u8));
        }
        (intestazione(gruppo, celle.len() as u8, RISERVA | j as u16), v)
    }
}

// --- chi riceve -------------------------------------------------------------------------------

#[derive(Default)]
struct Gruppo {
    /// m, quando lo dice una riserva
    m: Option<u8>,
    originali: BTreeMap<u8, Vec<u8>>,
    riserve: BTreeMap<u8, Vec<u8>>,
    /// originali già consegnate (in ordine)
    consegnate: u8,
}

/// Decodificatore: riceve unità codificate (da qualunque strada, in qualunque ordine) e
/// restituisce le celle **in ordine**, appena può.
pub struct Decodificatore {
    k: u8,
    prossimo: u64,
    gruppi: BTreeMap<u64, Gruppo>,
}

impl Decodificatore {
    pub fn nuovo(k: u8) -> Self {
        Self { k, prossimo: 0, gruppi: BTreeMap::new() }
    }

    pub fn inserisci(&mut self, intest: u64, cella: Vec<u8>) -> Vec<Vec<u8>> {
        let (g, m, idx) = leggi_intestazione(intest);
        if g < self.prossimo || (self.gruppi.len() >= GRUPPI_MAX && !self.gruppi.contains_key(&g)) {
            return Vec::new();
        }
        let gr = self.gruppi.entry(g).or_default();
        if idx & RISERVA != 0 {
            if m == 0 || m > self.k {
                return Vec::new();
            }
            gr.m = Some(m);
            gr.riserve.entry((idx & 0x7f) as u8).or_insert(cella);
        } else if (idx as u8) < self.k {
            gr.originali.entry(idx as u8).or_insert(cella);
        }
        self.consegna()
    }

    fn consegna(&mut self) -> Vec<Vec<u8>> {
        let mut pronte = Vec::new();
        while let Some(gr) = self.gruppi.get_mut(&self.prossimo) {
            // originali consecutive: subito (nessun ritardo se niente è rimasto indietro)
            while let Some(c) = gr.originali.get(&gr.consegnate) {
                pronte.push(c.clone());
                gr.consegnate += 1;
            }
            let m = gr.m.unwrap_or(self.k);
            if gr.consegnate < m {
                // mancano originali: si ricavano dalle riserve, se bastano
                let mancanti = (gr.consegnate..m).filter(|i| !gr.originali.contains_key(i)).count();
                if gr.m.is_none() || gr.riserve.len() < mancanti || !risolvi(gr, m) {
                    break;
                }
                continue;
            }
            // gruppo completo: m noto (riserva arrivata) o k originali tutte arrivate
            if gr.m.is_none() && gr.consegnate < self.k {
                break;
            }
            self.gruppi.remove(&self.prossimo);
            self.prossimo += 1;
        }
        pronte
    }
}

/// Ricava le originali mancanti del gruppo (sistema lineare in GF(2^8), eliminazione di Gauss).
fn risolvi(gr: &mut Gruppo, m: u8) -> bool {
    let mancanti: Vec<u8> = (0..m).filter(|i| !gr.originali.contains_key(i)).collect();
    if mancanti.is_empty() {
        return true;
    }
    let n = mancanti.len();
    let lung = gr.riserve.values().next().map(|v| v.len()).unwrap_or(0);
    // righe: per ogni riserva usata, coefficienti delle mancanti | riserva meno le originali note
    let mut righe: Vec<(Vec<u8>, Vec<u8>)> = gr
        .riserve
        .iter()
        .take(n)
        .map(|(&j, v)| {
            let mut rhs = v.clone();
            for (&i, c) in &gr.originali {
                if i < m {
                    somma_per(&mut rhs, c, coeff(j, i));
                }
            }
            (mancanti.iter().map(|&i| coeff(j, i)).collect(), rhs)
        })
        .collect();
    if righe.len() < n || righe.iter().any(|(_, r)| r.len() != lung) {
        return false;
    }
    for col in 0..n {
        let Some(p) = (col..n).find(|&r| righe[r].0[col] != 0) else { return false };
        righe.swap(col, p);
        let f = inv(righe[col].0[col]);
        let (a, rhs) = &mut righe[col];
        for x in a.iter_mut() {
            *x = mul(*x, f);
        }
        let rhs_c = {
            let mut t = vec![0u8; lung];
            somma_per(&mut t, rhs, f);
            t
        };
        *rhs = rhs_c;
        let (pivot_a, pivot_rhs) = (righe[col].0.clone(), righe[col].1.clone());
        for (r, riga) in righe.iter_mut().enumerate() {
            if r != col && riga.0[col] != 0 {
                let c = riga.0[col];
                for (x, y) in riga.0.iter_mut().zip(&pivot_a) {
                    *x ^= mul(c, *y);
                }
                somma_per(&mut riga.1, &pivot_rhs, c);
            }
        }
    }
    for (r, &i) in mancanti.iter().enumerate() {
        gr.originali.insert(i, righe[r].1.clone());
    }
    true
}

#[cfg(test)]
mod test {
    use super::*;

    fn celle(n: usize) -> Vec<Vec<u8>> {
        (0..n).map(|i| (0..100).map(|b| (i * 31 + b * 7) as u8).collect()).collect()
    }

    #[test]
    fn aritmetica_gf256() {
        for a in 1..=255u8 {
            assert_eq!(mul(a, inv(a)), 1);
        }
        assert_eq!(mul(0x53, 0xca), mul(0xca, 0x53));
    }

    #[test]
    fn tutto_in_ordine_subito() {
        let (mut c, mut d) = (Codificatore::nuovo(8, 2), Decodificatore::nuovo(8));
        let mut uscite = Vec::new();
        for x in celle(20) {
            for (h, u) in c.cella(x) {
                uscite.extend(d.inserisci(h, u));
            }
        }
        for (h, u) in c.chiudi() {
            uscite.extend(d.inserisci(h, u));
        }
        assert_eq!(uscite, celle(20));
    }

    #[test]
    fn qualunque_k_su_k_piu_r_bastano() {
        // per ogni coppia di originali perse, le 2 riserve le ricostruiscono
        for a in 0..8 {
            for b in (a + 1)..8 {
                let (mut c, mut d) = (Codificatore::nuovo(8, 2), Decodificatore::nuovo(8));
                let mut unita = Vec::new();
                for x in celle(8) {
                    unita.extend(c.cella(x));
                }
                let mut uscite = Vec::new();
                for (n, (h, u)) in unita.into_iter().enumerate() {
                    if n != a && n != b {
                        uscite.extend(d.inserisci(h, u));
                    }
                }
                assert_eq!(uscite, celle(8), "perse {a} e {b}");
            }
        }
    }

    #[test]
    fn fuori_ordine_e_gruppo_incompleto() {
        let (mut c, mut d) = (Codificatore::nuovo(8, 3), Decodificatore::nuovo(8));
        let mut unita = Vec::new();
        for x in celle(13) {
            unita.extend(c.cella(x));
        }
        unita.extend(c.chiudi()); // secondo gruppo: 5 celle vere
        // arrivi rimescolati, e manca l'originale 2 del primo gruppo e la 10 (indice 2 del secondo)
        unita.retain(|(h, _)| {
            let (g, _, i) = leggi_intestazione(*h);
            !(i == 2 && (g == 0 || g == 1))
        });
        unita.reverse();
        let mut uscite = Vec::new();
        for (h, u) in unita {
            uscite.extend(d.inserisci(h, u));
        }
        assert_eq!(uscite, celle(13));
    }

    #[test]
    fn rumore_utile_e_negoziazione() {
        let mut c = Codificatore::nuovo(4, 1);
        assert!(c.riserva_in_piu().is_none(), "niente gruppo: niente riserva");
        let mut unita = Vec::new();
        for x in celle(4) {
            unita.extend(c.cella(x));
        }
        let extra = c.riserva_in_piu().unwrap();
        // perse due originali: la riserva normale + quella «di rumore» le ricostruiscono
        let mut d = Decodificatore::nuovo(4);
        let mut uscite = Vec::new();
        for (h, u) in unita.into_iter().filter(|(h, _)| leggi_intestazione(*h).2 > 1) {
            uscite.extend(d.inserisci(h, u));
        }
        uscite.extend(d.inserisci(extra.0, extra.1));
        assert_eq!(uscite, celle(4));
        assert_eq!(leggi_richiesta(&richiesta(8, 2)), Some((8, 2)));
        assert_eq!(leggi_richiesta(&[0u8; 10]), None);
        assert_eq!(leggi_richiesta(&richiesta(0, 2)), None);
    }
}
