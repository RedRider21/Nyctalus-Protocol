// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Indirizzi `.nyct` dei siti e dei servizi interni (SPECIFICA §6).
//!
//! Come gli indirizzi `.onion` v3 di Tor, l'indirizzo **è** la chiave
//! pubblica del servizio: nessun registro, nessun DNS, nessuno può
//! "rubarlo" senza la chiave privata.
//!
//! ```text
//! indirizzo = base32( chiave_pubblica[32] ‖ controllo[2] ‖ versione[1] ) + ".nyct"
//! controllo = primi 2 byte di BLAKE3-derive_key("Nyctalus indirizzi v0 controllo",
//!                                               chiave_pubblica ‖ versione)
//! ```
//!
//! 35 byte in base32 (alfabeto RFC 4648 minuscolo, senza "=") sono
//! esattamente 56 caratteri. Il controllo serve a intercettare gli errori di
//! battitura: un indirizzo sbagliato viene rifiutato invece di portare a una
//! chiave inesistente.

use std::fmt;

pub const SUFFISSO: &str = ".nyct";
pub const VERSIONE: u8 = 0;
const LUNGHEZZA_BASE32: usize = 56;
const ALFABETO: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
const CONTESTO_CONTROLLO: &str = "Nyctalus indirizzi v0 controllo";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroreIndirizzo {
    SuffissoMancante,
    LunghezzaErrata,
    CarattereNonValido,
    VersioneSconosciuta,
    /// Il controllo non torna: quasi sempre un errore di battitura.
    ControlloFallito,
}

impl fmt::Display for ErroreIndirizzo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let testo = match self {
            Self::SuffissoMancante => "l'indirizzo deve finire con .nyct",
            Self::LunghezzaErrata => "l'indirizzo deve avere 56 caratteri prima di .nyct",
            Self::CarattereNonValido => "l'indirizzo contiene caratteri non validi",
            Self::VersioneSconosciuta => "versione dell'indirizzo non supportata",
            Self::ControlloFallito => "indirizzo non valido (probabile errore di battitura)",
        };
        f.write_str(testo)
    }
}

impl std::error::Error for ErroreIndirizzo {}

fn controllo(pubblica: &[u8; 32], versione: u8) -> [u8; 2] {
    let mut hasher = blake3::Hasher::new_derive_key(CONTESTO_CONTROLLO);
    hasher.update(pubblica);
    hasher.update(&[versione]);
    let hash = hasher.finalize();
    [hash.as_bytes()[0], hash.as_bytes()[1]]
}

/// Indirizzo `.nyct` di una chiave pubblica.
pub fn da_chiave(pubblica: &[u8; 32]) -> String {
    let mut byte = [0u8; 35];
    byte[..32].copy_from_slice(pubblica);
    byte[32..34].copy_from_slice(&controllo(pubblica, VERSIONE));
    byte[34] = VERSIONE;
    let mut indirizzo = in_base32(&byte);
    indirizzo.push_str(SUFFISSO);
    indirizzo
}

/// Chiave pubblica contenuta in un indirizzo `.nyct`. Accetta anche le
/// maiuscole e il punto finale dei nomi di dominio completi.
pub fn in_chiave(indirizzo: &str) -> Result<[u8; 32], ErroreIndirizzo> {
    let indirizzo = indirizzo.trim().trim_end_matches('.').to_ascii_lowercase();
    let corpo = indirizzo.strip_suffix(SUFFISSO).ok_or(ErroreIndirizzo::SuffissoMancante)?;
    if corpo.len() != LUNGHEZZA_BASE32 {
        return Err(ErroreIndirizzo::LunghezzaErrata);
    }
    let byte = da_base32(corpo)?;
    let pubblica: [u8; 32] = byte[..32].try_into().expect("35 byte decodificati");
    // Prima il controllo: un errore di battitura nell'ultimo carattere (che
    // contiene la versione) deve risultare come tale, non come "versione
    // sconosciuta".
    if byte[32..34] != controllo(&pubblica, byte[34]) {
        return Err(ErroreIndirizzo::ControlloFallito);
    }
    if byte[34] != VERSIONE {
        return Err(ErroreIndirizzo::VersioneSconosciuta);
    }
    Ok(pubblica)
}

fn in_base32(byte: &[u8; 35]) -> String {
    let mut testo = String::with_capacity(LUNGHEZZA_BASE32 + SUFFISSO.len());
    for gruppo in byte.chunks(5) {
        let valore = gruppo.iter().fold(0u64, |acc, &b| (acc << 8) | b as u64);
        for i in (0..8).rev() {
            testo.push(ALFABETO[((valore >> (i * 5)) & 31) as usize] as char);
        }
    }
    testo
}

fn da_base32(testo: &str) -> Result<[u8; 35], ErroreIndirizzo> {
    let mut byte = [0u8; 35];
    for (gruppo, caratteri) in testo.as_bytes().chunks(8).enumerate() {
        let mut valore = 0u64;
        for &c in caratteri {
            let cifra =
                ALFABETO.iter().position(|&a| a == c).ok_or(ErroreIndirizzo::CarattereNonValido)?;
            valore = (valore << 5) | cifra as u64;
        }
        for i in 0..5 {
            byte[gruppo * 5 + i] = (valore >> ((4 - i) * 8)) as u8;
        }
    }
    Ok(byte)
}

#[cfg(test)]
mod test {
    use super::*;

    const CHIAVE: [u8; 32] = [0x4a; 32];

    #[test]
    fn andata_e_ritorno() {
        let indirizzo = da_chiave(&CHIAVE);
        assert_eq!(indirizzo.len(), 56 + SUFFISSO.len());
        assert!(indirizzo.ends_with(".nyct"));
        assert_eq!(in_chiave(&indirizzo), Ok(CHIAVE));
    }

    #[test]
    fn chiavi_diverse_indirizzi_diversi() {
        let mut altra = CHIAVE;
        altra[31] ^= 1;
        assert_ne!(da_chiave(&CHIAVE), da_chiave(&altra));
    }

    #[test]
    fn accetta_maiuscole_e_punto_finale() {
        let indirizzo = da_chiave(&CHIAVE).to_uppercase() + ".";
        assert_eq!(in_chiave(&indirizzo), Ok(CHIAVE));
    }

    #[test]
    fn intercetta_errori_di_battitura() {
        let indirizzo = da_chiave(&CHIAVE);
        let mut sbagliato: Vec<u8> = indirizzo.into_bytes();
        sbagliato[10] = if sbagliato[10] == b'a' { b'b' } else { b'a' };
        let sbagliato = String::from_utf8(sbagliato).unwrap();
        assert_eq!(in_chiave(&sbagliato), Err(ErroreIndirizzo::ControlloFallito));
    }

    #[test]
    fn errore_nell_ultimo_carattere_e_di_battitura() {
        let mut indirizzo = da_chiave(&CHIAVE);
        let posizione = indirizzo.len() - SUFFISSO.len() - 1;
        indirizzo.replace_range(posizione..=posizione, "q");
        assert_eq!(in_chiave(&indirizzo), Err(ErroreIndirizzo::ControlloFallito));
    }

    #[test]
    fn rifiuta_forme_non_valide() {
        let indirizzo = da_chiave(&CHIAVE);
        let corpo = indirizzo.strip_suffix(SUFFISSO).unwrap();
        assert_eq!(in_chiave(corpo), Err(ErroreIndirizzo::SuffissoMancante));
        assert_eq!(in_chiave(&format!("{corpo}.onion")), Err(ErroreIndirizzo::SuffissoMancante));
        assert_eq!(in_chiave("abc.nyct"), Err(ErroreIndirizzo::LunghezzaErrata));
        let con_uno = format!("1{}{SUFFISSO}", &corpo[1..]);
        assert_eq!(in_chiave(&con_uno), Err(ErroreIndirizzo::CarattereNonValido));
    }
}
