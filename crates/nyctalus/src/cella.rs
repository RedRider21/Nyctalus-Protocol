// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Celle multiplexate dentro un circuito condiviso (punto D).
//!
//! Un **solo** circuito trasporta più connessioni dell'applicazione, distinte
//! da un identificativo di flusso. La cella vive **dentro** gli strati a
//! cipolla: i nodi intermedi vedono solo byte cifrati, la interpretano solo il
//! client e l'uscita del circuito.
//!
//! Formato: `[flusso u16 BE][comando u8][dati…]`.

/// Comandi di una cella.
pub mod cmd {
    /// client → uscita: apri un flusso verso `dati` = "host:porta".
    pub const APRI: u8 = 0x01;
    /// entrambe le direzioni: dati applicativi.
    pub const DATI: u8 = 0x02;
    /// entrambe le direzioni: il flusso è terminato.
    pub const CHIUDI: u8 = 0x03;
    /// uscita → client: flusso aperto con successo.
    pub const APERTO: u8 = 0x10;
    /// uscita → client: apertura del flusso fallita.
    pub const RIFIUTATO: u8 = 0x11;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cella {
    pub flusso: u16,
    pub comando: u8,
    pub dati: Vec<u8>,
}

impl Cella {
    pub fn nuova(flusso: u16, comando: u8, dati: Vec<u8>) -> Self {
        Self { flusso, comando, dati }
    }

    pub fn controllo(flusso: u16, comando: u8) -> Self {
        Self { flusso, comando, dati: Vec::new() }
    }

    pub fn in_byte(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(3 + self.dati.len());
        v.extend_from_slice(&self.flusso.to_be_bytes());
        v.push(self.comando);
        v.extend_from_slice(&self.dati);
        v
    }

    pub fn da_byte(byte: &[u8]) -> Option<Self> {
        if byte.len() < 3 {
            return None;
        }
        Some(Self {
            flusso: u16::from_be_bytes([byte[0], byte[1]]),
            comando: byte[2],
            dati: byte[3..].to_vec(),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn andata_e_ritorno() {
        let c = Cella::nuova(7, cmd::DATI, b"ciao".to_vec());
        assert_eq!(Cella::da_byte(&c.in_byte()), Some(c));
    }

    #[test]
    fn cella_di_controllo_senza_dati() {
        let c = Cella::controllo(3, cmd::CHIUDI);
        let b = c.in_byte();
        assert_eq!(b, vec![0, 3, cmd::CHIUDI]);
        assert_eq!(Cella::da_byte(&b).unwrap().dati, Vec::<u8>::new());
    }

    #[test]
    fn troppo_corta_e_none() {
        assert_eq!(Cella::da_byte(&[0, 1]), None);
    }
}
