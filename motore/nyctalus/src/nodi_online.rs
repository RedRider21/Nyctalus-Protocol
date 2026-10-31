// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Comando `nodes`: mostra i nodi dell'**elenco firmato** (indirizzo, chiave
//! X25519, impronta TLS) e, con `--probe`, verifica quali sono **raggiungibili
//! ora** aprendo verso ciascuno una connessione QUIC autenticata (la stessa che
//! userebbe un circuito), misurando il tempo di risposta.
//!
//! Serve a scegliere un nodo come **destinazione**: i valori `address` e
//! `fingerprint` stampati si incollano in `single-hop`/`send`.
//!
//! Nota sulla privacy: senza `--probe` non si contatta nessuno (si legge solo
//! l'elenco). Con `--probe` ci si connette davvero ai nodi, che quindi vedono
//! la nostra sonda: è utile come diagnostica, non come azione anonima.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crate::Risultato;

/// Elenca i nodi dell'elenco firmato; se `sonda`, prova a raggiungerli.
pub async fn elenca(
    percorso: &str,
    autorita: [u8; 32],
    offusca: Option<[u8; 32]>,
    sonda: bool,
    scadenza: Duration,
) -> Risultato<()> {
    let testo = std::fs::read_to_string(percorso)?;
    let voci = nyctalus_core::elenco::verifica_elenco(&testo, &autorita)?;
    println!("Signed directory: {percorso}  ({} nodes)", voci.len());
    if voci.is_empty() {
        println!("  (the list is empty)");
        return Ok(());
    }
    if sonda {
        println!("  probing reachability (timeout {:.0}s)…", scadenza.as_secs_f32());
    }
    println!();
    println!("  {:>2}  {:<22}  {:<14}  {:<14}  {}", "#", "address", "x25519", "fingerprint", "status");

    let mut online = 0usize;
    for (i, v) in voci.iter().enumerate() {
        let x = &crate::in_esadecimale(&v.x25519)[..12];
        let fp = &crate::in_esadecimale(&v.impronta_tls)[..12];
        let stato = if sonda {
            match crate::circuito_rete::risolvi_voce(&v.indirizzo, &v.x25519) {
                Ok(addr) => match sonda_nodo(addr, v.impronta_tls, offusca, scadenza).await {
                    Ok(rtt) => {
                        online += 1;
                        format!("● online  {} ms", rtt.as_millis())
                    }
                    Err(e) => format!("○ offline ({e})"),
                },
                Err(_) => "○ bad address".into(),
            }
        } else {
            "listed".into()
        };
        println!("  {:>2}  {:<22}  {:<14}  {:<14}  {}", i + 1, v.indirizzo, x, fp, stato);
    }

    if sonda {
        println!("\n{online}/{} nodes reachable now.", voci.len());
    } else {
        println!("\n(add --probe to test which ones are reachable now)");
    }

    // Come usarne uno come destinazione (i valori sono già quelli giusti).
    let v = &voci[0];
    println!("\nTo target a node, copy its address and fingerprint, e.g.:");
    println!(
        "  nyctalus single-hop --to {} --fingerprint {}",
        v.indirizzo,
        crate::in_esadecimale(&v.impronta_tls)
    );
    Ok(())
}

/// Prova ad aprire una connessione QUIC verso un nodo (fingerprint pinnata) e
/// restituisce il tempo impiegato, oppure il motivo del fallimento.
async fn sonda_nodo(
    addr: SocketAddr,
    impronta: [u8; 32],
    offusca: Option<[u8; 32]>,
    scadenza: Duration,
) -> Result<Duration, String> {
    let locale: SocketAddr = if addr.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }
        .parse()
        .expect("indirizzo locale valido");
    let endpoint = crate::trasporto::endpoint_client(locale, offusca).map_err(|e| e.to_string())?;
    let config = crate::tls::config_client(impronta).map_err(|e| e.to_string())?;
    let inizio = Instant::now();
    let connessione = tokio::time::timeout(scadenza, async {
        endpoint
            .connect_with(config, addr, "nyctalus.invalid")
            .map_err(|e| e.to_string())?
            .await
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "timeout".to_string())??;
    let rtt = inizio.elapsed();
    connessione.close(0u32.into(), b"probe");
    Ok(rtt)
}
