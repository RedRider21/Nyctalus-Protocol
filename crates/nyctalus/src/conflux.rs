// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Conflux (punto E2): un **singolo** flusso di celle viene spezzato su più
//! circuiti paralleli e **ricomposto a destinazione**.
//!
//! Ogni unità porta un numero di sequenza end-to-end a 8 byte, posto **dentro**
//! gli strati a cipolla: i relè non lo vedono. Il ricevitore usa [`Riordina`]
//! per riconsegnare le celle nell'ordine originale, qualunque sia il circuito
//! su cui sono arrivate.

use std::collections::BTreeMap;

/// Massimo numero di unità fuori ordine tenute in attesa (anti-DoS).
const MAX_IN_ATTESA: usize = 8192;

/// Antepone il numero di sequenza alla cella.
pub fn impacchetta(seq: u64, cella: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(8 + cella.len());
    v.extend_from_slice(&seq.to_be_bytes());
    v.extend_from_slice(cella);
    v
}

/// Lunghezza di un'unità conflux (seq + cella a lunghezza fissa): la stessa per
/// ogni quadro dati del circuito.
pub const LUNGHEZZA_UNITA: usize = 8 + crate::cella::LUNGHEZZA;

/// Quadro di LINK (set_id del gruppo) **riempito** alla lunghezza di un'unità
/// dati, così il primo quadro del circuito non si distingue per dimensione. Il
/// destinatario legge i primi 16 byte come set_id e ignora il riempimento.
pub fn quadro_link(set_id: &[u8; 16]) -> Vec<u8> {
    let mut v = vec![0u8; LUNGHEZZA_UNITA];
    v[..16].copy_from_slice(set_id);
    v
}

/// Estrae (numero di sequenza, cella) da un'unità conflux.
pub fn spacchetta(byte: &[u8]) -> Option<(u64, Vec<u8>)> {
    if byte.len() < 8 {
        return None;
    }
    let seq = u64::from_be_bytes(byte[..8].try_into().unwrap());
    Some((seq, byte[8..].to_vec()))
}

/// Ricompositore: riordina le unità arrivate da circuiti diversi.
pub struct Riordina {
    prossimo: u64,
    buffer: BTreeMap<u64, Vec<u8>>,
}

impl Default for Riordina {
    fn default() -> Self {
        Self::nuovo()
    }
}

impl Riordina {
    pub fn nuovo() -> Self {
        Self { prossimo: 0, buffer: BTreeMap::new() }
    }

    /// Inserisce un'unità e restituisce le celle diventate consegnabili, in
    /// ordine. Scarta i duplicati/arretrati e, se il buffer è pieno, le unità
    /// troppo in avanti.
    pub fn inserisci(&mut self, seq: u64, cella: Vec<u8>) -> Vec<Vec<u8>> {
        let mut pronte = Vec::new();
        if seq < self.prossimo || self.buffer.contains_key(&seq) {
            return pronte;
        }
        if self.buffer.len() >= MAX_IN_ATTESA && seq != self.prossimo {
            return pronte;
        }
        self.buffer.insert(seq, cella);
        while let Some(c) = self.buffer.remove(&self.prossimo) {
            pronte.push(c);
            self.prossimo += 1;
        }
        pronte
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn impacchetta_e_spacchetta() {
        let u = impacchetta(42, b"cella");
        assert_eq!(spacchetta(&u), Some((42, b"cella".to_vec())));
        assert_eq!(spacchetta(&[0, 1, 2]), None);
    }

    #[test]
    fn unita_grande_come_il_pacchetto_di_apertura() {
        // Uniformità dei quadri (H2): un'unità dati ha la stessa taglia del
        // pacchetto di apertura Sphinx, così il 1º quadro non si distingue.
        assert_eq!(LUNGHEZZA_UNITA, nyctalus_core::circuito::TAGLIA_PACCHETTO);
        assert_eq!(quadro_link(&[0u8; 16]).len(), LUNGHEZZA_UNITA);
    }

    #[test]
    fn in_ordine() {
        let mut r = Riordina::nuovo();
        assert_eq!(r.inserisci(0, b"a".to_vec()), vec![b"a".to_vec()]);
        assert_eq!(r.inserisci(1, b"b".to_vec()), vec![b"b".to_vec()]);
    }

    #[test]
    fn fuori_ordine_si_ricompone() {
        let mut r = Riordina::nuovo();
        assert_eq!(r.inserisci(2, b"c".to_vec()), Vec::<Vec<u8>>::new());
        assert_eq!(r.inserisci(0, b"a".to_vec()), vec![b"a".to_vec()]);
        // Arrivo del pezzo 1: sblocca 1 e 2.
        assert_eq!(r.inserisci(1, b"b".to_vec()), vec![b"b".to_vec(), b"c".to_vec()]);
    }

    #[test]
    fn scarta_duplicati_e_arretrati() {
        let mut r = Riordina::nuovo();
        r.inserisci(0, b"a".to_vec());
        assert!(r.inserisci(0, b"a".to_vec()).is_empty());
        assert!(r.inserisci(0, b"x".to_vec()).is_empty());
    }

    #[test]
    fn simulazione_due_circuiti() {
        // seq pari su un circuito, dispari sull'altro, interlacciati e in disordine.
        let mut r = Riordina::nuovo();
        let mut uscita = Vec::new();
        for seq in [1u64, 0, 3, 2, 5, 4, 6] {
            for c in r.inserisci(seq, vec![seq as u8]) {
                uscita.push(c[0]);
            }
        }
        assert_eq!(uscita, vec![0, 1, 2, 3, 4, 5, 6]);
    }
}
