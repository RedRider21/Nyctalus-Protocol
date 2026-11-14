// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! **Prove a raffica** (piano di sicurezza, punto 7: fuzzing leggero, senza strumenti esterni). Ogni
//! lettore che riceve dati da fuori riceve decine di migliaia di input: casuali di ogni lunghezza e
//! messaggi VALIDI rovinati (bit invertiti, tagliati, allungati, byte ripetuti). Nessuno deve far
//! cadere il programma (panic): devono solo rispondere «non valido». Generatore deterministico:
//! un caso trovato si rifà identico.

#![cfg(test)]

use crate::{casella, circuito, elenco, indirizzo, offuscamento, socks5, stretta};

/// Generatore deterministico (xorshift64*): niente dipendenze, casi ripetibili.
pub struct Caso(u64);

impl Caso {
    pub fn nuovo(seme: u64) -> Self {
        Self(seme.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn n(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn fino(&mut self, max: usize) -> usize {
        (self.n() % (max as u64 + 1)) as usize
    }
    pub fn byte(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.n() as u8).collect()
    }
    /// Un input: a caso, oppure uno dei `buoni` rovinato in uno dei modi tipici.
    pub fn input(&mut self, buoni: &[Vec<u8>], max: usize) -> Vec<u8> {
        if buoni.is_empty() || self.n() % 4 == 0 {
            let n = self.fino(max);
            return self.byte(n);
        }
        let mut v = buoni[self.fino(buoni.len() - 1)].clone();
        match self.n() % 6 {
            0 if !v.is_empty() => {
                for _ in 0..1 + self.fino(8) {
                    let i = self.fino(v.len() - 1);
                    v[i] ^= 1 << (self.n() % 8);
                }
            }
            1 => {
                let n = self.fino(v.len());
                v.truncate(n);
            }
            2 => {
                let n = self.fino(64);
                let extra = self.byte(n);
                v.extend(extra);
            }
            3 if !v.is_empty() => {
                let i = self.fino(v.len() - 1);
                let b = self.n() as u8;
                for x in v.iter_mut().skip(i).take(self.fino(32)) {
                    *x = b;
                }
            }
            4 if v.len() > 2 => {
                let i = self.fino(v.len() - 2);
                v.remove(i);
            }
            _ => {
                if !v.is_empty() {
                    let i = self.fino(v.len() - 1);
                    v[i] = [0u8, 0xff, 0x7f, 0x80][self.fino(3)];
                }
            }
        }
        v
    }
}

const GIRI: u64 = 20_000;

#[test]
fn raffica_elenco_firmato() {
    let (segreto, pubblica) = elenco::genera_autorita();
    let voci = vec![
        elenco::VoceElenco { indirizzo: "81.10.0.1:4601".into(), x25519: [1; 32], impronta_tls: [2; 32], famiglia: Some("a".into()), banda: Some(10), intermedio: true, guardia: true, senza_uscita: false },
        elenco::VoceElenco { indirizzo: "nodo.esempio.it:4601".into(), x25519: [3; 32], impronta_tls: [4; 32], famiglia: None, banda: None, intermedio: false, guardia: false, senza_uscita: true },
    ];
    let buono = elenco::firma_elenco(&voci, &segreto).into_bytes();
    let mut c = Caso::nuovo(1);
    for _ in 0..GIRI {
        let x = c.input(std::slice::from_ref(&buono), 2000);
        if let Ok(t) = std::str::from_utf8(&x) {
            let _ = elenco::verifica_elenco(t, &pubblica);
        }
        let _ = elenco::verifica_elenco(&String::from_utf8_lossy(&x), &pubblica);
    }
}

#[test]
fn raffica_socks5() {
    let buoni = vec![vec![5, 2, 0, 2], vec![5, 1, 0, 3, 9, b'e', b'x', b'.', b'n', b'y', b'c', b't', b'x', b'y', 0, 80], vec![1, 3, b'm', b'i', b'x', 1, b'x'], vec![5, 1, 0, 1, 1, 2, 3, 4, 0, 80], vec![5, 1, 0, 4].into_iter().chain([0u8; 18]).collect()];
    let mut c = Caso::nuovo(2);
    for _ in 0..GIRI * 3 {
        let x = c.input(&buoni, 300);
        let _ = socks5::Saluto::analizza(&x);
        let _ = socks5::Richiesta::analizza(&x);
        let _ = socks5::analizza_credenziali(&x);
    }
}

#[test]
fn raffica_pacchetto_sphinx() {
    let segreti: Vec<[u8; 32]> = (0..3).map(|_| circuito::genera_segreto()).collect();
    let percorso: Vec<circuito::NodoPercorso> = segreti
        .iter()
        .enumerate()
        .map(|(i, s)| circuito::NodoPercorso { indirizzo: [i as u8 + 4; 32], pubblica: circuito::chiave_pubblica(s) })
        .collect();
    let buono = circuito::apri_circuito(&percorso, b"prova").unwrap().pacchetto;
    let mut c = Caso::nuovo(3);
    for _ in 0..GIRI / 10 {
        let x = c.input(std::slice::from_ref(&buono), 3000);
        let _ = circuito::elabora_nodo(&x, &segreti[0]);
    }
}

#[test]
fn raffica_stretta_noise_e_casella() {
    let servizio = stretta::Identita::genera().unwrap();
    let (_m, primo) = stretta::Mittente::inizia(&servizio.pubblica()).unwrap();
    let mittente = stretta::Identita::genera().unwrap();
    let busta = casella::sigilla(&mittente, &servizio.pubblica(), b"ciao").unwrap();
    let buoni = vec![primo, busta];
    let mut c = Caso::nuovo(4);
    for _ in 0..GIRI / 10 {
        let x = c.input(&buoni, 1500);
        let _ = stretta::rispondi(&servizio, &x);
        let _ = casella::apri(&servizio, &x);
        let _ = stretta::Identita::da_byte(&x);
    }
}

#[test]
fn raffica_offuscamento_e_indirizzi() {
    let k = offuscamento::chiave_offuscamento(&[9; 32]);
    let buono = offuscamento::offusca(&k, [1; offuscamento::LUNGHEZZA_NONCE], b"un datagramma QUIC");
    let ind = indirizzo::da_chiave(&[5; 32]).into_bytes();
    let mut c = Caso::nuovo(5);
    for _ in 0..GIRI {
        let x = c.input(std::slice::from_ref(&buono), 1500);
        let _ = offuscamento::deoffusca(&k, &x);
        let y = c.input(std::slice::from_ref(&ind), 100);
        let _ = indirizzo::in_chiave(&String::from_utf8_lossy(&y));
    }
}
