// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Celle multiplexate dentro un circuito condiviso (punto D) a **lunghezza
//! fissa** (punto H2).
//!
//! Un **solo** circuito trasporta più connessioni dell'applicazione, distinte
//! da un identificativo di flusso. La cella vive **dentro** gli strati a
//! cipolla: i nodi intermedi vedono solo byte cifrati, la interpretano solo il
//! client e l'uscita del circuito.
//!
//! **Lunghezza fissa ([`LUNGHEZZA`]).** Tutte le celle hanno la stessa
//! dimensione sul filo: un osservatore non deduce la lunghezza dei contenuti e,
//! soprattutto, le **celle fittizie** ([`cmd::RIEMPIMENTO`], il rumore di fondo)
//! sono indistinguibili da quelle vere. La vera lunghezza dei dati sta in un
//! campo interno; il resto è riempimento a zero.
//!
//! Formato: `[flusso u16 BE][comando u8][lunghezza u16 LE][dati…][zeri]`.

/// Comandi di una cella.
pub mod cmd {
    /// client → uscita: apri un flusso verso `dati` = "host:porta".
    pub const APRI: u8 = 0x01;
    /// entrambe le direzioni: dati applicativi.
    pub const DATI: u8 = 0x02;
    /// entrambe le direzioni: il flusso è terminato.
    pub const CHIUDI: u8 = 0x03;
    /// entrambe le direzioni: cella **fittizia** (cover traffic), da scartare.
    pub const RIEMPIMENTO: u8 = 0x04;
    /// uscita → client: flusso aperto con successo.
    pub const APERTO: u8 = 0x10;
    /// uscita → client: apertura del flusso fallita.
    pub const RIFIUTATO: u8 = 0x11;
}

/// Byte utili trasportabili in una cella (il resto è riempimento).
pub const CAPACITA: usize = 1024;
const INTESTAZIONE: usize = 2 + 1 + 2; // flusso(2) + comando(1) + lunghezza(2)
/// Lunghezza fissa di una cella serializzata (uguale per tutte).
pub const LUNGHEZZA: usize = INTESTAZIONE + CAPACITA;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cella {
    pub flusso: u16,
    pub comando: u8,
    pub dati: Vec<u8>,
}

impl Cella {
    pub fn nuova(flusso: u16, comando: u8, dati: Vec<u8>) -> Self {
        assert!(dati.len() <= CAPACITA, "i dati di una cella non superano CAPACITA");
        Self { flusso, comando, dati }
    }

    pub fn controllo(flusso: u16, comando: u8) -> Self {
        Self { flusso, comando, dati: Vec::new() }
    }

    /// Cella fittizia per il rumore di fondo: da scartare a destinazione.
    pub fn riempimento() -> Self {
        Self { flusso: 0, comando: cmd::RIEMPIMENTO, dati: Vec::new() }
    }

    /// Serializza a lunghezza **fissa** [`LUNGHEZZA`] (con riempimento a zero).
    pub fn in_byte(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(LUNGHEZZA);
        v.extend_from_slice(&self.flusso.to_be_bytes());
        v.push(self.comando);
        v.extend_from_slice(&(self.dati.len() as u16).to_le_bytes());
        v.extend_from_slice(&self.dati);
        v.resize(LUNGHEZZA, 0);
        v
    }

    /// Legge una cella: usa il campo lunghezza per prendere i dati reali e
    /// ignorare il riempimento. Tollera input della sola intestazione in su.
    pub fn da_byte(byte: &[u8]) -> Option<Self> {
        if byte.len() < INTESTAZIONE {
            return None;
        }
        let flusso = u16::from_be_bytes([byte[0], byte[1]]);
        let comando = byte[2];
        let lunghezza = u16::from_le_bytes([byte[3], byte[4]]) as usize;
        if lunghezza > CAPACITA || INTESTAZIONE + lunghezza > byte.len() {
            return None;
        }
        Some(Self { flusso, comando, dati: byte[INTESTAZIONE..INTESTAZIONE + lunghezza].to_vec() })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn lunghezza_fissa_per_ogni_cella() {
        let a = Cella::nuova(7, cmd::DATI, b"ciao".to_vec());
        let b = Cella::controllo(3, cmd::CHIUDI);
        let c = Cella::riempimento();
        assert_eq!(a.in_byte().len(), LUNGHEZZA);
        assert_eq!(b.in_byte().len(), LUNGHEZZA);
        assert_eq!(c.in_byte().len(), LUNGHEZZA);
    }

    #[test]
    fn andata_e_ritorno() {
        let c = Cella::nuova(7, cmd::DATI, b"ciao".to_vec());
        assert_eq!(Cella::da_byte(&c.in_byte()), Some(c));
    }

    #[test]
    fn il_riempimento_non_rivela_i_dati() {
        let corta = Cella::nuova(1, cmd::DATI, b"x".to_vec()).in_byte();
        let piena = Cella::nuova(1, cmd::DATI, vec![0xAB; CAPACITA]).in_byte();
        assert_eq!(corta.len(), piena.len()); // stessa lunghezza sul filo
    }

    #[test]
    fn cella_di_controllo_senza_dati() {
        let c = Cella::controllo(3, cmd::CHIUDI);
        let b = c.in_byte();
        assert_eq!(b.len(), LUNGHEZZA);
        assert_eq!(Cella::da_byte(&b).unwrap().dati, Vec::<u8>::new());
    }

    #[test]
    fn riempimento_si_riconosce() {
        let c = Cella::riempimento();
        let letta = Cella::da_byte(&c.in_byte()).unwrap();
        assert_eq!(letta.comando, cmd::RIEMPIMENTO);
        assert!(letta.dati.is_empty());
    }

    #[test]
    fn troppo_corta_e_none() {
        assert_eq!(Cella::da_byte(&[0, 1]), None);
    }

    #[test]
    #[should_panic]
    fn dati_oltre_capacita_vietati() {
        Cella::nuova(1, cmd::DATI, vec![0; CAPACITA + 1]);
    }
}
