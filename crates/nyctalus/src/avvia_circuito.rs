// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! `nyctalus avvia-circuito`: proxy SOCKS5 locale che instrada **tutte** le
//! connessioni attraverso un **unico circuito condiviso** a più nodi (punto D).
//!
//! Il circuito (e la sua apertura Sphinx) viene creato una sola volta; ogni
//! connessione SOCKS diventa un *flusso* multiplexato dentro gli strati a
//! cipolla ([`crate::cella`]). I nodi intermedi non distinguono i flussi: per
//! loro è un unico canale di byte cifrati.
//!
//! Un task scrittore serializza le celle in andata (contatore nonce unico) e un
//! task lettore smista le celle di ritorno al flusso giusto. Prototipo: i link
//! fra nodi sono autenticati solo con l'elenco firmato; c'è un solo circuito
//! (nessuna rotazione), e i flussi condividono lo stesso percorso.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use nyctalus_core::circuito::{apri_circuito, NodoPercorso};
use nyctalus_core::cipolla;
use nyctalus_core::socks5::{self, Destinazione, Esito, Richiesta, Saluto};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::cella::{cmd, Cella};
use crate::circuito_rete::{BIT_RISPOSTA, codifica_indirizzo, leggi_quadro, scrivi_quadro};
use crate::Risultato;

/// Un nodo del percorso: indirizzo, chiave X25519 e, se noto dall'elenco
/// firmato, l'impronta TLS da pinnare.
pub type NodoRete = (SocketAddr, [u8; 32], Option<[u8; 32]>);

type MappaFlussi = Arc<Mutex<HashMap<u16, mpsc::Sender<Cella>>>>;

pub async fn avvia_circuito(percorso_rete: Vec<NodoRete>, socks: SocketAddr) -> Risultato<()> {
    if percorso_rete.is_empty() {
        return Err("indicare i nodi del circuito con --nodi o --elenco".into());
    }

    // Apertura dell'unico circuito condiviso.
    let percorso: Vec<NodoPercorso> = percorso_rete
        .iter()
        .map(|(a, p, _)| Ok(NodoPercorso { indirizzo: codifica_indirizzo(*a)?, pubblica: *p }))
        .collect::<Risultato<_>>()?;
    let circuito = apri_circuito(&percorso, b"apertura")?;
    let chiavi = Arc::new(circuito.chiavi_salto);

    let (guardiano, _, impronta_guard) = percorso_rete[0];
    let locale: SocketAddr = if guardiano.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse()?;
    let mut endpoint = quinn::Endpoint::client(locale)?;
    endpoint.set_default_client_config(match impronta_guard {
        Some(fp) => crate::tls::config_client(fp)?,
        None => crate::tls::config_client_insicuro()?,
    });
    let connessione = endpoint.connect(guardiano, "nyctalus.invalid")?.await?;
    let (mut invio, mut ricezione) = connessione.open_bi().await?;
    scrivi_quadro(&mut invio, &circuito.pacchetto).await?;

    // Scrittore: serializza le celle in andata, con contatore nonce unico.
    let (tx_cella, mut rx_cella) = mpsc::channel::<Vec<u8>>(256);
    let chiavi_s = chiavi.clone();
    tokio::spawn(async move {
        let mut n: u64 = 0;
        while let Some(cella) = rx_cella.recv().await {
            let avvolto = cipolla::avvolgi(n, &cella, &chiavi_s);
            if scrivi_quadro(&mut invio, &avvolto).await.is_err() {
                break;
            }
            n += 1;
        }
    });

    // Lettore: smista le celle di ritorno al flusso di competenza.
    let flussi: MappaFlussi = Arc::new(Mutex::new(HashMap::new()));
    let flussi_l = flussi.clone();
    let chiavi_l = chiavi.clone();
    tokio::spawn(async move {
        let mut n: u64 = 0;
        while let Ok(Some(mut payload)) = leggi_quadro(&mut ricezione).await {
            svolgi(BIT_RISPOSTA | n, &chiavi_l, &mut payload);
            n += 1;
            if let Some(cella) = Cella::da_byte(&payload) {
                let inbox = flussi_l.lock().unwrap().get(&cella.flusso).cloned();
                if let Some(inbox) = inbox {
                    let _ = inbox.send(cella).await;
                }
            }
        }
    });

    let ascoltatore = TcpListener::bind(socks).await?;
    println!("Proxy SOCKS5 su {socks} — circuito condiviso a {} nodi", percorso.len());
    println!("Prova:  curl --socks5-hostname {socks} http://example.com\n");

    let mut prossimo_flusso: u16 = 0;
    loop {
        let (cliente, _) = ascoltatore.accept().await?;
        let flusso = prossimo_flusso;
        prossimo_flusso = prossimo_flusso.wrapping_add(1);
        let (tx_in, rx_in) = mpsc::channel::<Cella>(64);
        flussi.lock().unwrap().insert(flusso, tx_in);
        let tx_cella = tx_cella.clone();
        let flussi = flussi.clone();
        tokio::spawn(async move {
            if let Err(e) = gestisci_flusso(cliente, flusso, &tx_cella, rx_in).await {
                eprintln!("flusso {flusso} chiuso: {e}");
            }
            flussi.lock().unwrap().remove(&flusso);
        });
    }
}

async fn gestisci_flusso(
    mut cliente: TcpStream,
    flusso: u16,
    tx_cella: &mpsc::Sender<Vec<u8>>,
    mut rx_in: mpsc::Receiver<Cella>,
) -> Risultato<()> {
    // SOCKS5: saluto + richiesta.
    let saluto = leggi_socks(&mut cliente, Saluto::analizza).await?;
    if !saluto.offre_senza_autenticazione() {
        cliente.write_all(&socks5::risposta_saluto(socks5::METODO_NESSUNO_ACCETTABILE)).await?;
        return Err("il client SOCKS non offre 'nessuna autenticazione'".into());
    }
    cliente.write_all(&socks5::risposta_saluto(socks5::METODO_SENZA_AUTENTICAZIONE)).await?;
    let richiesta = leggi_socks(&mut cliente, Richiesta::analizza).await?;
    let host = match &richiesta.destinazione {
        Destinazione::Nome(n) => n.clone(),
        Destinazione::Ipv4(b) => std::net::Ipv4Addr::from(*b).to_string(),
        Destinazione::Ipv6(b) => std::net::Ipv6Addr::from(*b).to_string(),
    };
    let destinazione = format!("{host}:{}", richiesta.porta);

    // Apri il flusso verso l'uscita e attendi l'esito.
    tx_cella.send(Cella::nuova(flusso, cmd::APRI, destinazione.into_bytes()).in_byte()).await?;
    let aperto = matches!(rx_in.recv().await, Some(c) if c.comando == cmd::APERTO);
    let codice = if aperto { Esito::Riuscito } else { Esito::ConnessioneRifiutata };
    cliente.write_all(&socks5::risposta_richiesta(codice)).await?;
    if !aperto {
        return Ok(());
    }

    // Tramite bidirezionale, multiplexato sul circuito condiviso.
    let (mut app_lettura, mut app_scrittura) = cliente.into_split();
    let andata = async {
        let mut buffer = vec![0u8; 16 * 1024];
        loop {
            let letti = app_lettura.read(&mut buffer).await?;
            if letti == 0 {
                break;
            }
            tx_cella
                .send(Cella::nuova(flusso, cmd::DATI, buffer[..letti].to_vec()).in_byte())
                .await
                .map_err(|_| "circuito chiuso")?;
        }
        let _ = tx_cella.send(Cella::controllo(flusso, cmd::CHIUDI).in_byte()).await;
        Risultato::Ok(())
    };
    let ritorno = async {
        while let Some(cella) = rx_in.recv().await {
            match cella.comando {
                cmd::DATI => app_scrittura.write_all(&cella.dati).await?,
                cmd::CHIUDI => break,
                _ => {}
            }
        }
        let _ = app_scrittura.shutdown().await;
        Risultato::Ok(())
    };
    let (a, b) = tokio::join!(andata, ritorno);
    a?;
    b?;
    Ok(())
}

/// Toglie tutti gli strati dal pacchetto (lato client, ritorno).
fn svolgi(nonce: u64, chiavi: &[[u8; 32]], payload: &mut [u8]) {
    for k in chiavi {
        cipolla::sbuccia(nonce, k, payload);
    }
}

async fn leggi_socks<T>(
    cliente: &mut TcpStream,
    analizza: impl Fn(&[u8]) -> Result<T, socks5::ErroreSocks>,
) -> Risultato<T> {
    let mut buffer = Vec::with_capacity(262);
    loop {
        match analizza(&buffer) {
            Ok(v) => return Ok(v),
            Err(socks5::ErroreSocks::Incompleto) => {}
            Err(altro) => return Err(altro.into()),
        }
        let mut pezzo = [0u8; 262];
        let n = cliente.read(&mut pezzo).await?;
        if n == 0 {
            return Err("il client SOCKS ha chiuso durante la negoziazione".into());
        }
        buffer.extend_from_slice(&pezzo[..n]);
    }
}
