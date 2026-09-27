//! Ricomposizione dei frammenti arrivati in disordine dalla ragnatela.
//!
//! Riprende l'idea dello `ShardReassembler` degli appunti, correggendone i
//! punti deboli:
//! - memoria limitata (numero di frammenti e byte in attesa): un attaccante
//!   non può far crescere il buffer all'infinito;
//! - la fine del flusso è segnata dal frammento marcato "ultimo", non da un
//!   "totale frammenti" dichiarato nel primo pacchetto che arriva;
//! - duplicati e frammenti fuori finestra vengono segnalati, non ignorati in
//!   silenzio, così il livello superiore può misurare i nodi che si
//!   comportano male.

use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroreRicomposizione {
    /// Frammento già ricevuto o già consegnato.
    Duplicato,
    /// Indice troppo avanti rispetto a quello atteso.
    FuoriFinestra,
    /// Superato il limite di byte in attesa.
    MemoriaEsaurita,
    /// Indice oltre il frammento marcato come ultimo.
    OltreLaFine,
    /// Due frammenti diversi dichiarano di essere l'ultimo.
    FineIncoerente,
}

impl fmt::Display for ErroreRicomposizione {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let testo = match self {
            Self::Duplicato => "frammento duplicato",
            Self::FuoriFinestra => "frammento fuori finestra",
            Self::MemoriaEsaurita => "memoria di ricomposizione esaurita",
            Self::OltreLaFine => "frammento oltre la fine del flusso",
            Self::FineIncoerente => "fine del flusso incoerente",
        };
        f.write_str(testo)
    }
}

impl std::error::Error for ErroreRicomposizione {}

pub struct Ricompositore {
    prossimo: u64,
    finestra: u64,
    max_byte: usize,
    byte_in_attesa: usize,
    in_attesa: BTreeMap<u64, Vec<u8>>,
    ultimo: Option<u64>,
}

impl Ricompositore {
    /// `finestra`: quanti frammenti oltre quello atteso si accettano.
    /// `max_byte`: limite di memoria per i frammenti in attesa.
    pub fn nuovo(finestra: u64, max_byte: usize) -> Self {
        Self {
            prossimo: 0,
            finestra,
            max_byte,
            byte_in_attesa: 0,
            in_attesa: BTreeMap::new(),
            ultimo: None,
        }
    }

    pub fn inserisci(
        &mut self,
        indice: u64,
        dati: Vec<u8>,
        ultimo: bool,
    ) -> Result<(), ErroreRicomposizione> {
        if indice < self.prossimo || self.in_attesa.contains_key(&indice) {
            return Err(ErroreRicomposizione::Duplicato);
        }
        if indice - self.prossimo >= self.finestra {
            return Err(ErroreRicomposizione::FuoriFinestra);
        }
        match self.ultimo {
            Some(fine) if indice > fine => return Err(ErroreRicomposizione::OltreLaFine),
            Some(fine) if ultimo && indice != fine => {
                return Err(ErroreRicomposizione::FineIncoerente);
            }
            None if ultimo && self.in_attesa.keys().next_back().is_some_and(|&m| m > indice) => {
                return Err(ErroreRicomposizione::FineIncoerente);
            }
            _ => {}
        }
        if self.byte_in_attesa + dati.len() > self.max_byte {
            return Err(ErroreRicomposizione::MemoriaEsaurita);
        }
        if ultimo {
            self.ultimo = Some(indice);
        }
        self.byte_in_attesa += dati.len();
        self.in_attesa.insert(indice, dati);
        Ok(())
    }

    /// Consegna tutti i dati contigui a partire dal prossimo indice atteso.
    pub fn estrai_pronti(&mut self) -> Vec<u8> {
        let mut pronti = Vec::new();
        while let Some(dati) = self.in_attesa.remove(&self.prossimo) {
            self.byte_in_attesa -= dati.len();
            pronti.extend_from_slice(&dati);
            self.prossimo += 1;
        }
        pronti
    }

    pub fn prossimo_atteso(&self) -> u64 {
        self.prossimo
    }

    /// Vero quando anche il frammento marcato "ultimo" è stato consegnato.
    pub fn completo(&self) -> bool {
        self.ultimo.is_some_and(|fine| self.prossimo > fine)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn ricompone_in_disordine() {
        let mut r = Ricompositore::nuovo(8, 1024);
        r.inserisci(2, b"C".to_vec(), true).unwrap();
        r.inserisci(0, b"A".to_vec(), false).unwrap();
        assert_eq!(r.estrai_pronti(), b"A");
        assert!(!r.completo());
        r.inserisci(1, b"B".to_vec(), false).unwrap();
        assert_eq!(r.estrai_pronti(), b"BC");
        assert!(r.completo());
    }

    #[test]
    fn rifiuta_duplicati_e_fuori_finestra() {
        let mut r = Ricompositore::nuovo(4, 1024);
        r.inserisci(0, b"A".to_vec(), false).unwrap();
        assert_eq!(r.inserisci(0, b"A".to_vec(), false), Err(ErroreRicomposizione::Duplicato));
        r.estrai_pronti();
        assert_eq!(r.inserisci(0, b"A".to_vec(), false), Err(ErroreRicomposizione::Duplicato));
        assert_eq!(r.inserisci(5, b"X".to_vec(), false), Err(ErroreRicomposizione::FuoriFinestra));
    }

    #[test]
    fn limita_la_memoria() {
        let mut r = Ricompositore::nuovo(100, 10);
        r.inserisci(1, vec![0; 6], false).unwrap();
        assert_eq!(r.inserisci(2, vec![0; 6], false), Err(ErroreRicomposizione::MemoriaEsaurita));
    }

    #[test]
    fn fine_del_flusso_protetta() {
        let mut r = Ricompositore::nuovo(100, 1024);
        r.inserisci(5, b"X".to_vec(), false).unwrap();
        assert_eq!(r.inserisci(3, b"Y".to_vec(), true), Err(ErroreRicomposizione::FineIncoerente));
        r.inserisci(6, b"Z".to_vec(), true).unwrap();
        assert_eq!(r.inserisci(7, b"W".to_vec(), false), Err(ErroreRicomposizione::OltreLaFine));
        assert_eq!(r.inserisci(4, b"V".to_vec(), true), Err(ErroreRicomposizione::FineIncoerente));
    }
}
