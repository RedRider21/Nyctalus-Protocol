// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Buste della **casella cifrata**: messaggi per chi non è in linea, lasciati in deposito
//! presso la casella della rete e ritirati dal destinatario quando torna.
//!
//! Schema **Noise_X_25519_ChaChaPoly_BLAKE2s** (a senso unico, un solo messaggio):
//!
//! ```text
//!   <- s              il mittente conosce la chiave pubblica del destinatario (il suo .nyct)
//!   ...
//!   -> e, es, s, ss   chiave effimera, poi la chiave STATICA del mittente cifrata, poi il testo
//! ```
//!
//! - Solo il destinatario apre la busta (es, ss richiedono la sua chiave privata).
//! - Il mittente è **autenticato** (ss): nessuno può lasciare posta a nome di un altro, e la
//!   sua identità viaggia cifrata, quindi la casella non sa chi scrive.
//! - Non c'è protezione dalle ripetizioni (un solo messaggio): il carico porta un
//!   identificativo casuale e il destinatario scarta quelli già visti.
//! - Niente forward secrecy rispetto alla chiave del destinatario: è il prezzo di poter
//!   scrivere a chi è spento. Per questo la casella tiene la posta per un tempo limitato.

use crate::stretta::{ErroreStretta, Identita};

pub const SCHEMA_CASELLA: &str = "Noise_X_25519_ChaChaPoly_BLAKE2s";
/// Lega le buste a questa versione del protocollo e le distingue dalle strette di mano.
const PROLOGO_CASELLA: &[u8] = b"Nyctalus v0 2026-10-31 casella";
/// Byte aggiunti al carico: effimera (32) + statica cifrata (32+16) + tag del carico (16).
pub const SOVRAPPIU: usize = 96;
/// Carico massimo di una busta (un messaggio Noise sta in 65535 byte).
pub const CARICO_MAX: usize = 65535 - SOVRAPPIU;

fn costruttore() -> snow::Builder<'static> {
    snow::Builder::new(SCHEMA_CASELLA.parse().expect("schema Noise valido"))
}

/// Chiude `carico` in una busta che solo il titolare di `destinatario` può aprire, firmata
/// (in senso Noise) dall'identità del mittente.
pub fn sigilla(mittente: &Identita, destinatario: &[u8; 32], carico: &[u8]) -> Result<Vec<u8>, ErroreStretta> {
    if carico.len() > CARICO_MAX {
        return Err(ErroreStretta("messaggio troppo lungo per la casella".into()));
    }
    let mut stato = costruttore()
        .prologue(PROLOGO_CASELLA)?
        .local_private_key(mittente.privata())?
        .remote_public_key(destinatario)?
        .build_initiator()?;
    let mut busta = vec![0u8; carico.len() + SOVRAPPIU];
    let n = stato.write_message(carico, &mut busta)?;
    busta.truncate(n);
    Ok(busta)
}

/// Apre una busta: restituisce la chiave pubblica del mittente (autenticata) e il carico.
pub fn apri(destinatario: &Identita, busta: &[u8]) -> Result<([u8; 32], Vec<u8>), ErroreStretta> {
    if busta.len() < SOVRAPPIU || busta.len() > 65535 {
        return Err(ErroreStretta("busta di lunghezza errata".into()));
    }
    let mut stato = costruttore()
        .prologue(PROLOGO_CASELLA)?
        .local_private_key(destinatario.privata())?
        .build_responder()?;
    let mut carico = vec![0u8; busta.len()];
    let n = stato.read_message(busta, &mut carico)?;
    carico.truncate(n);
    let statica = stato.get_remote_static().ok_or_else(|| ErroreStretta("mittente assente".into()))?;
    let mut mittente = [0u8; 32];
    mittente.copy_from_slice(statica);
    Ok((mittente, carico))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn il_destinatario_apre_e_riconosce_il_mittente() {
        let anna = Identita::genera().unwrap();
        let bruno = Identita::genera().unwrap();
        let busta = sigilla(&anna, &bruno.pubblica(), b"ciao Bruno").unwrap();
        assert_eq!(busta.len(), b"ciao Bruno".len() + SOVRAPPIU);
        let (da, testo) = apri(&bruno, &busta).unwrap();
        assert_eq!(da, anna.pubblica());
        assert_eq!(testo, b"ciao Bruno");
    }

    #[test]
    fn nessun_altro_la_apre() {
        let anna = Identita::genera().unwrap();
        let bruno = Identita::genera().unwrap();
        let carla = Identita::genera().unwrap();
        let busta = sigilla(&anna, &bruno.pubblica(), b"segreto").unwrap();
        assert!(apri(&carla, &busta).is_err());
    }

    #[test]
    fn una_busta_alterata_viene_rifiutata() {
        let anna = Identita::genera().unwrap();
        let bruno = Identita::genera().unwrap();
        let mut busta = sigilla(&anna, &bruno.pubblica(), b"ciao").unwrap();
        let ultimo = busta.len() - 1;
        busta[ultimo] ^= 1;
        assert!(apri(&bruno, &busta).is_err());
        busta[ultimo] ^= 1;
        busta[40] ^= 1; // la statica cifrata del mittente
        assert!(apri(&bruno, &busta).is_err());
    }

    #[test]
    fn il_testo_non_compare_in_chiaro_e_ogni_busta_e_diversa() {
        let anna = Identita::genera().unwrap();
        let bruno = Identita::genera().unwrap();
        let a = sigilla(&anna, &bruno.pubblica(), b"testo riconoscibile").unwrap();
        let b = sigilla(&anna, &bruno.pubblica(), b"testo riconoscibile").unwrap();
        assert!(!a.windows(5).any(|w| w == b"testo"));
        assert!(!a.windows(32).any(|w| w == anna.pubblica()));
        assert_ne!(a, b);
    }

    #[test]
    fn troppo_lungo_rifiutato() {
        let anna = Identita::genera().unwrap();
        let bruno = Identita::genera().unwrap();
        assert!(sigilla(&anna, &bruno.pubblica(), &vec![0u8; CARICO_MAX + 1]).is_err());
        assert!(sigilla(&anna, &bruno.pubblica(), &vec![0u8; CARICO_MAX]).is_ok());
    }
}
