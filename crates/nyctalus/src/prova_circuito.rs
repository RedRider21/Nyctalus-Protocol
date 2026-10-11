// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Comando `nyctalus prova-circuito`: apre un circuito a più nodi, invia un
//! messaggio fino all'uscita attraverso gli strati a cipolla e stampa la
//! risposta. Dimostra che l'onion routing (tappe 1a+1b+2) funziona davvero in
//! rete: i nodi intermedi vedono solo byte cifrati, l'uscita il messaggio.

use std::net::SocketAddr;

use nyctalus_core::circuito::{apri_circuito, NodoPercorso};
use nyctalus_core::cipolla;

use crate::cella::{cmd, Cella};
use crate::circuito_rete::{BIT_RISPOSTA, codifica_indirizzo, leggi_quadro, scrivi_quadro};
use crate::Risultato;

pub async fn prova_circuito(percorso_rete: Vec<crate::avvia_circuito::NodoRete>, messaggio: String) -> Risultato<()> {
    if percorso_rete.is_empty() {
        return Err("specify at least one node with --nodes or --directory".into());
    }
    let percorso: Vec<NodoPercorso> = percorso_rete
        .iter()
        .map(|(addr, pubblica, _)| Ok(NodoPercorso { indirizzo: codifica_indirizzo(*addr)?, pubblica: *pubblica }))
        .collect::<Risultato<_>>()?;

    let circuito = apri_circuito(&percorso, b"apertura nyctalus")?;
    println!("Circuit with {} nodes opened.", percorso.len());

    // Collegamento al primo nodo (guard), autenticato se l'impronta è nota.
    let (guardiano, _, impronta_guard) = percorso_rete[0];
    let locale: SocketAddr = if guardiano.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse()?;
    let mut endpoint = quinn::Endpoint::client(locale)?;
    let config = match impronta_guard {
        Some(fp) => crate::tls::config_client(fp)?,
        None => crate::tls::config_client_insicuro()?,
    };
    endpoint.set_default_client_config(config);
    let connessione = endpoint.connect(guardiano, "nyctalus.invalid")?.await?;
    let (mut invio, mut ricezione) = connessione.open_bi().await?;

    // 1) Apertura del circuito (1º quadro).
    scrivi_quadro(&mut invio, &circuito.pacchetto).await?;

    // 2) LINK: set_id del gruppo conflux (qui un circuito solo).
    let mut set_id = [0u8; 16];
    getrandom::fill(&mut set_id).expect("generatore casuale di sistema");
    let link = cipolla::avvolgi(0, &set_id, &circuito.chiavi_salto);
    scrivi_quadro(&mut invio, &link).await?;

    // 3) Unità conflux #0 = cella APRI con la destinazione "host:porta".
    let destinazione = messaggio;
    let cella = Cella::nuova(0, cmd::APRI, destinazione.clone().into_bytes()).in_byte();
    let unita = crate::conflux::impacchetta(0, &cella);
    let avvolto = cipolla::avvolgi(1, &unita, &circuito.chiavi_salto);
    scrivi_quadro(&mut invio, &avvolto).await?;
    println!("Connection request to {destinazione:?} through the circuit");

    // 4) Esito dall'uscita (ritorno #0): si tolgono gli strati e si ricompone.
    match leggi_quadro(&mut ricezione).await? {
        Some(mut payload) => {
            for chiave in &circuito.chiavi_salto {
                cipolla::sbuccia(BIT_RISPOSTA, chiave, &mut payload);
            }
            let cella = crate::conflux::spacchetta(&payload).and_then(|(_, c)| Cella::da_byte(&c));
            let esito = match cella.map(|c| c.comando) {
                Some(cmd::APERTO) => "OPENED (connection succeeded)",
                Some(cmd::RIFIUTATO) => "REFUSED (connection failed)",
                _ => "unrecognized reply",
            };
            println!("Outcome from the exit: {esito}");
        }
        None => return Err("no reply from the exit".into()),
    }
    connessione.close(0u32.into(), b"fine");
    endpoint.wait_idle().await;
    Ok(())
}
