// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Comando `nyctalus prova-circuito`: apre un circuito a più nodi, invia un
//! messaggio fino all'uscita attraverso gli strati a cipolla e stampa la
//! risposta. Dimostra che l'onion routing (tappe 1a+1b+2) funziona davvero in
//! rete: i nodi intermedi vedono solo byte cifrati, l'uscita il messaggio.

use std::net::SocketAddr;

use nyctalus_core::circuito::{apri_circuito, NodoPercorso};
use nyctalus_core::cipolla;

use crate::circuito_rete::{BIT_RISPOSTA, codifica_indirizzo, leggi_quadro, scrivi_quadro};
use crate::Risultato;

pub async fn prova_circuito(percorso_rete: Vec<crate::avvia_circuito::NodoRete>, messaggio: String) -> Risultato<()> {
    if percorso_rete.is_empty() {
        return Err("indicare almeno un nodo con --nodi o --elenco".into());
    }
    let percorso: Vec<NodoPercorso> = percorso_rete
        .iter()
        .map(|(addr, pubblica, _)| Ok(NodoPercorso { indirizzo: codifica_indirizzo(*addr)?, pubblica: *pubblica }))
        .collect::<Risultato<_>>()?;

    let circuito = apri_circuito(&percorso, b"apertura nyctalus")?;
    println!("Circuito a {} nodi aperto.", percorso.len());

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

    // 2) Richiesta di connessione (andata #0): la destinazione "host:porta".
    let destinazione = messaggio; // qui è la destinazione da raggiungere
    let avvolto = cipolla::avvolgi(0, destinazione.as_bytes(), &circuito.chiavi_salto);
    scrivi_quadro(&mut invio, &avvolto).await?;
    println!("Richiesta connessione a {destinazione:?} attraverso il circuito");

    // 3) Esito dall'uscita (ritorno #0): si tolgono tutti gli strati.
    match leggi_quadro(&mut ricezione).await? {
        Some(mut payload) => {
            for chiave in &circuito.chiavi_salto {
                cipolla::sbuccia(BIT_RISPOSTA, chiave, &mut payload);
            }
            let esito = String::from_utf8_lossy(&payload);
            println!("Esito dall'uscita: {esito:?} (OK = connessione aperta)");
        }
        None => return Err("nessuna risposta dall'uscita".into()),
    }
    connessione.close(0u32.into(), b"fine");
    endpoint.wait_idle().await;
    Ok(())
}
