// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! **Ripresa dai circuiti caduti** (scaletta punto 5). Nella ragnatela conflux le unità numerate
//! viaggiano su più strade (circuiti) e l'altro capo le rimette in ordine. Se una strada cade (un
//! nodo intermedio si spegne), le unità che erano in viaggio su di essa si perdono e il riordino le
//! aspetterebbe per sempre. Qui ogni capo:
//! - tiene in **memoria** le unità spedite finché l'altro capo non le **conferma** (cella `CONFERMA`:
//!   «ho tutto, in ordine, fino a N»);
//! - quando una strada cade la toglie dal giro e **rispedisce** sulle altre le sue unità non
//!   confermate (i doppioni il riordino li scarta già);
//! - se in quel momento non ci sono strade, le unità aspettano la strada nuova.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::conflux;

/// Unità non ancora assegnate a una strada (nessuna strada viva al momento dell'invio).
const NESSUNA: u64 = u64::MAX;
/// Unità non confermate tenute al massimo (oltre: le più vecchie si lasciano, ~22 MB).
const MEMORIA_MAX: usize = 16_384;

pub type Lista = Arc<Mutex<Vec<(u64, mpsc::Sender<Vec<u8>>)>>>;

pub struct Strade {
    /// strade vive: (id, coda dello scrittore)
    pub lista: Lista,
    /// seq → (unità, id della strada su cui è partita)
    memoria: Mutex<BTreeMap<u64, (Vec<u8>, u64)>>,
    prossimo_id: AtomicU64,
}

impl Strade {
    pub fn nuove() -> Arc<Self> {
        Arc::new(Self { lista: Arc::new(Mutex::new(Vec::new())), memoria: Mutex::new(BTreeMap::new()), prossimo_id: AtomicU64::new(0) })
    }

    pub fn aggiungi(&self, tx: mpsc::Sender<Vec<u8>>) -> u64 {
        let id = self.prossimo_id.fetch_add(1, Ordering::Relaxed);
        self.lista.lock().unwrap().push((id, tx));
        id
    }

    pub fn vive(&self) -> usize {
        self.lista.lock().unwrap().len()
    }

    /// Memorizza e spedisce l'unità `seq` sulla strada più libera. `false` se non ce n'è nessuna
    /// (l'unità resta in memoria e partirà con la prossima strada).
    pub async fn spedisci(&self, seq: u64, unita: Vec<u8>) -> bool {
        loop {
            let scelta = {
                let l = self.lista.lock().unwrap();
                if l.is_empty() {
                    None
                } else {
                    let liberi: Vec<usize> = l.iter().map(|(_, t)| t.capacity()).collect();
                    let (id, tx) = &l[conflux::scegli_strada(&liberi, seq as usize)];
                    Some((*id, tx.clone()))
                }
            };
            let Some((id, tx)) = scelta else {
                self.memorizza(seq, unita, NESSUNA);
                return false;
            };
            self.memorizza(seq, unita.clone(), id);
            if tx.send(unita.clone()).await.is_ok() {
                return true;
            }
            // strada morta senza che nessuno se ne sia accorto: via, e si riprova con un'altra
            self.lista.lock().unwrap().retain(|(i, _)| *i != id);
        }
    }

    fn memorizza(&self, seq: u64, unita: Vec<u8>, id: u64) {
        let mut m = self.memoria.lock().unwrap();
        m.insert(seq, (unita, id));
        while m.len() > MEMORIA_MAX {
            m.pop_first();
        }
    }

    /// L'altro capo ha tutto fino a `fino` compreso: quelle unità non servono più.
    pub fn conferma(&self, fino: u64) {
        let mut m = self.memoria.lock().unwrap();
        *m = m.split_off(&(fino.saturating_add(1)));
    }

    pub fn in_memoria(&self) -> usize {
        self.memoria.lock().unwrap().len()
    }

    /// La strada `id` è caduta: la toglie e rispedisce le unità non confermate che non hanno più
    /// una strada viva. Restituisce quante ne ha rispedite.
    pub async fn caduta(&self, id: u64) -> usize {
        self.lista.lock().unwrap().retain(|(i, _)| *i != id);
        self.rispedisci_orfane().await
    }

    /// Rispedisce (in ordine) le unità la cui strada non c'è più.
    pub async fn rispedisci_orfane(&self) -> usize {
        let vive: Vec<u64> = self.lista.lock().unwrap().iter().map(|(i, _)| *i).collect();
        if vive.is_empty() {
            return 0;
        }
        let orfane: Vec<(u64, Vec<u8>)> = self
            .memoria
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, (_, id))| !vive.contains(id))
            .map(|(s, (u, _))| (*s, u.clone()))
            .collect();
        let n = orfane.len();
        for (seq, unita) in orfane {
            self.spedisci(seq, unita).await;
        }
        n
    }
}

/// Contenuto di una cella `CONFERMA`: il numero fino a cui si è ricevuto tutto.
pub fn conferma_in_byte(fino: u64) -> Vec<u8> {
    fino.to_be_bytes().to_vec()
}

pub fn conferma_da_byte(d: &[u8]) -> Option<u64> {
    Some(u64::from_be_bytes(d.get(..8)?.try_into().ok()?))
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn caduta_rispedisce_le_non_confermate_sulle_altre_strade() {
        let s = Strade::nuove();
        let (tx_a, mut rx_a) = mpsc::channel(64);
        let (tx_b, mut rx_b) = mpsc::channel(64);
        let a = s.aggiungi(tx_a);
        s.aggiungi(tx_b);
        for seq in 0..10u64 {
            s.spedisci(seq, vec![seq as u8]).await;
        }
        let mut su_a = Vec::new();
        while let Ok(u) = rx_a.try_recv() {
            su_a.push(u[0]);
        }
        while rx_b.try_recv().is_ok() {}
        assert!(!su_a.is_empty(), "il distributore usa entrambe le strade");
        // l'altro capo conferma fino a 3: quelle non si rispediscono
        s.conferma(3);
        assert_eq!(s.in_memoria(), 6);
        // cade la strada A: le sue unità non confermate passano su B, in ordine
        let n = s.caduta(a).await;
        let attese: Vec<u8> = su_a.iter().copied().filter(|x| *x > 3).collect();
        assert_eq!(n, attese.len());
        let mut rispedite = Vec::new();
        while let Ok(u) = rx_b.try_recv() {
            rispedite.push(u[0]);
        }
        assert_eq!(rispedite, attese);
        // nessuna strada: si tiene in memoria e si rispedisce con la strada nuova
        drop(rx_b);
        s.lista.lock().unwrap().clear();
        assert!(!s.spedisci(10, vec![10]).await);
        let (tx_c, mut rx_c) = mpsc::channel(64);
        s.aggiungi(tx_c);
        s.rispedisci_orfane().await;
        let mut su_c = Vec::new();
        while let Ok(u) = rx_c.try_recv() {
            su_c.push(u[0]);
        }
        assert_eq!(*su_c.last().unwrap(), 10, "l'unità senza strada parte con la strada nuova");
        assert!(su_c.windows(2).all(|w| w[0] < w[1]), "in ordine");
        assert_eq!(conferma_da_byte(&conferma_in_byte(77)), Some(77));
    }
}
