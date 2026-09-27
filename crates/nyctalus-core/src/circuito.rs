// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Apertura del circuito (livello a cipolla L1, piano di controllo).
//!
//! Prima di poter spedire dati sulla ragnatela, il client deve concordare con
//! **ogni** nodo del percorso una chiave di salto (quella usata da
//! [`crate::cipolla`]), senza che nessun nodo scopra l'intero percorso.
//!
//! Per questo si usa **Sphinx** (Danezis–Goldberg), nell'implementazione
//! collaudata e usata in produzione di Nym (`sphinx-packet`). Da un'unica
//! chiave effimera Sphinx ricava, tramite "blinding", un segreto condiviso
//! diverso per ogni nodo; l'intestazione resta di **dimensione costante** a
//! ogni tratta, quindi non rivela né la lunghezza del percorso né la posizione
//! del nodo. Ogni nodo, con la propria chiave privata, ricava lo stesso
//! segreto e scopre **solo** l'indirizzo del nodo successivo.
//!
//! Qui non si improvvisa crittografia: la matematica del blinding e la cifratura
//! dell'intestazione sono della libreria. Da parte nostra deriviamo la chiave
//! di salto ChaCha20 dal "seme" per-nodo che Sphinx espone (`payload_key_seed`),
//! con BLAKE3. Client e nodo ottengono lo stesso seme, quindi la stessa chiave.
//!
//! ## Cosa resta da fare (documentato)
//! - **Trasporto:** spedire il pacchetto di apertura ai nodi e raccogliere le
//!   conferme vive nel demone, non qui.
//! - **1c — ID di circuito per tratta:** l'associazione tra pacchetti dati e
//!   circuito, con un identificativo che cambia a ogni salto.

use std::fmt;

use sphinx_packet::header::delays::Delay;
use sphinx_packet::header::keys::KeyMaterial;
use sphinx_packet::packet::builder::SphinxPacketBuilder;
use sphinx_packet::route::{Destination, DestinationAddressBytes, Node, NodeAddressBytes};
use sphinx_packet::{ProcessedPacketData, SphinxPacket};
use x25519_dalek::{PublicKey, StaticSecret};

use crate::cipolla::ChiaveSalto;

/// Massimo numero di nodi in un percorso (vincolo di `sphinx-packet`).
pub const MAX_NODI: usize = 5;
const CONTESTO_CHIAVE: &str = "Nyctalus v0 2026-09-27 chiave di salto da seme Sphinx";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErroreCircuito(String);

impl fmt::Display for ErroreCircuito {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "apertura del circuito fallita: {}", self.0)
    }
}
impl std::error::Error for ErroreCircuito {}
impl From<sphinx_packet::Error> for ErroreCircuito {
    fn from(e: sphinx_packet::Error) -> Self {
        Self(e.to_string())
    }
}

/// Un nodo del percorso, come lo conosce il client.
#[derive(Debug, Clone)]
pub struct NodoPercorso {
    /// Indirizzo di rete del nodo (16..32 byte utili, riempito a 32).
    pub indirizzo: [u8; 32],
    /// Chiave pubblica X25519 del nodo.
    pub pubblica: [u8; 32],
}

/// Risultato dell'apertura: le chiavi di salto ordinate (per
/// [`crate::cipolla::avvolgi`]) e il pacchetto Sphinx da spedire al 1º nodo.
pub struct CircuitoAperto {
    pub chiavi_salto: Vec<ChiaveSalto>,
    pub pacchetto: Vec<u8>,
}

fn chiave_da_seme(seme: &[u8; 16]) -> ChiaveSalto {
    blake3::derive_key(CONTESTO_CHIAVE, seme)
}

/// Chiave pubblica X25519 corrispondente a un segreto: un nodo la pubblica
/// come propria identità di circuito, il client la usa in [`NodoPercorso`].
pub fn chiave_pubblica(segreto: &[u8; 32]) -> [u8; 32] {
    PublicKey::from(&StaticSecret::from(*segreto)).to_bytes()
}

/// Genera un nuovo segreto X25519 per l'identità di circuito di un nodo.
pub fn genera_segreto() -> [u8; 32] {
    StaticSecret::random().to_bytes()
}

/// Lato client: sceglie le chiavi di salto e costruisce il pacchetto di
/// apertura per il percorso dato.
pub fn apri_circuito(percorso: &[NodoPercorso], messaggio: &[u8]) -> Result<CircuitoAperto, ErroreCircuito> {
    if percorso.is_empty() {
        return Err(ErroreCircuito("percorso vuoto".into()));
    }
    if percorso.len() > MAX_NODI {
        return Err(ErroreCircuito(format!("percorso oltre {MAX_NODI} nodi")));
    }
    let nodi: Vec<Node> = percorso
        .iter()
        .map(|n| Node::new(NodeAddressBytes::from_bytes(n.indirizzo), PublicKey::from(n.pubblica)))
        .collect();

    // Un'unica chiave effimera per l'intero percorso (proprietà di Sphinx).
    let effimera = StaticSecret::random();
    let materiale = KeyMaterial::derive(&nodi, &effimera);
    let chiavi_salto = materiale
        .expanded_shared_secrets
        .iter()
        .map(|e| chiave_da_seme(e.payload_key_seed()))
        .collect();

    let destinazione = Destination::new(
        DestinationAddressBytes::from_bytes(percorso[percorso.len() - 1].indirizzo),
        [0u8; 16],
    );
    let ritardi: Vec<Delay> = percorso.iter().map(|_| Delay::new_from_nanos(0)).collect();
    let pacchetto = SphinxPacketBuilder::new()
        .with_initial_secret(&effimera)
        .build_packet(messaggio, &nodi, &destinazione, &ritardi)?
        .to_bytes();

    Ok(CircuitoAperto { chiavi_salto, pacchetto })
}

/// Cosa deve fare il nodo dopo aver elaborato il pacchetto di apertura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PassoNodo {
    /// Inoltrare il pacchetto (già trasformato) al nodo successivo.
    Inoltra { prossimo_indirizzo: [u8; 32], pacchetto: Vec<u8> },
    /// Questo nodo è l'ultimo del percorso.
    Finale,
}

pub struct ElaborazioneNodo {
    /// La chiave di salto concordata con questo nodo (per [`crate::cipolla`]).
    pub chiave_salto: ChiaveSalto,
    pub passo: PassoNodo,
}

/// Lato nodo: con la propria chiave privata ricava la chiave di salto e scopre
/// il nodo successivo (o che è l'ultimo). Non apprende nient'altro del percorso.
pub fn elabora_nodo(pacchetto: &[u8], segreto_nodo: &[u8; 32]) -> Result<ElaborazioneNodo, ErroreCircuito> {
    let pacchetto = SphinxPacket::from_bytes(pacchetto)?;
    let segreto = StaticSecret::from(*segreto_nodo);

    // Stesso seme per-nodo che ha calcolato il client → stessa chiave.
    let seme = pacchetto.header.compute_expanded_shared_secret(&segreto);
    let chiave_salto = chiave_da_seme(seme.payload_key_seed());

    let elaborato = pacchetto.process(&segreto)?;
    let passo = match elaborato.data {
        ProcessedPacketData::ForwardHop { next_hop_packet, next_hop_address, .. } => {
            PassoNodo::Inoltra {
                prossimo_indirizzo: next_hop_address.to_bytes(),
                pacchetto: next_hop_packet.to_bytes(),
            }
        }
        ProcessedPacketData::FinalHop { .. } => PassoNodo::Finale,
    };
    Ok(ElaborazioneNodo { chiave_salto, passo })
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::cipolla;

    /// Crea `n` nodi con chiavi X25519 e indirizzi distinti.
    fn nodi(n: usize) -> (Vec<[u8; 32]>, Vec<NodoPercorso>) {
        let mut segreti = Vec::new();
        let mut percorso = Vec::new();
        for i in 0..n {
            let segreto = StaticSecret::random();
            let sb = segreto.to_bytes();
            let pubblica = PublicKey::from(&segreto).to_bytes();
            let mut indirizzo = [0u8; 32];
            indirizzo[0] = (i + 1) as u8;
            segreti.push(sb);
            percorso.push(NodoPercorso { indirizzo, pubblica });
        }
        (segreti, percorso)
    }

    #[test]
    fn client_e_nodi_concordano_le_chiavi_e_il_percorso() {
        let (segreti, percorso) = nodi(3);
        let circuito = apri_circuito(&percorso, b"apertura").unwrap();
        assert_eq!(circuito.chiavi_salto.len(), 3);

        // Nodo 0
        let e0 = elabora_nodo(&circuito.pacchetto, &segreti[0]).unwrap();
        assert_eq!(e0.chiave_salto, circuito.chiavi_salto[0]);
        let PassoNodo::Inoltra { prossimo_indirizzo, pacchetto: p1 } = e0.passo else {
            panic!("il nodo 0 dovrebbe inoltrare");
        };
        assert_eq!(prossimo_indirizzo, percorso[1].indirizzo);

        // Nodo 1
        let e1 = elabora_nodo(&p1, &segreti[1]).unwrap();
        assert_eq!(e1.chiave_salto, circuito.chiavi_salto[1]);
        let PassoNodo::Inoltra { prossimo_indirizzo, pacchetto: p2 } = e1.passo else {
            panic!("il nodo 1 dovrebbe inoltrare");
        };
        assert_eq!(prossimo_indirizzo, percorso[2].indirizzo);

        // Nodo 2 (ultimo)
        let e2 = elabora_nodo(&p2, &segreti[2]).unwrap();
        assert_eq!(e2.chiave_salto, circuito.chiavi_salto[2]);
        assert_eq!(e2.passo, PassoNodo::Finale);
    }

    #[test]
    fn le_chiavi_di_salto_sono_diverse_per_nodo() {
        let (_, percorso) = nodi(3);
        let c = apri_circuito(&percorso, b"x").unwrap();
        assert_ne!(c.chiavi_salto[0], c.chiavi_salto[1]);
        assert_ne!(c.chiavi_salto[1], c.chiavi_salto[2]);
    }

    #[test]
    fn un_nodo_con_la_chiave_sbagliata_non_apre_il_pacchetto() {
        let (segreti, percorso) = nodi(3);
        let c = apri_circuito(&percorso, b"x").unwrap();
        // Il nodo 1 non può elaborare il pacchetto destinato al nodo 0.
        assert!(elabora_nodo(&c.pacchetto, &segreti[1]).is_err());
    }

    /// 1b + 1a insieme: le chiavi concordate all'apertura ricompongono davvero
    /// un pacchetto dati avvolto a strati (cipolla).
    #[test]
    fn le_chiavi_del_circuito_ricompongono_un_pacchetto_dati() {
        let (segreti, percorso) = nodi(3);
        let circuito = apri_circuito(&percorso, b"apertura").unwrap();

        let originale = b"pacchetto dati end-to-end".to_vec();
        let mut dati = cipolla::avvolgi(1, &originale, &circuito.chiavi_salto);

        // Ogni nodo, ricavata la sua chiave dall'apertura, toglie il suo strato.
        let mut pacchetto_apertura = circuito.pacchetto.clone();
        for segreto in &segreti {
            let e = elabora_nodo(&pacchetto_apertura, segreto).unwrap();
            cipolla::sbuccia(1, &e.chiave_salto, &mut dati);
            if let PassoNodo::Inoltra { pacchetto, .. } = e.passo {
                pacchetto_apertura = pacchetto;
            }
        }
        assert_eq!(dati, originale);
    }
}
