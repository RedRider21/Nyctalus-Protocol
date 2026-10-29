// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! `monitor`: cruscotto testuale che interroga la porta di controllo del
//! demone (stile `nyx` per Tor) e ne mostra le metriche, aggiornandole ogni
//! secondo.

use std::io::Write;
use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

use crate::Risultato;

pub async fn monitor(controllo: SocketAddr) -> Risultato<()> {
    loop {
        print!("\x1b[2J\x1b[H"); // pulisci schermo
        println!("Nyctalus monitor — {controllo}   (Ctrl+C to quit)\n");
        match TcpStream::connect(controllo).await {
            Ok(mut s) => {
                let mut buf = String::new();
                let _ = s.read_to_string(&mut buf).await;
                print!("{buf}");
            }
            Err(e) => println!("daemon not reachable: {e}"),
        }
        let _ = std::io::stdout().flush();
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
