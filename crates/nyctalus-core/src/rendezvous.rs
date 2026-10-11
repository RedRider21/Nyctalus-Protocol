// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Scelta **deterministica** del punto d'incontro di un servizio `.nyct`
//! (SPECIFICA §6), senza bisogno di pubblicare un descrittore.
//!
//! Servizio e client partono dagli **stessi** dati — la chiave del servizio
//! (il suo `.nyct`) e l'elenco firmato dei nodi, uguale per tutti — e calcolano
//! lo **stesso ordine** di nodi candidati. Il servizio si registra al primo
//! raggiungibile; il client prova i candidati nello stesso ordine finché trova
//! il servizio. Nessuno deve indicare il punto d'incontro a mano.
//!
//! L'ordine dipende anche da un'**epoca** (il giorno), così il punto d'incontro
//! ruota nel tempo e un singolo nodo non resta legato per sempre a un servizio.
//! Vicino al cambio di giorno i due lati potrebbero calcolare epoche diverse:
//! il client prova anche l'epoca precedente (vedi [`ordine_con_ricambio`]).

const CONTESTO: &str = "Nyctalus punto d'incontro v0";
const SECONDI_PER_EPOCA: u64 = 24 * 60 * 60;

/// Epoca corrente (giorni dall'epoch UNIX, UTC).
pub fn epoca_attuale() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / SECONDI_PER_EPOCA)
        .unwrap_or(0)
}

/// Punteggio di un nodo per un dato servizio ed epoca (minore = preferito).
fn punteggio(chiave_servizio: &[u8; 32], epoca: u64, indice: usize) -> u64 {
    let mut hasher = blake3::Hasher::new_derive_key(CONTESTO);
    hasher.update(chiave_servizio);
    hasher.update(&epoca.to_le_bytes());
    hasher.update(&(indice as u64).to_le_bytes());
    let digest = hasher.finalize();
    let mut byte = [0u8; 8];
    byte.copy_from_slice(&digest.as_bytes()[..8]);
    u64::from_le_bytes(byte)
}

/// Ordine dei nodi (per indice nell'elenco) in cui provare il punto d'incontro,
/// dal più preferito al meno, per quel servizio e quell'epoca. È una
/// permutazione deterministica di `0..numero_nodi`.
pub fn ordine(chiave_servizio: &[u8; 32], numero_nodi: usize, epoca: u64) -> Vec<usize> {
    let mut punteggi: Vec<(u64, usize)> =
        (0..numero_nodi).map(|i| (punteggio(chiave_servizio, epoca, i), i)).collect();
    punteggi.sort_unstable();
    punteggi.into_iter().map(|(_, i)| i).collect()
}

/// Come [`ordine`] ma unisce l'epoca corrente e quella precedente (senza
/// ripetere i nodi): così il client trova il servizio anche subito dopo il
/// cambio di giorno, quando il servizio potrebbe essere ancora registrato
/// all'epoca vecchia.
pub fn ordine_con_ricambio(chiave_servizio: &[u8; 32], numero_nodi: usize, epoca: u64) -> Vec<usize> {
    let mut risultato = ordine(chiave_servizio, numero_nodi, epoca);
    if epoca > 0 {
        for i in ordine(chiave_servizio, numero_nodi, epoca - 1) {
            if !risultato.contains(&i) {
                risultato.push(i);
            }
        }
    }
    risultato
}

#[cfg(test)]
mod test {
    use super::*;

    const CHIAVE: [u8; 32] = [0x11; 32];

    #[test]
    fn e_una_permutazione() {
        let mut o = ordine(&CHIAVE, 7, 42);
        assert_eq!(o.len(), 7);
        o.sort_unstable();
        assert_eq!(o, (0..7).collect::<Vec<_>>());
    }

    #[test]
    fn e_deterministico() {
        assert_eq!(ordine(&CHIAVE, 10, 5), ordine(&CHIAVE, 10, 5));
    }

    #[test]
    fn dipende_da_chiave_ed_epoca() {
        let mut altra = CHIAVE;
        altra[0] ^= 1;
        assert_ne!(ordine(&CHIAVE, 10, 5), ordine(&altra, 10, 5));
        assert_ne!(ordine(&CHIAVE, 10, 5), ordine(&CHIAVE, 10, 6));
    }

    #[test]
    fn casi_limite() {
        assert!(ordine(&CHIAVE, 0, 1).is_empty());
        assert_eq!(ordine(&CHIAVE, 1, 1), vec![0]);
    }

    #[test]
    fn ricambio_include_entrambe_le_epoche_senza_ripetere() {
        let o = ordine_con_ricambio(&CHIAVE, 5, 100);
        let mut unici = o.clone();
        unici.sort_unstable();
        unici.dedup();
        assert_eq!(o.len(), unici.len()); // nessun doppione
        assert!(o.len() >= 5);
    }
}
