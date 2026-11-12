// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! **Prove a raffica del motore** (piano di sicurezza, punto 7): come quelle del nucleo, sui
//! lettori di ciò che arriva dalla rete nel motore. Nessuno deve far cadere il programma.

#![cfg(test)]

use std::net::SocketAddr;

const GIRI: u64 = 20_000;

/// Generatore deterministico (xorshift64*), come nel nucleo.
struct Caso(u64);

impl Caso {
    fn nuovo(seme: u64) -> Self {
        Self(seme.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn n(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn fino(&mut self, max: usize) -> usize {
        (self.n() % (max as u64 + 1)) as usize
    }
    fn byte(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.n() as u8).collect()
    }
    fn input(&mut self, buoni: &[Vec<u8>], max: usize) -> Vec<u8> {
        if buoni.is_empty() || self.n() % 4 == 0 {
            let n = self.fino(max);
            return self.byte(n);
        }
        let mut v = buoni[self.fino(buoni.len() - 1)].clone();
        match self.n() % 5 {
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
                v[i] = [0u8, 0xff, 0x7f, 0x80][self.fino(3)];
            }
            _ => {
                if v.len() > 2 {
                    let i = self.fino(v.len() - 2);
                    v.remove(i);
                }
            }
        }
        v
    }
}

#[test]
fn raffica_celle_ragnatela_e_riordino() {
    use crate::cella::{cmd, Cella};
    let buoni = vec![
        Cella::nuova(3, cmd::DATI, b"ciao".to_vec()).in_byte(),
        Cella::nuova(crate::cella::FLUSSO_CONTROLLO, cmd::CONFERMA, crate::ripresa::conferma_in_byte(7)).in_byte(),
        Cella::nuova(0, cmd::COSTANTE, crate::costante::richiesta(60, true)).in_byte(),
        crate::conflux::impacchetta(5, &Cella::riempimento().in_byte()),
        crate::conflux::quadro_link(&[7; 16]),
    ];
    let mut c = Caso::nuovo(11);
    let mut r = crate::conflux::Riordina::nuovo();
    let mut d = crate::codifica::Decodificatore::nuovo(8);
    for _ in 0..GIRI {
        let x = c.input(&buoni, 1500);
        if let Some(cella) = Cella::da_byte(&x) {
            let _ = crate::ripresa::conferma_da_byte(&cella.dati);
            let _ = crate::costante::leggi_richiesta(&cella.dati);
        }
        if let Some((seq, cella)) = crate::conflux::spacchetta(&x) {
            // numeri di sequenza qualunque (anche enormi) non devono rompere il riordino
            let _ = r.inserisci(seq, cella.clone());
            let _ = d.inserisci(seq, cella);
        }
        let _ = crate::codifica::leggi_richiesta(&x);
    }
}

#[test]
fn raffica_videochiamata_stun_e_srtp() {
    let k = crate::webrtc::Chiavi::da(&[4; 32]);
    let buoni = vec![
        crate::webrtc::incarta(&k, 1, 2, 3, true, b"un pacchetto QUIC"),
        crate::webrtc::richiesta_stun(&k, "abcd:efgh"),
    ];
    let da: SocketAddr = "203.0.113.9:5000".parse().unwrap();
    let da6: SocketAddr = "[2001:db8::1]:5000".parse().unwrap();
    let mut c = Caso::nuovo(12);
    for _ in 0..GIRI {
        let x = c.input(&buoni, 1500);
        let _ = crate::webrtc::e_stun(&x);
        let _ = crate::webrtc::rispondi_stun(&k, &x, da);
        let _ = crate::webrtc::rispondi_stun(&k, &x, da6);
        let _ = crate::webrtc::scarta_o_apri(&k, &x);
    }
}

#[test]
fn raffica_punto_d_incontro_e_salti() {
    let mut controllo = crate::incontro::MAGIA.to_vec();
    controllo.push(1);
    controllo.extend_from_slice(&[5; 32]);
    controllo.extend_from_slice(&[0; 48]);
    let buoni = vec![
        controllo.clone(),
        crate::incontro::impacchetta_tunnel(&controllo),
        crate::circuito_rete::con_ritardo(crate::circuito_rete::codifica_indirizzo("81.1.2.3:4600".parse().unwrap()).unwrap(), 900).to_vec(),
    ];
    let mut c = Caso::nuovo(13);
    for _ in 0..GIRI {
        let x = c.input(&buoni, 300);
        let _ = crate::incontro::analizza_controllo_provato(&x);
        let _ = crate::incontro::spacchetta_tunnel(&x);
        if x.len() >= 32 {
            let mut a = [0u8; 32];
            a.copy_from_slice(&x[..32]);
            let _ = crate::circuito_rete::decodifica_indirizzo(&a);
            let _ = crate::circuito_rete::ritardo_di(&a);
        }
    }
}
