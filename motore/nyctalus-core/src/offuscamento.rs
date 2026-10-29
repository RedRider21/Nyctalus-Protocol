// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Offuscamento del trasporto (camuffamento, SPECIFICA §8).
//!
//! Avvolge ogni **datagramma UDP** con un cifrario a flusso (ChaCha20) e una
//! chiave condivisa dalla rete, così che sul filo i byte appaiano **casuali**:
//! niente firma di QUIC riconoscibile da un sistema di ispezione profonda (DPI).
//! È l'equivalente, semplificato, di un "pluggable transport" stile obfs4.
//!
//! Formato: `[nonce 12 byte casuali][ChaCha20(chiave, nonce, datagramma)]`.
//! Il nonce è in chiaro ma casuale, quindi l'insieme è indistinguibile da
//! rumore. Non c'è autenticazione: il contenuto reale è già cifrato e
//! autenticato da QUIC sotto; qui si nasconde solo la **forma** del protocollo.
//!
//! NOTA: questo offuscamento non resiste al "probing attivo" né modella i tempi
//! (shaping); sono raffinamenti successivi. Qui si toglie la firma passiva.

use chacha20::cipher::{KeyIvInit, StreamCipher};
use chacha20::{ChaCha20, Key, Nonce};

use crate::chiavi;

pub const LUNGHEZZA_NONCE: usize = 12;
const CONTESTO: &str = "Nyctalus v0 2026-10-11 chiave di offuscamento del trasporto";

/// Deriva la chiave di offuscamento da un segreto di rete (così la chiave del
/// cifrario non coincide con il segreto grezzo configurato).
pub fn chiave_offuscamento(segreto_rete: &[u8; 32]) -> [u8; 32] {
    blake3::derive_key(CONTESTO, segreto_rete)
}

fn applica(chiave: &[u8; 32], nonce: &[u8; LUNGHEZZA_NONCE], dati: &mut [u8]) {
    let mut cifrario = ChaCha20::new(&Key::from(*chiave), &Nonce::from(*nonce));
    cifrario.apply_keystream(dati);
}

/// Offusca un datagramma: restituisce `nonce ‖ cifrato`. `nonce_casuale` deve
/// essere imprevedibile (12 byte dal generatore di sistema).
pub fn offusca(chiave: &[u8; 32], nonce_casuale: [u8; LUNGHEZZA_NONCE], datagramma: &[u8]) -> Vec<u8> {
    let mut uscita = Vec::with_capacity(LUNGHEZZA_NONCE + datagramma.len());
    uscita.extend_from_slice(&nonce_casuale);
    uscita.extend_from_slice(datagramma);
    applica(chiave, &nonce_casuale, &mut uscita[LUNGHEZZA_NONCE..]);
    uscita
}

/// Toglie l'offuscamento. `None` se il datagramma è troppo corto per contenere
/// il nonce (pacchetto estraneo).
pub fn deoffusca(chiave: &[u8; 32], offuscato: &[u8]) -> Option<Vec<u8>> {
    if offuscato.len() < LUNGHEZZA_NONCE {
        return None;
    }
    let (nonce, cifrato) = offuscato.split_at(LUNGHEZZA_NONCE);
    let nonce: [u8; LUNGHEZZA_NONCE] = nonce.try_into().ok()?;
    let mut dati = cifrato.to_vec();
    applica(chiave, &nonce, &mut dati);
    Some(dati)
}

/// Azzera la chiave di offuscamento quando non serve più.
pub fn azzera(chiave: &mut [u8; 32]) {
    chiavi::azzera(chiave);
}

#[cfg(test)]
mod test {
    use super::*;

    const SEGRETO: [u8; 32] = [5; 32];

    #[test]
    fn andata_e_ritorno() {
        let k = chiave_offuscamento(&SEGRETO);
        let dato = b"un pacchetto QUIC qualunque".to_vec();
        let off = offusca(&k, [7; LUNGHEZZA_NONCE], &dato);
        assert_eq!(off.len(), dato.len() + LUNGHEZZA_NONCE);
        assert_ne!(&off[LUNGHEZZA_NONCE..], &dato[..]); // il corpo è cifrato
        assert_eq!(deoffusca(&k, &off).unwrap(), dato);
    }

    #[test]
    fn nonce_diversi_output_diversi() {
        let k = chiave_offuscamento(&SEGRETO);
        let d = b"stesso dato".to_vec();
        assert_ne!(offusca(&k, [1; 12], &d), offusca(&k, [2; 12], &d));
    }

    #[test]
    fn chiave_sbagliata_non_recupera() {
        let k = chiave_offuscamento(&SEGRETO);
        let altra = chiave_offuscamento(&[9; 32]);
        let off = offusca(&k, [3; 12], b"segreto");
        assert_ne!(deoffusca(&altra, &off).unwrap(), b"segreto");
    }

    #[test]
    fn troppo_corto_e_none() {
        let k = chiave_offuscamento(&SEGRETO);
        assert_eq!(deoffusca(&k, &[0u8; 4]), None);
    }
}
