// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! `nyctalus avvia-circuito`: proxy SOCKS5 con **ragnatela conflux** (punto E2).
//!
//! Il demone apre più circuiti che condividono **guard e uscita** ma passano
//! per nodi intermedi diversi. Le celle di **tutte** le connessioni vengono
//! numerate con una sequenza end-to-end e **spezzate** sui vari circuiti; la
//! destinazione le **ricompone** nell'ordine originale ([`crate::conflux`]).
//! Il numero di sequenza viaggia dentro gli strati a cipolla: i relè non lo
//! vedono. Così un singolo scaricamento sfrutta la banda di più strade.
//!
//! Prototipo: guard e uscita fissi per la sessione; nessuna gestione della
//! morte di un circuito (un circuito perso blocca la ricomposizione).

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
use crate::conflux::{self, Riordina};
use crate::Risultato;

/// Un nodo del percorso: indirizzo, chiave X25519 e, se noto, l'impronta TLS.
pub type NodoRete = (SocketAddr, [u8; 32], Option<[u8; 32]>);

type Chiavi = Arc<Vec<[u8; 32]>>;
type MappaFlussi = Arc<Mutex<HashMap<u16, mpsc::Sender<Cella>>>>;

pub const SALTI_MIN: usize = 1;
pub const SALTI_MAX: usize = nyctalus_core::circuito::MAX_NODI; // 5
pub const CIRCUITI_MIN: usize = 1;
pub const CIRCUITI_MAX: usize = 8;

pub async fn avvia_circuito(
    nodi: Vec<NodoRete>,
    circuiti: usize,
    salti: usize,
    socks: SocketAddr,
    controllo: SocketAddr,
    offusca: Option<[u8; 32]>,
) -> Risultato<()> {
    if !(SALTI_MIN..=SALTI_MAX).contains(&salti) {
        return Err(format!("--hops must be between {SALTI_MIN} and {SALTI_MAX}").into());
    }
    if !(CIRCUITI_MIN..=CIRCUITI_MAX).contains(&circuiti) {
        return Err(format!("--circuits must be between {CIRCUITI_MIN} and {CIRCUITI_MAX}").into());
    }
    if nodi.len() < salti {
        return Err(format!("need at least {salti} nodes in the directory").into());
    }

    // Identificativo del gruppo conflux e percorsi (guard + uscita condivisi).
    let mut set_id = [0u8; 16];
    getrandom::fill(&mut set_id).expect("generatore casuale di sistema");
    let percorsi = seleziona_conflux(&nodi, circuiti, salti)?;
    for (i, p) in percorsi.iter().enumerate() {
        let strada: Vec<String> = p.iter().map(|n| n.0.to_string()).collect();
        println!("  circuit {i}: {}", strada.join(" → "));
    }

    // Un solo collegamento QUIC verso il guard condiviso.
    let (guardiano, _, impronta_guard) = percorsi[0][0];
    let locale: SocketAddr = if guardiano.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse()?;
    let mut endpoint = crate::trasporto::endpoint_client(locale, offusca)?;
    endpoint.set_default_client_config(match impronta_guard {
        Some(fp) => crate::tls::config_client(fp)?,
        None => crate::tls::config_client_insicuro()?,
    });
    let connessione = endpoint.connect(guardiano, "nyctalus.invalid")?.await?;

    // Canali condivisi: scrittore(i) in andata, ritorno riordinato.
    let flussi: MappaFlussi = Arc::new(Mutex::new(HashMap::new()));
    let (tx_out, rx_out) = mpsc::channel::<Vec<u8>>(256); // celle in uscita (senza seq)
    let (tx_ret, rx_ret) = mpsc::channel::<(u64, Vec<u8>)>(256); // unità di ritorno

    // Apre i circuiti e raccoglie le caselle dei loro scrittori.
    let mut inbox_circuiti = Vec::with_capacity(percorsi.len());
    for percorso in &percorsi {
        let nodi_sphinx: Vec<NodoPercorso> = percorso
            .iter()
            .map(|(a, p, _)| Ok(NodoPercorso { indirizzo: codifica_indirizzo(*a)?, pubblica: *p }))
            .collect::<Risultato<_>>()?;
        let circuito = apri_circuito(&nodi_sphinx, b"nyctalus")?;
        let chiavi: Chiavi = Arc::new(circuito.chiavi_salto);
        let (mut invio, ricezione) = connessione.open_bi().await?;
        scrivi_quadro(&mut invio, &circuito.pacchetto).await?;

        let (tx_unit, rx_unit) = mpsc::channel::<Vec<u8>>(256);
        tokio::spawn(scrittore_circuito(invio, chiavi.clone(), set_id, rx_unit));
        tokio::spawn(lettore_circuito(ricezione, chiavi, tx_ret.clone()));
        inbox_circuiti.push(tx_unit);
    }

    tokio::spawn(schedulatore(rx_out, inbox_circuiti));
    tokio::spawn(collettore_ritorno(rx_ret, flussi.clone()));

    // Metriche + porta di controllo per `monitor`.
    let metriche = Arc::new(crate::metriche::Metriche::nuovo());
    metriche.circuiti.store(percorsi.len() as u64, std::sync::atomic::Ordering::Relaxed);
    avvia_controllo(controllo, metriche.clone());

    let ascoltatore = TcpListener::bind(socks).await?;
    println!(
        "SOCKS5 proxy on {socks} — conflux over {} circuits (shared guard and exit, {salti} hops)",
        percorsi.len()
    );
    println!("Monitor:  nyctalus monitor --control {controllo}");
    println!("Try:  curl --socks5-hostname {socks} http://example.com\n");

    // Nodi utilizzabili come punto d'incontro (e come tappe del circuito verso
    // di esso) per i siti `.nyct`: lo stesso elenco firmato che alimenta i
    // circuiti d'uscita.
    let punti_incontro: Arc<Vec<crate::incontro::Punto>> = Arc::new(nodi.clone());

    let mut prossimo_flusso: u16 = 0;
    loop {
        let (cliente, _) = ascoltatore.accept().await?;
        let flusso = prossimo_flusso;
        prossimo_flusso = prossimo_flusso.wrapping_add(1);
        let (tx_in, rx_in) = mpsc::channel::<Cella>(64);
        flussi.lock().unwrap().insert(flusso, tx_in);
        let tx_out = tx_out.clone();
        let flussi = flussi.clone();
        let metriche = metriche.clone();
        let punti_incontro = punti_incontro.clone();
        tokio::spawn(async move {
            if let Err(e) = gestisci_flusso(cliente, flusso, &tx_out, rx_in, &metriche, &punti_incontro, offusca, salti).await {
                eprintln!("flow {flusso} closed: {e}");
            }
            flussi.lock().unwrap().remove(&flusso);
        });
    }
}

/// Porta di controllo locale: a ogni connessione invia un'istantanea delle
/// metriche e chiude. Se non riesce a mettersi in ascolto avvisa e prosegue.
fn avvia_controllo(indirizzo: SocketAddr, metriche: Arc<crate::metriche::Metriche>) {
    tokio::spawn(async move {
        let ascoltatore = match TcpListener::bind(indirizzo).await {
            Ok(a) => a,
            Err(e) => {
                eprintln!("control port {indirizzo} unavailable: {e} (monitor disabled)");
                return;
            }
        };
        while let Ok((mut cliente, _)) = ascoltatore.accept().await {
            let snapshot = metriche.snapshot();
            let _ = cliente.write_all(snapshot.as_bytes()).await;
            let _ = cliente.shutdown().await;
        }
    });
}

/// Scrittore di un circuito: prima il LINK (set_id) poi le unità conflux, ognuna
/// avvolta negli strati a cipolla con nonce = contatore locale.
async fn scrittore_circuito(mut invio: quinn::SendStream, chiavi: Chiavi, set_id: [u8; 16], mut rx: mpsc::Receiver<Vec<u8>>) {
    let mut n: u64 = 0;
    let link = cipolla::avvolgi(n, &set_id, &chiavi);
    if scrivi_quadro(&mut invio, &link).await.is_err() {
        return;
    }
    n += 1;
    while let Some(unita) = rx.recv().await {
        let avvolto = cipolla::avvolgi(n, &unita, &chiavi);
        if scrivi_quadro(&mut invio, &avvolto).await.is_err() {
            break;
        }
        n += 1;
    }
}

/// Lettore di un circuito: toglie gli strati e passa (seq, cella) al collettore.
async fn lettore_circuito(mut ricezione: quinn::RecvStream, chiavi: Chiavi, tx_ret: mpsc::Sender<(u64, Vec<u8>)>) {
    let mut n: u64 = 0;
    while let Ok(Some(mut payload)) = leggi_quadro(&mut ricezione).await {
        svolgi(BIT_RISPOSTA | n, &chiavi, &mut payload);
        n += 1;
        if let Some((seq, cella)) = conflux::spacchetta(&payload)
            && tx_ret.send((seq, cella)).await.is_err()
        {
            break;
        }
    }
}

/// Assegna la sequenza end-to-end e spezza le celle sui circuiti a rotazione.
async fn schedulatore(mut rx_out: mpsc::Receiver<Vec<u8>>, inbox: Vec<mpsc::Sender<Vec<u8>>>) {
    let mut seq: u64 = 0;
    while let Some(cella) = rx_out.recv().await {
        let unita = conflux::impacchetta(seq, &cella);
        let i = (seq as usize) % inbox.len();
        if inbox[i].send(unita).await.is_err() {
            break;
        }
        seq += 1;
    }
}

/// Ricompone le unità di ritorno e consegna le celle al flusso giusto.
async fn collettore_ritorno(mut rx_ret: mpsc::Receiver<(u64, Vec<u8>)>, flussi: MappaFlussi) {
    let mut riordina = Riordina::nuovo();
    while let Some((seq, cella_bytes)) = rx_ret.recv().await {
        for cb in riordina.inserisci(seq, cella_bytes) {
            if let Some(cella) = Cella::da_byte(&cb) {
                let inbox = flussi.lock().unwrap().get(&cella.flusso).cloned();
                if let Some(inbox) = inbox {
                    let _ = inbox.send(cella).await;
                }
            }
        }
    }
}

/// Sceglie `circuiti` percorsi con **guard e uscita condivisi** e nodi
/// intermedi diversi.
fn seleziona_conflux(nodi: &[NodoRete], circuiti: usize, salti: usize) -> Risultato<Vec<Vec<NodoRete>>> {
    let guard = nodi[0];
    if salti == 1 {
        // Guard = uscita: nessun anonimato, ma valido.
        return Ok(vec![vec![guard]; circuiti]);
    }
    let uscita = nodi[1];
    let interni = salti - 2;
    let mut candidati: Vec<usize> = (2..nodi.len()).collect();
    if interni > 0 && candidati.is_empty() {
        return Err("need middle nodes besides guard and exit".into());
    }
    mescola(&mut candidati);

    let mut percorsi = Vec::with_capacity(circuiti);
    let mut i = 0usize;
    for _ in 0..circuiti {
        let mut percorso = vec![guard];
        for _ in 0..interni {
            if i >= candidati.len() {
                mescola(&mut candidati);
                i = 0;
            }
            percorso.push(nodi[candidati[i]]);
            i += 1;
        }
        percorso.push(uscita);
        percorsi.push(percorso);
    }
    Ok(percorsi)
}

fn mescola<T>(v: &mut [T]) {
    for i in (1..v.len()).rev() {
        let mut b = [0u8; 8];
        getrandom::fill(&mut b).expect("generatore casuale di sistema");
        let j = (u64::from_le_bytes(b) % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
}

async fn gestisci_flusso(
    mut cliente: TcpStream,
    flusso: u16,
    tx_out: &mpsc::Sender<Vec<u8>>,
    mut rx_in: mpsc::Receiver<Cella>,
    metriche: &Arc<crate::metriche::Metriche>,
    punti_incontro: &Arc<Vec<crate::incontro::Punto>>,
    offusca: Option<[u8; 32]>,
    salti: usize,
) -> Risultato<()> {
    use std::sync::atomic::Ordering::Relaxed;
    let saluto = leggi_socks(&mut cliente, Saluto::analizza).await?;
    if !saluto.offre_senza_autenticazione() {
        cliente.write_all(&socks5::risposta_saluto(socks5::METODO_NESSUNO_ACCETTABILE)).await?;
        return Err("the SOCKS client offers no 'no authentication' method".into());
    }
    cliente.write_all(&socks5::risposta_saluto(socks5::METODO_SENZA_AUTENTICAZIONE)).await?;
    let richiesta = leggi_socks(&mut cliente, Richiesta::analizza).await?;

    // Siti interni `.nyct`: non passano dall'uscita del circuito, ma dal punto
    // d'incontro con sessione cifrata end-to-end verso il servizio.
    if richiesta.destinazione.e_interna() {
        if let Destinazione::Nome(nome) = &richiesta.destinazione {
            metriche.flussi.fetch_add(1, Relaxed);
            let _guardia = GuardiaFlusso(metriche);
            return crate::incontro::tunnel_socks(cliente, nome, (**punti_incontro).clone(), offusca, salti).await;
        }
    }

    let host = match &richiesta.destinazione {
        Destinazione::Nome(n) => n.clone(),
        Destinazione::Ipv4(b) => std::net::Ipv4Addr::from(*b).to_string(),
        Destinazione::Ipv6(b) => std::net::Ipv6Addr::from(*b).to_string(),
    };
    let destinazione = format!("{host}:{}", richiesta.porta);

    tx_out.send(Cella::nuova(flusso, cmd::APRI, destinazione.into_bytes()).in_byte()).await?;
    let aperto = matches!(rx_in.recv().await, Some(c) if c.comando == cmd::APERTO);
    let codice = if aperto { Esito::Riuscito } else { Esito::ConnessioneRifiutata };
    cliente.write_all(&socks5::risposta_richiesta(codice)).await?;
    if !aperto {
        return Ok(());
    }

    metriche.flussi.fetch_add(1, Relaxed);
    let _guardia = GuardiaFlusso(metriche);

    let (mut app_lettura, mut app_scrittura) = cliente.into_split();
    let andata = async {
        let mut buffer = vec![0u8; 16 * 1024];
        loop {
            let letti = app_lettura.read(&mut buffer).await?;
            if letti == 0 {
                break;
            }
            metriche.su.fetch_add(letti as u64, Relaxed);
            tx_out
                .send(Cella::nuova(flusso, cmd::DATI, buffer[..letti].to_vec()).in_byte())
                .await
                .map_err(|_| "circuit closed")?;
        }
        let _ = tx_out.send(Cella::controllo(flusso, cmd::CHIUDI).in_byte()).await;
        Risultato::Ok(())
    };
    let ritorno = async {
        while let Some(cella) = rx_in.recv().await {
            match cella.comando {
                cmd::DATI => {
                    metriche.giu.fetch_add(cella.dati.len() as u64, Relaxed);
                    app_scrittura.write_all(&cella.dati).await?;
                }
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

/// Decrementa il contatore dei flussi aperti quando la connessione termina.
struct GuardiaFlusso<'a>(&'a Arc<crate::metriche::Metriche>);
impl Drop for GuardiaFlusso<'_> {
    fn drop(&mut self) {
        self.0.flussi.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Toglie tutti gli strati dal pacchetto (lato client).
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
            return Err("the SOCKS client closed during negotiation".into());
        }
        buffer.extend_from_slice(&pezzo[..n]);
    }
}
