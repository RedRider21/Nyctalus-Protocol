// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Etichette segrete rotanti (VISIONE.md §5.4).
//!
//! Ogni frammento porta 8 byte di etichetta. Per i nodi che la vedono è rumore
//! casuale e cambia a ogni frammento; solo chi conosce il segreto condiviso
//! del flusso sa ricavarne il numero d'ordine del frammento.
//!
//! Etichetta del frammento `i` = primi 8 byte di BLAKE3 in modalità "keyed"
//! con chiave derivata da (segreto condiviso, id del flusso) e messaggio `i`.
//! Il destinatario pre-calcola le etichette di una finestra di indici attesi
//! e le cerca in una tabella: il riconoscimento costa una sola lettura.

use std::collections::HashMap;

use crate::chiavi::{self, CONTESTO_ETICHETTE};

pub const LUNGHEZZA_ETICHETTA: usize = 8;
pub type Etichetta = [u8; LUNGHEZZA_ETICHETTA];

/// Calcola le etichette di un flusso. Lo usano sia il mittente sia il
/// destinatario, che condividono lo stesso segreto.
pub struct GeneratoreEtichette {
    chiave: [u8; 32],
}

impl GeneratoreEtichette {
    pub fn nuovo(segreto_condiviso: &[u8; 32], id_flusso: u64) -> Self {
        Self { chiave: chiavi::deriva(CONTESTO_ETICHETTE, segreto_condiviso, id_flusso) }
    }

    pub fn etichetta(&self, indice: u64) -> Etichetta {
        let hash = blake3::keyed_hash(&self.chiave, &indice.to_le_bytes());
        let mut etichetta = [0u8; LUNGHEZZA_ETICHETTA];
        etichetta.copy_from_slice(&hash.as_bytes()[..LUNGHEZZA_ETICHETTA]);
        etichetta
    }
}

impl Drop for GeneratoreEtichette {
    fn drop(&mut self) {
        chiavi::azzera(&mut self.chiave);
    }
}

/// Lato destinatario: riconosce le etichette degli indici nella finestra
/// `[base, base + finestra)`.
///
/// Il riconoscimento è in due tempi: [`cerca`](Self::cerca) trova l'indice,
/// [`consuma`](Self::consuma) toglie l'etichetta dalla tabella **solo dopo**
/// che il pacchetto ha superato l'autenticazione. Così:
/// - un replay (stesso frammento rispedito) non viene più riconosciuto;
/// - un nodo che ha visto passare un'etichetta non può "bruciarla" mandando
///   per primo un pacchetto falso con la stessa etichetta.
pub struct RiconoscitoreEtichette {
    generatore: GeneratoreEtichette,
    finestra: u64,
    base: u64,
    fine: u64,
    attese: HashMap<Etichetta, u64>,
}

impl RiconoscitoreEtichette {
    pub fn nuovo(generatore: GeneratoreEtichette, finestra: u64) -> Self {
        assert!(finestra > 0, "la finestra deve contenere almeno un indice");
        let mut riconoscitore = Self {
            generatore,
            finestra,
            base: 0,
            fine: 0,
            attese: HashMap::with_capacity(finestra as usize),
        };
        riconoscitore.avanza(0);
        riconoscitore
    }

    /// Restituisce l'indice del frammento, oppure `None` se l'etichetta non
    /// appartiene a questo flusso, è fuori finestra o è già stata consumata.
    pub fn cerca(&self, etichetta: &Etichetta) -> Option<u64> {
        self.attese.get(etichetta).copied()
    }

    /// Segna l'etichetta come usata: da chiamare dopo l'autenticazione.
    pub fn consuma(&mut self, etichetta: &Etichetta) {
        self.attese.remove(etichetta);
    }

    /// Sposta l'inizio della finestra a `nuova_base` (tipicamente il prossimo
    /// indice che il ricompositore aspetta) e calcola le nuove etichette.
    pub fn avanza(&mut self, nuova_base: u64) {
        if nuova_base < self.base {
            return;
        }
        self.base = nuova_base;
        let base = self.base;
        self.attese.retain(|_, indice| *indice >= base);
        let nuova_fine = self.base.saturating_add(self.finestra);
        for indice in self.fine.max(self.base)..nuova_fine {
            self.attese.insert(self.generatore.etichetta(indice), indice);
        }
        self.fine = nuova_fine;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const SEGRETO: [u8; 32] = [7; 32];

    #[test]
    fn stesse_chiavi_stesse_etichette() {
        let a = GeneratoreEtichette::nuovo(&SEGRETO, 1);
        let b = GeneratoreEtichette::nuovo(&SEGRETO, 1);
        assert_eq!(a.etichetta(42), b.etichetta(42));
    }

    #[test]
    fn etichette_diverse_per_indice_flusso_e_segreto() {
        let a = GeneratoreEtichette::nuovo(&SEGRETO, 1);
        assert_ne!(a.etichetta(0), a.etichetta(1));
        let altro_flusso = GeneratoreEtichette::nuovo(&SEGRETO, 2);
        assert_ne!(a.etichetta(0), altro_flusso.etichetta(0));
        let altro_segreto = GeneratoreEtichette::nuovo(&[8; 32], 1);
        assert_ne!(a.etichetta(0), altro_segreto.etichetta(0));
    }

    #[test]
    fn riconosce_una_volta_sola() {
        let mittente = GeneratoreEtichette::nuovo(&SEGRETO, 1);
        let mut ricevente =
            RiconoscitoreEtichette::nuovo(GeneratoreEtichette::nuovo(&SEGRETO, 1), 16);
        let etichetta = mittente.etichetta(5);
        assert_eq!(ricevente.cerca(&etichetta), Some(5));
        assert_eq!(ricevente.cerca(&etichetta), Some(5), "cercare non deve consumare");
        ricevente.consuma(&etichetta);
        assert_eq!(ricevente.cerca(&etichetta), None, "replay accettato");
    }

    #[test]
    fn rifiuta_etichette_fuori_finestra_ed_estranee() {
        let mittente = GeneratoreEtichette::nuovo(&SEGRETO, 1);
        let mut ricevente =
            RiconoscitoreEtichette::nuovo(GeneratoreEtichette::nuovo(&SEGRETO, 1), 16);
        assert_eq!(ricevente.cerca(&mittente.etichetta(16)), None);
        assert_eq!(ricevente.cerca(&[0xAA; LUNGHEZZA_ETICHETTA]), None);
        ricevente.avanza(10);
        assert_eq!(ricevente.cerca(&mittente.etichetta(16)), Some(16));
        assert_eq!(ricevente.cerca(&mittente.etichetta(3)), None);
    }
}
