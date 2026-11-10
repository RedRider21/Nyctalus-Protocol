// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! **Copertura costante** (PIANO-ANONIMATO, invisibilità punto 4): mentre usi la rete (o sempre,
//! a scelta) il traffico va a **ritmo fisso**, R celle al secondo in ciascun verso: a ogni colpo
//! esce una cella vera se c'è, altrimenti una finta della stessa misura. Chi osserva la linea vede
//! un flusso piatto: non capisce quando scrivi, parli o scarichi, né quanto. Il prezzo: la banda di
//! R celle al secondo anche quando non serve, e la velocità limitata a R celle al secondo (circa
//! R × 1,3 KB/s). Il client regola l'andata; l'uscita, su richiesta del client (cella `COSTANTE`),
//! il ritorno.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;

/// Limiti del ritmo (celle al secondo).
pub const RITMO_MIN: u32 = 5;
pub const RITMO_MAX: u32 = 2000;

/// Ritmo chiesto: celle al secondo e «sempre» (anche da fermi) oppure solo durante l'uso.
pub type Ritmo = Arc<Mutex<Option<(u32, bool)>>>;

/// Il ritmo scelto per questo client: `--constant-rate N` / `NYCTALUS_COSTANTE`, `--constant-always`
/// / `NYCTALUS_COSTANTE_SEMPRE=1`.
static SCELTO: std::sync::OnceLock<Option<(u32, bool)>> = std::sync::OnceLock::new();

pub fn imposta(celle_s: u32, sempre: bool) {
    let _ = SCELTO.set(Some((celle_s.clamp(RITMO_MIN, RITMO_MAX), sempre)));
}

pub fn scelto() -> Option<(u32, bool)> {
    *SCELTO.get_or_init(|| {
        let n: u32 = std::env::var("NYCTALUS_COSTANTE").ok()?.parse().ok()?;
        let sempre = std::env::var("NYCTALUS_COSTANTE_SEMPRE").is_ok_and(|v| v == "1");
        Some((n.clamp(RITMO_MIN, RITMO_MAX), sempre))
    })
}

/// Contenuto della cella di controllo `COSTANTE`: ritmo (2 byte) e modo (1 = durante l'uso, 2 = sempre).
pub fn richiesta(celle_s: u32, sempre: bool) -> Vec<u8> {
    let r = celle_s.clamp(RITMO_MIN, RITMO_MAX) as u16;
    vec![(r >> 8) as u8, r as u8, if sempre { 2 } else { 1 }]
}

pub fn leggi_richiesta(d: &[u8]) -> Option<(u32, bool)> {
    let [a, b, m, ..] = d else { return None };
    let r = u16::from_be_bytes([*a, *b]) as u32;
    (r > 0).then(|| (r.clamp(RITMO_MIN, RITMO_MAX), *m == 2))
}

/// Regolatore fra chi produce le celle (`rx`) e chi le spedisce (`tx`). Senza ritmo le passa così
/// come arrivano; con il ritmo, a ogni colpo una vera o (`finta()`) una finta. Nel modo «durante
/// l'uso», a flussi chiusi (`attivi` = 0) torna a passarle come arrivano.
pub async fn regola(
    mut rx: mpsc::Receiver<Vec<u8>>,
    tx: mpsc::Sender<Vec<u8>>,
    ritmo: Ritmo,
    attivi: Arc<AtomicUsize>,
    finta: impl Fn() -> Vec<u8>,
) {
    let mut colpi: Option<(u32, tokio::time::Interval)> = None;
    loop {
        let cfg = *ritmo.lock().unwrap();
        let attivo = cfg.is_some_and(|(_, sempre)| sempre || attivi.load(Ordering::Relaxed) > 0);
        if !attivo {
            colpi = None;
            // senza ritmo (o da fermi): passa le celle come arrivano, ma ricontrolla ogni tanto
            match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
                Ok(Some(c)) => {
                    if tx.send(c).await.is_err() {
                        return;
                    }
                }
                Ok(None) => return,
                Err(_) => {}
            }
            continue;
        }
        let (r, _) = cfg.expect("attivo");
        if colpi.as_ref().is_none_or(|(rr, _)| *rr != r) {
            let mut i = tokio::time::interval(Duration::from_secs_f64(1.0 / r as f64));
            i.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            colpi = Some((r, i));
        }
        colpi.as_mut().expect("colpi").1.tick().await;
        let cella = match rx.try_recv() {
            Ok(c) => c,
            Err(TryRecvError::Empty) => finta(),
            Err(TryRecvError::Disconnected) => return,
        };
        if tx.send(cella).await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn ritmo_fisso_con_finte_e_vere_in_ordine() {
        let (tx_in, rx_in) = mpsc::channel(64);
        let (tx_out, mut rx_out) = mpsc::channel(4096);
        let ritmo: Ritmo = Arc::new(Mutex::new(Some((100, true))));
        let attivi = Arc::new(AtomicUsize::new(0));
        tokio::spawn(regola(rx_in, tx_out, ritmo.clone(), attivi, || b"F".to_vec()));
        for i in 0..5u8 {
            tx_in.send(vec![i]).await.unwrap();
        }
        tokio::time::sleep(Duration::from_millis(1000)).await;
        let mut viste = Vec::new();
        while let Ok(c) = rx_out.try_recv() {
            viste.push(c);
        }
        // ~100 celle in un secondo (ritmo fisso), le 5 vere per prime e in ordine, poi finte
        assert!((80..=110).contains(&viste.len()), "celle in 1 s: {}", viste.len());
        assert_eq!(&viste[..5], &[vec![0], vec![1], vec![2], vec![3], vec![4]]);
        assert!(viste[5..].iter().all(|c| c == b"F"));
        // modo «durante l'uso» senza flussi: nessuna finta
        *ritmo.lock().unwrap() = Some((100, false));
        tokio::time::sleep(Duration::from_millis(300)).await;
        while rx_out.try_recv().is_ok() {}
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(rx_out.try_recv().is_err(), "da fermi nessuna cella finta");
        assert_eq!(leggi_richiesta(&richiesta(300, true)), Some((300, true)));
        assert_eq!(leggi_richiesta(&richiesta(99_999, false)), Some((RITMO_MAX, false)));
    }
}
