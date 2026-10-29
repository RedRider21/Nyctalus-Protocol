// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Livello a cipolla (L1), parte "data-plane": gli strati simmetrici che
//! ogni pacchetto attraversa una volta aperto il circuito (SPECIFICA §4).
//!
//! **Cosa fa questo modulo.** Il carico utile è già un pacchetto L2, cioè
//! cifrato e autenticato da un estremo all'altro ([`crate::cifratura`]).
//! Sopra di esso L1 aggiunge uno strato di solo cifrario a flusso (ChaCha20)
//! per ogni nodo del percorso. Serve a un unico scopo: far sì che gli stessi
//! byte appaiano **diversi su ogni tratta**, così un osservatore che vede il
//! pacchetto entrare in un nodo e uscirne non può collegare le due cose
//! confrontando i byte. L'integrità end-to-end resta compito di L2.
//!
//! **Perché il cifrario a flusso e non un AEAD per strato.** Un AEAD
//! aggiungerebbe un tag (16 byte) per ogni strato, e la lunghezza del
//! pacchetto cambierebbe di nodo in nodo rivelando a che punto del percorso
//! ci si trova. Il cifrario a flusso non cambia la lunghezza: tutti i
//! pacchetti restano identici in dimensione a ogni tratta.
//!
//! **Nonce = numero di sequenza del pacchetto.** Ogni pacchetto porta il suo
//! keystream, indipendente dagli altri: così i frammenti possono arrivare in
//! disordine dalla ragnatela senza rompere nulla (a differenza di un
//! keystream unico e continuo per l'intero circuito, che richiederebbe
//! l'ordine). Le chiavi di salto sono diverse per ogni nodo, quindi lo stesso
//! nonce su nodi diversi non è un riuso.
//!
//! **Correttezza.** Applicare uno strato è uno XOR con il keystream della
//! chiave del nodo; lo XOR è commutativo, quindi il pacchetto si ricompone se
//! ogni nodo del percorso toglie la propria chiave, in qualunque ordine.
//!
//! ## Cosa NON è ancora qui (da fare)
//! - **Apertura del circuito** (Sphinx): come ogni nodo riceve la propria
//!   chiave di salto e l'indirizzo del nodo successivo. Vive nel piano di
//!   controllo, non qui.
//! - **Identificativo di circuito che cambia a ogni tratta**, per non far
//!   collegare le tratte tramite un ID costante.
//! - **Il numero di sequenza è in chiaro e uguale su ogni tratta:** da solo è
//!   un possibile aggancio per la correlazione lungo una singola tratta. La
//!   difesa (rumore di fondo e ritardi, SPECIFICA §7) è un livello a parte.

use chacha20::cipher::{KeyIvInit, StreamCipher};
use chacha20::{ChaCha20, Key, Nonce};

use crate::chiavi;

/// Chiave simmetrica di un salto (la stessa nozione delle chiavi di L2).
pub type ChiaveSalto = [u8; 32];

/// Applica il keystream della chiave al buffer, in posizione (XOR).
/// Applicare due volte la stessa chiave con lo stesso `seq` annulla l'effetto.
fn applica_strato(chiave: &ChiaveSalto, seq: u64, buffer: &mut [u8]) {
    let mut nonce = [0u8; 12];
    nonce[..8].copy_from_slice(&seq.to_le_bytes());
    let mut cifrario = ChaCha20::new(&Key::from(*chiave), &Nonce::from(nonce));
    cifrario.apply_keystream(buffer);
}

/// Lato client: avvolge il carico utile in uno strato per ogni chiave di
/// salto. La lunghezza non cambia. L'ordine delle chiavi non conta per la
/// correttezza (XOR commutativo), ma per chiarezza si passano dal primo
/// all'ultimo nodo del percorso.
pub fn avvolgi(seq: u64, carico: &[u8], chiavi_salto: &[ChiaveSalto]) -> Vec<u8> {
    let mut pacchetto = carico.to_vec();
    for chiave in chiavi_salto {
        applica_strato(chiave, seq, &mut pacchetto);
    }
    pacchetto
}

/// Lato nodo: toglie il proprio strato dal pacchetto, in posizione.
pub fn sbuccia(seq: u64, chiave: &ChiaveSalto, pacchetto: &mut [u8]) {
    applica_strato(chiave, seq, pacchetto);
}

/// Deriva le chiavi di salto di un circuito dai segreti condivisi con ogni
/// nodo (che nella rete vera arriveranno dall'apertura del circuito). Contesto
/// BLAKE3 distinto: la chiave di salto non coincide con altre chiavi ricavate
/// dallo stesso segreto.
pub fn chiave_salto(segreto_con_nodo: &[u8; 32]) -> ChiaveSalto {
    chiavi::deriva("Nyctalus v0 2026-09-27 chiave di salto cipolla", segreto_con_nodo, 0)
}

#[cfg(test)]
mod test {
    use super::*;

    fn chiavi(n: usize) -> Vec<ChiaveSalto> {
        (0..n).map(|i| chiave_salto(&[i as u8 + 1; 32])).collect()
    }

    #[test]
    fn andata_e_ritorno_su_tre_nodi() {
        let percorso = chiavi(3);
        let originale = b"pacchetto L2 gia' cifrato e autenticato".to_vec();
        let mut pacchetto = avvolgi(7, &originale, &percorso);
        assert_ne!(pacchetto, originale);
        assert_eq!(pacchetto.len(), originale.len());
        // Ogni nodo del percorso toglie il suo strato, in ordine.
        for chiave in &percorso {
            sbuccia(7, chiave, &mut pacchetto);
        }
        assert_eq!(pacchetto, originale);
    }

    #[test]
    fn ogni_tratta_mostra_byte_diversi() {
        let percorso = chiavi(3);
        let originale = vec![0u8; 64];
        let mut dopo_avvolgimento = avvolgi(1, &originale, &percorso);
        let tratta0 = dopo_avvolgimento.clone(); // client -> nodo 0
        sbuccia(1, &percorso[0], &mut dopo_avvolgimento);
        let tratta1 = dopo_avvolgimento.clone(); // nodo 0 -> nodo 1
        sbuccia(1, &percorso[1], &mut dopo_avvolgimento);
        let tratta2 = dopo_avvolgimento.clone(); // nodo 1 -> nodo 2
        assert_ne!(tratta0, tratta1);
        assert_ne!(tratta1, tratta2);
        assert_ne!(tratta0, tratta2);
    }

    #[test]
    fn pacchetti_indipendenti_tra_loro() {
        let percorso = chiavi(2);
        let carico = vec![0xABu8; 32];
        // Stesso carico, sequenza diversa -> byte diversi (nessun keystream ripetuto).
        assert_ne!(avvolgi(1, &carico, &percorso), avvolgi(2, &carico, &percorso));
    }

    #[test]
    fn l_ordine_di_sbucciatura_non_conta() {
        let percorso = chiavi(3);
        let originale = b"prova".to_vec();
        let mut a = avvolgi(5, &originale, &percorso);
        // Toglie gli strati in ordine inverso: deve funzionare lo stesso.
        for chiave in percorso.iter().rev() {
            sbuccia(5, chiave, &mut a);
        }
        assert_eq!(a, originale);
    }

    #[test]
    fn una_chiave_sbagliata_non_ricompone() {
        let percorso = chiavi(2);
        let originale = b"prova".to_vec();
        let mut pacchetto = avvolgi(0, &originale, &percorso);
        sbuccia(0, &percorso[0], &mut pacchetto);
        sbuccia(0, &chiave_salto(&[99; 32]), &mut pacchetto); // chiave errata
        assert_ne!(pacchetto, originale);
    }
}
