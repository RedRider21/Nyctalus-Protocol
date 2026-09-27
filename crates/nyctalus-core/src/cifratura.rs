//! Cifratura end-to-end dei frammenti con ChaCha20-Poly1305.
//!
//! Formato del pacchetto sulla rete (tutti i pacchetti di un flusso hanno la
//! **stessa lunghezza**, così dalla dimensione non si capisce nulla):
//!
//! ```text
//! +-----------+--------------------------------------------------+---------+
//! | etichetta |            parte cifrata (ChaCha20)               |   tag   |
//! |  8 byte   | segnali 1 | lunghezza 2 | dati | riempimento a 0 | 16 byte |
//! +-----------+--------------------------------------------------+---------+
//! ```
//!
//! - L'etichetta resta in chiaro (serve al destinatario per riconoscere il
//!   frammento) ma è autenticata: se qualcuno la cambia, l'apertura fallisce.
//! - Il segnale "ultimo frammento" e la lunghezza reale dei dati sono dentro
//!   la parte cifrata: i nodi non vedono dove finisce un flusso.
//! - Il nonce è l'indice del frammento: unico per chiave, perché ogni flusso
//!   ha la sua chiave e ogni indice si usa una volta sola.

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use std::fmt;

use crate::chiavi::{self, CONTESTO_CIFRATURA};
use crate::etichette::{Etichetta, LUNGHEZZA_ETICHETTA};

pub const LUNGHEZZA_TAG_AUTENTICAZIONE: usize = 16;
const INTESTAZIONE_INTERNA: usize = 3;
const SEGNALE_ULTIMO: u8 = 0b0000_0001;

/// Lunghezza sulla rete di un pacchetto con `dimensione_frammento` byte utili.
pub const fn lunghezza_pacchetto(dimensione_frammento: usize) -> usize {
    LUNGHEZZA_ETICHETTA + INTESTAZIONE_INTERNA + dimensione_frammento + LUNGHEZZA_TAG_AUTENTICAZIONE
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroreCifratura {
    /// Lunghezza del pacchetto diversa da quella del flusso.
    LunghezzaErrata,
    /// Pacchetto manomesso, chiave sbagliata o indice sbagliato.
    AutenticazioneFallita,
    /// Autentico ma con contenuto incoerente (lunghezza o segnali non validi).
    ContenutoMalformato,
}

impl fmt::Display for ErroreCifratura {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let testo = match self {
            Self::LunghezzaErrata => "lunghezza del pacchetto errata",
            Self::AutenticazioneFallita => "autenticazione del pacchetto fallita",
            Self::ContenutoMalformato => "contenuto del pacchetto malformato",
        };
        f.write_str(testo)
    }
}

impl std::error::Error for ErroreCifratura {}

pub struct CifrarioFlusso {
    aead: ChaCha20Poly1305,
    dimensione_frammento: usize,
}

impl CifrarioFlusso {
    pub fn nuovo(segreto_condiviso: &[u8; 32], id_flusso: u64, dimensione_frammento: usize) -> Self {
        assert!(
            dimensione_frammento <= u16::MAX as usize,
            "la lunghezza dei dati deve stare in 2 byte"
        );
        let mut chiave = chiavi::deriva(CONTESTO_CIFRATURA, segreto_condiviso, id_flusso);
        let aead = ChaCha20Poly1305::new(&Key::from(chiave));
        chiavi::azzera(&mut chiave);
        Self { aead, dimensione_frammento }
    }

    pub fn lunghezza_pacchetto(&self) -> usize {
        lunghezza_pacchetto(self.dimensione_frammento)
    }

    /// Cifra un frammento e restituisce il pacchetto pronto per la rete.
    pub fn sigilla(&self, indice: u64, etichetta: &Etichetta, dati: &[u8], ultimo: bool) -> Vec<u8> {
        assert!(dati.len() <= self.dimensione_frammento, "frammento troppo grande");
        let mut chiaro = Vec::with_capacity(INTESTAZIONE_INTERNA + self.dimensione_frammento);
        chiaro.push(if ultimo { SEGNALE_ULTIMO } else { 0 });
        chiaro.extend_from_slice(&(dati.len() as u16).to_le_bytes());
        chiaro.extend_from_slice(dati);
        chiaro.resize(INTESTAZIONE_INTERNA + self.dimensione_frammento, 0);

        let cifrato = self
            .aead
            .encrypt(&nonce(indice), Payload { msg: &chiaro, aad: etichetta })
            .expect("la cifratura ChaCha20-Poly1305 non fallisce su buffer in memoria");
        chiavi::azzera(&mut chiaro);

        let mut pacchetto = Vec::with_capacity(self.lunghezza_pacchetto());
        pacchetto.extend_from_slice(etichetta);
        pacchetto.extend_from_slice(&cifrato);
        pacchetto
    }

    /// Verifica e decifra un pacchetto di cui si conosce già l'indice
    /// (ricavato dall'etichetta). Restituisce i dati e il segnale "ultimo".
    pub fn apri(&self, indice: u64, pacchetto: &[u8]) -> Result<(Vec<u8>, bool), ErroreCifratura> {
        if pacchetto.len() != self.lunghezza_pacchetto() {
            return Err(ErroreCifratura::LunghezzaErrata);
        }
        let (etichetta, cifrato) = pacchetto.split_at(LUNGHEZZA_ETICHETTA);
        let mut chiaro = self
            .aead
            .decrypt(&nonce(indice), Payload { msg: cifrato, aad: etichetta })
            .map_err(|_| ErroreCifratura::AutenticazioneFallita)?;

        let segnali = chiaro[0];
        let lunghezza = u16::from_le_bytes([chiaro[1], chiaro[2]]) as usize;
        if segnali & !SEGNALE_ULTIMO != 0 || lunghezza > self.dimensione_frammento {
            chiavi::azzera(&mut chiaro);
            return Err(ErroreCifratura::ContenutoMalformato);
        }
        let dati = chiaro[INTESTAZIONE_INTERNA..INTESTAZIONE_INTERNA + lunghezza].to_vec();
        chiavi::azzera(&mut chiaro);
        Ok((dati, segnali & SEGNALE_ULTIMO != 0))
    }
}

fn nonce(indice: u64) -> Nonce {
    let mut byte = [0u8; 12];
    byte[..8].copy_from_slice(&indice.to_le_bytes());
    Nonce::from(byte)
}

#[cfg(test)]
mod test {
    use super::*;

    const SEGRETO: [u8; 32] = [3; 32];
    const ETICHETTA: Etichetta = [9; LUNGHEZZA_ETICHETTA];

    #[test]
    fn andata_e_ritorno() {
        let c = CifrarioFlusso::nuovo(&SEGRETO, 1, 64);
        let pacchetto = c.sigilla(7, &ETICHETTA, b"ciao", true);
        assert_eq!(pacchetto.len(), lunghezza_pacchetto(64));
        assert_eq!(c.apri(7, &pacchetto), Ok((b"ciao".to_vec(), true)));
    }

    #[test]
    fn stessa_lunghezza_per_ogni_contenuto() {
        let c = CifrarioFlusso::nuovo(&SEGRETO, 1, 64);
        let corto = c.sigilla(0, &ETICHETTA, b"", false);
        let lungo = c.sigilla(1, &ETICHETTA, &[0xFF; 64], true);
        assert_eq!(corto.len(), lungo.len());
    }

    #[test]
    fn rifiuta_manomissioni() {
        let c = CifrarioFlusso::nuovo(&SEGRETO, 1, 64);
        let originale = c.sigilla(7, &ETICHETTA, b"ciao", false);

        let mut dati_cambiati = originale.clone();
        dati_cambiati[20] ^= 1;
        assert_eq!(c.apri(7, &dati_cambiati), Err(ErroreCifratura::AutenticazioneFallita));

        let mut etichetta_cambiata = originale.clone();
        etichetta_cambiata[0] ^= 1;
        assert_eq!(c.apri(7, &etichetta_cambiata), Err(ErroreCifratura::AutenticazioneFallita));

        assert_eq!(c.apri(8, &originale), Err(ErroreCifratura::AutenticazioneFallita));
        assert_eq!(c.apri(7, &originale[1..]), Err(ErroreCifratura::LunghezzaErrata));

        let altro_flusso = CifrarioFlusso::nuovo(&SEGRETO, 2, 64);
        assert_eq!(altro_flusso.apri(7, &originale), Err(ErroreCifratura::AutenticazioneFallita));
    }

    #[test]
    fn il_contenuto_non_compare_in_chiaro() {
        let c = CifrarioFlusso::nuovo(&SEGRETO, 1, 64);
        let segreto = b"password-segretissima";
        let pacchetto = c.sigilla(0, &ETICHETTA, segreto, false);
        assert!(!pacchetto.windows(segreto.len()).any(|w| w == segreto));
    }
}
