//! Stretta di mano Noise tra i due estremi di una conversazione (SPECIFICA §5).
//!
//! Schema **Noise_NK_25519_ChaChaPoly_BLAKE2s**:
//!
//! ```text
//!   <- s              il mittente conosce già la chiave pubblica del destinatario
//!   ...
//!   -> e, es          1° messaggio: chiave effimera del mittente
//!   <- e, ee          2° messaggio: chiave effimera del destinatario
//! ```
//!
//! - **N** (mittente): nessuna chiave statica, quindi il mittente resta
//!   anonimo anche verso il destinatario. Con IK dovrebbe rivelare la sua
//!   identità.
//! - **K** (destinatario): la chiave pubblica statica del destinatario è
//!   nota in anticipo, e funziona da "indirizzo". Solo chi possiede la chiave
//!   privata corrispondente riesce a completare la stretta di mano.
//! - **Forward secrecy:** i segreti finali dipendono dallo scambio tra le
//!   due chiavi effimere (ee), che vengono buttate a fine stretta di mano.
//!
//! Il risultato sono due segreti da 32 byte, uno per direzione, che
//! diventano il `segreto_condiviso` dei flussi L2 (§3.2).

use std::fmt;

use crate::chiavi;

pub const SCHEMA: &str = "Noise_NK_25519_ChaChaPoly_BLAKE2s";
/// Lega la stretta di mano alla versione del protocollo: versioni diverse
/// non riescono a completarla.
const PROLOGO: &[u8] = b"Nyctalus v0 2026-09-27 stretta di mano";
/// Messaggi della stretta di mano: 32 byte di chiave effimera + 16 di tag.
pub const LUNGHEZZA_MESSAGGIO: usize = 48;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroreStretta(String);

impl fmt::Display for ErroreStretta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "stretta di mano fallita: {}", self.0)
    }
}

impl std::error::Error for ErroreStretta {}

impl From<snow::Error> for ErroreStretta {
    fn from(errore: snow::Error) -> Self {
        Self(errore.to_string())
    }
}

fn costruttore() -> snow::Builder<'static> {
    snow::Builder::new(SCHEMA.parse().expect("schema Noise valido"))
}

/// Identità di lungo periodo di un destinatario (sito interno, nodo d'uscita).
pub struct Identita {
    privata: [u8; 32],
    pubblica: [u8; 32],
}

impl Identita {
    pub fn genera() -> Result<Self, ErroreStretta> {
        let coppia = costruttore().generate_keypair()?;
        Self::da_byte(&[coppia.private.as_slice(), coppia.public.as_slice()].concat())
    }

    /// Ricostruisce l'identità da 64 byte: privata ‖ pubblica.
    pub fn da_byte(byte: &[u8]) -> Result<Self, ErroreStretta> {
        if byte.len() != 64 {
            return Err(ErroreStretta("identità di lunghezza errata".into()));
        }
        let mut identita = Self { privata: [0; 32], pubblica: [0; 32] };
        identita.privata.copy_from_slice(&byte[..32]);
        identita.pubblica.copy_from_slice(&byte[32..]);
        Ok(identita)
    }

    /// 64 byte da salvare su disco (privata ‖ pubblica).
    pub fn in_byte(&self) -> Vec<u8> {
        [self.privata.as_slice(), self.pubblica.as_slice()].concat()
    }

    pub fn pubblica(&self) -> [u8; 32] {
        self.pubblica
    }
}

impl Drop for Identita {
    fn drop(&mut self) {
        chiavi::azzera(&mut self.privata);
    }
}

/// I due segreti prodotti dalla stretta di mano, uno per direzione.
pub struct SegretiSessione {
    pub verso_destinatario: [u8; 32],
    pub verso_mittente: [u8; 32],
}

impl Drop for SegretiSessione {
    fn drop(&mut self) {
        chiavi::azzera(&mut self.verso_destinatario);
        chiavi::azzera(&mut self.verso_mittente);
    }
}

fn segreti(stato: &mut snow::HandshakeState) -> SegretiSessione {
    let (verso_destinatario, verso_mittente) = stato.dangerously_get_raw_split();
    SegretiSessione { verso_destinatario, verso_mittente }
}

/// Lato mittente (chi inizia la conversazione).
pub struct Mittente {
    stato: snow::HandshakeState,
}

impl Mittente {
    /// Prepara la stretta di mano verso il destinatario con quella chiave
    /// pubblica. Restituisce anche il 1° messaggio da spedire.
    pub fn inizia(pubblica_destinatario: &[u8; 32]) -> Result<(Self, Vec<u8>), ErroreStretta> {
        let mut stato = costruttore()
            .prologue(PROLOGO)?
            .remote_public_key(pubblica_destinatario)?
            .build_initiator()?;
        let mut messaggio = vec![0u8; LUNGHEZZA_MESSAGGIO];
        let n = stato.write_message(&[], &mut messaggio)?;
        messaggio.truncate(n);
        Ok((Self { stato }, messaggio))
    }

    /// Legge il 2° messaggio e ricava i segreti della sessione.
    pub fn completa(mut self, risposta: &[u8]) -> Result<SegretiSessione, ErroreStretta> {
        let mut carico = [0u8; LUNGHEZZA_MESSAGGIO];
        self.stato.read_message(risposta, &mut carico)?;
        if !self.stato.is_handshake_finished() {
            return Err(ErroreStretta("stretta di mano non conclusa".into()));
        }
        Ok(segreti(&mut self.stato))
    }
}

/// Lato destinatario: legge il 1° messaggio e restituisce la risposta da
/// spedire insieme ai segreti della sessione.
pub fn rispondi(
    identita: &Identita,
    primo: &[u8],
) -> Result<(Vec<u8>, SegretiSessione), ErroreStretta> {
    let mut stato = costruttore()
        .prologue(PROLOGO)?
        .local_private_key(&identita.privata)?
        .build_responder()?;
    let mut carico = [0u8; LUNGHEZZA_MESSAGGIO];
    stato.read_message(primo, &mut carico)?;
    let mut risposta = vec![0u8; LUNGHEZZA_MESSAGGIO];
    let n = stato.write_message(&[], &mut risposta)?;
    risposta.truncate(n);
    if !stato.is_handshake_finished() {
        return Err(ErroreStretta("stretta di mano non conclusa".into()));
    }
    Ok((risposta, segreti(&mut stato)))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn i_due_lati_ottengono_gli_stessi_segreti() {
        let destinatario = Identita::genera().unwrap();
        let (mittente, primo) = Mittente::inizia(&destinatario.pubblica()).unwrap();
        assert_eq!(primo.len(), LUNGHEZZA_MESSAGGIO);
        let (risposta, lato_destinatario) = rispondi(&destinatario, &primo).unwrap();
        assert_eq!(risposta.len(), LUNGHEZZA_MESSAGGIO);
        let lato_mittente = mittente.completa(&risposta).unwrap();

        assert_eq!(lato_mittente.verso_destinatario, lato_destinatario.verso_destinatario);
        assert_eq!(lato_mittente.verso_mittente, lato_destinatario.verso_mittente);
        assert_ne!(lato_mittente.verso_destinatario, lato_mittente.verso_mittente);
    }

    #[test]
    fn ogni_sessione_ha_segreti_nuovi() {
        let destinatario = Identita::genera().unwrap();
        let sessione = || {
            let (mittente, primo) = Mittente::inizia(&destinatario.pubblica()).unwrap();
            let (risposta, _) = rispondi(&destinatario, &primo).unwrap();
            mittente.completa(&risposta).unwrap().verso_destinatario
        };
        assert_ne!(sessione(), sessione());
    }

    /// Un impostore che non possiede la chiave privata del destinatario non
    /// riesce nemmeno a leggere il 1° messaggio.
    #[test]
    fn un_impostore_non_completa_la_stretta() {
        let vero = Identita::genera().unwrap();
        let impostore = Identita::genera().unwrap();
        let (_, primo) = Mittente::inizia(&vero.pubblica()).unwrap();
        assert!(rispondi(&impostore, &primo).is_err());
    }

    #[test]
    fn una_risposta_manomessa_viene_rifiutata() {
        let destinatario = Identita::genera().unwrap();
        let (mittente, primo) = Mittente::inizia(&destinatario.pubblica()).unwrap();
        let (mut risposta, _) = rispondi(&destinatario, &primo).unwrap();
        risposta[40] ^= 1;
        assert!(mittente.completa(&risposta).is_err());
    }

    #[test]
    fn identita_salvata_e_ricaricata() {
        let originale = Identita::genera().unwrap();
        let ricaricata = Identita::da_byte(&originale.in_byte()).unwrap();
        assert_eq!(originale.pubblica(), ricaricata.pubblica());
        let (mittente, primo) = Mittente::inizia(&originale.pubblica()).unwrap();
        let (risposta, _) = rispondi(&ricaricata, &primo).unwrap();
        assert!(mittente.completa(&risposta).is_ok());
    }
}
