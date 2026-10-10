// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Elenco firmato dei nodi (directory), SPECIFICA §9.
//!
//! Nell'onion routing i nodi conoscono solo l'indirizzo del vicino, non la sua
//! identità: per **autenticare** i collegamenti serve una fonte comune che dica
//! "a questo indirizzo corrisponde questa chiave". È l'elenco dei nodi, firmato
//! da un'**autorità** (chiave Ed25519). Client e nodi lo leggono, verificano la
//! firma e ne ricavano, per ogni nodo: indirizzo, chiave X25519 (per i circuiti
//! Sphinx) e impronta del certificato TLS (per autenticare il collegamento QUIC).
//!
//! Formato testuale (una riga per nodo), semplice e verificabile a occhio:
//! ```text
//! nyctalus-elenco v0
//! v 127.0.0.1:4601 <x25519_hex> <impronta_tls_hex>
//! v ...
//! firma <firma_hex>
//! ```
//! La firma copre le righe `v ...` (in ordine), con un contesto di versione.

use std::fmt;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

const INTESTAZIONE: &str = "nyctalus-elenco v0";
const CONTESTO_FIRMA: &[u8] = b"Nyctalus elenco nodi v0\n";

/// Un nodo nell'elenco.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoceElenco {
    pub indirizzo: String,
    pub x25519: [u8; 32],
    pub impronta_tls: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroreElenco {
    Malformato(String),
    FirmaMancante,
    FirmaNonValida,
}

impl fmt::Display for ErroreElenco {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformato(m) => write!(f, "elenco malformato: {m}"),
            Self::FirmaMancante => f.write_str("elenco senza firma"),
            Self::FirmaNonValida => f.write_str("firma dell'elenco non valida"),
        }
    }
}
impl std::error::Error for ErroreElenco {}

/// Genera una nuova autorità: (seme segreto, chiave pubblica). Il seme va
/// custodito; la pubblica si distribuisce a client e nodi.
pub fn genera_autorita() -> ([u8; 32], [u8; 32]) {
    // 32 byte casuali dal generatore di sistema (via x25519-dalek) come seme.
    let seme = x25519_dalek::StaticSecret::random().to_bytes();
    let pubblica = SigningKey::from_bytes(&seme).verifying_key().to_bytes();
    (seme, pubblica)
}

/// Chiave pubblica dell'autorità dato il suo seme segreto.
pub fn autorita_pubblica(seme_segreto: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(seme_segreto).verifying_key().to_bytes()
}

/// Costruisce e firma un elenco.
pub fn firma_elenco(voci: &[VoceElenco], seme_autorita: &[u8; 32]) -> String {
    let righe = righe_voci(voci);
    let firma = SigningKey::from_bytes(seme_autorita).sign(&messaggio(&righe));
    format!("{INTESTAZIONE}\n{righe}firma {}\n", esadecimale(&firma.to_bytes()))
}

/// Verifica la firma di un elenco e restituisce le voci.
pub fn verifica_elenco(testo: &str, pubblica_autorita: &[u8; 32]) -> Result<Vec<VoceElenco>, ErroreElenco> {
    let mut voci = Vec::new();
    let mut firma: Option<[u8; 64]> = None;
    for riga in testo.lines() {
        let riga = riga.trim();
        if riga.is_empty() || riga == INTESTAZIONE {
            continue;
        }
        if let Some(resto) = riga.strip_prefix("v ") {
            voci.push(analizza_voce(resto)?);
        } else if let Some(resto) = riga.strip_prefix("firma ") {
            let byte = da_esadecimale(resto).ok_or(ErroreElenco::FirmaNonValida)?;
            firma = Some(byte.try_into().map_err(|_| ErroreElenco::FirmaNonValida)?);
        } else {
            return Err(ErroreElenco::Malformato(format!("riga sconosciuta: {riga}")));
        }
    }
    let firma = firma.ok_or(ErroreElenco::FirmaMancante)?;
    let chiave = VerifyingKey::from_bytes(pubblica_autorita).map_err(|_| ErroreElenco::FirmaNonValida)?;
    let righe = righe_voci(&voci);
    chiave
        .verify_strict(&messaggio(&righe), &Signature::from_bytes(&firma))
        .map_err(|_| ErroreElenco::FirmaNonValida)?;
    Ok(voci)
}

fn messaggio(righe: &str) -> Vec<u8> {
    let mut m = CONTESTO_FIRMA.to_vec();
    m.extend_from_slice(righe.as_bytes());
    m
}

fn righe_voci(voci: &[VoceElenco]) -> String {
    let mut s = String::new();
    for v in voci {
        s.push_str(&format!("v {} {} {}\n", v.indirizzo, esadecimale(&v.x25519), esadecimale(&v.impronta_tls)));
    }
    s
}

fn analizza_voce(resto: &str) -> Result<VoceElenco, ErroreElenco> {
    let campi: Vec<&str> = resto.split_whitespace().collect();
    let [indirizzo, x25519, impronta] = campi.as_slice() else {
        return Err(ErroreElenco::Malformato("attesi indirizzo, x25519, impronta".into()));
    };
    let a_32 = |h: &str| -> Result<[u8; 32], ErroreElenco> {
        da_esadecimale(h)
            .and_then(|b| b.try_into().ok())
            .ok_or_else(|| ErroreElenco::Malformato("chiave esadecimale non a 32 byte".into()))
    };
    Ok(VoceElenco { indirizzo: (*indirizzo).to_string(), x25519: a_32(x25519)?, impronta_tls: a_32(impronta)? })
}

fn esadecimale(byte: &[u8]) -> String {
    byte.iter().map(|b| format!("{b:02x}")).collect()
}

fn da_esadecimale(testo: &str) -> Option<Vec<u8>> {
    if !testo.len().is_multiple_of(2) {
        return None;
    }
    (0..testo.len()).step_by(2).map(|i| u8::from_str_radix(&testo[i..i + 2], 16).ok()).collect()
}

#[cfg(test)]
mod test {
    use super::*;

    fn voci() -> Vec<VoceElenco> {
        vec![
            VoceElenco { indirizzo: "127.0.0.1:4601".into(), x25519: [1; 32], impronta_tls: [2; 32] },
            VoceElenco { indirizzo: "127.0.0.1:4602".into(), x25519: [3; 32], impronta_tls: [4; 32] },
        ]
    }

    #[test]
    fn firma_e_verifica() {
        let (segreto, pubblica) = genera_autorita();
        let testo = firma_elenco(&voci(), &segreto);
        assert_eq!(verifica_elenco(&testo, &pubblica).unwrap(), voci());
    }

    #[test]
    fn autorita_sbagliata_rifiutata() {
        let (segreto, _) = genera_autorita();
        let (_, altra_pub) = genera_autorita();
        let testo = firma_elenco(&voci(), &segreto);
        assert_eq!(verifica_elenco(&testo, &altra_pub), Err(ErroreElenco::FirmaNonValida));
    }

    #[test]
    fn manomissione_rifiutata() {
        let (segreto, pubblica) = genera_autorita();
        let testo = firma_elenco(&voci(), &segreto).replace("127.0.0.1:4602", "6.6.6.6:4602");
        assert_eq!(verifica_elenco(&testo, &pubblica), Err(ErroreElenco::FirmaNonValida));
    }

    #[test]
    fn firma_mancante() {
        let (_, pubblica) = genera_autorita();
        let testo = "nyctalus-elenco v0\nv 127.0.0.1:4601 ".to_string()
            + &esadecimale(&[1; 32]) + " " + &esadecimale(&[2; 32]) + "\n";
        assert_eq!(verifica_elenco(&testo, &pubblica), Err(ErroreElenco::FirmaMancante));
    }
}
