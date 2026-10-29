// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Metriche del demone, lette dal comando `monitor` tramite una porta di
//! controllo locale. Semplici contatori atomici; il formato è testuale e
//! leggibile a occhio.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub struct Metriche {
    pub su: AtomicU64,       // byte applicativi client → Internet
    pub giu: AtomicU64,      // byte applicativi Internet → client
    pub flussi: AtomicU64,   // connessioni attualmente aperte
    pub circuiti: AtomicU64, // circuiti della ragnatela
    inizio: Instant,
}

impl Default for Metriche {
    fn default() -> Self {
        Self::nuovo()
    }
}

impl Metriche {
    pub fn nuovo() -> Self {
        Self {
            su: AtomicU64::new(0),
            giu: AtomicU64::new(0),
            flussi: AtomicU64::new(0),
            circuiti: AtomicU64::new(0),
            inizio: Instant::now(),
        }
    }

    /// Istantanea testuale (una metrica per riga).
    pub fn snapshot(&self) -> String {
        let r = Ordering::Relaxed;
        format!(
            "circuits: {}\nflows: {}\nup: {} bytes\ndown: {} bytes\nuptime: {} s\n",
            self.circuiti.load(r),
            self.flussi.load(r),
            self.su.load(r),
            self.giu.load(r),
            self.inizio.elapsed().as_secs(),
        )
    }
}
