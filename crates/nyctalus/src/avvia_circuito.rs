// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! `nyctalus avvia-circuito`: proxy SOCKS5 locale con **ragnatela a più
//! circuiti** (punto E, stadio 1).
//!
//! Il demone apre più circuiti in parallelo che condividono il **guard** ma
//! prendono percorsi diversi (nodi intermedi e uscita diversi). Ogni nuova
//! connessione SOCKS viene assegnata a un circuito a rotazione: connessioni
//! concorrenti viaggiano quindi su **strade diverse**. Dentro ciascun circuito
//! le connessioni restano multiplexate con celle ([`crate::cella`], punto D).
//!
//! Lo splitting di un **singolo** flusso su più strade (conflux, con
//! ricomposizione a destinazione) è lo stadio successivo (E2).
//!
//! Prototipo: guard fisso e condiviso fra i circuiti; percorsi scelti a caso
//! dall'elenco firmato; link autenticati solo tramite l'elenco.

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

/// Un circuito attivo del demone: dove mandare le celle in andata e la tabella
/// dei flussi aperti su di esso.
struct Circuito {
    tx_cella: mpsc::Sender<Vec<u8>>,
    flussi: MappaFlussi,
    prossimo_flusso: Mutex<u16>,
}

impl Circuito {
    /// Apre un circuito sul collegamento QUIC verso il guard e avvia i task
    /// scrittore (andata) e lettore (ritorno).
    async fn apri(connessione: &quinn::Connection, percorso: &[NodoRete]) -> Risultato<Arc<Self>> {
        let nodi: Vec<NodoPercorso> = percorso
            .iter()
            .map(|(a, p, _)| Ok(NodoPercorso { indirizzo: codifica_indirizzo(*a)?, pubblica: *p }))
            .collect::<Risultato<_>>()?;
        let circuito = apri_circuito(&nodi, b"apertura")?;
        let chiavi = Arc::new(circuito.chiavi_salto);

        let (mut invio, mut ricezione) = connessione.open_bi().await?;
        scrivi_quadro(&mut invio, &circuito.pacchetto).await?;

        // Scrittore: serializza le celle in andata, nonce = contatore locale.
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
        tokio::spawn(async move {
            let mut n: u64 = 0;
            while let Ok(Some(mut payload)) = leggi_quadro(&mut ricezione).await {
                svolgi(BIT_RISPOSTA | n, &chiavi, &mut payload);
                n += 1;
                if let Some(cella) = Cella::da_byte(&payload) {
                    let inbox = flussi_l.lock().unwrap().get(&cella.flusso).cloned();
                    if let Some(inbox) = inbox {
                        let _ = inbox.send(cella).await;
                    }
                }
            }
        });

        Ok(Arc::new(Self { tx_cella, flussi, prossimo_flusso: Mutex::new(0) }))
    }

    /// Registra un nuovo flusso e restituisce (id, casella di ricezione).
    fn registra_flusso(&self) -> (u16, mpsc::Receiver<Cella>) {
        let mut p = self.prossimo_flusso.lock().unwrap();
        let flusso = *p;
        *p = p.wrapping_add(1);
        let (tx, rx) = mpsc::channel::<Cella>(64);
        self.flussi.lock().unwrap().insert(flusso, tx);
        (flusso, rx)
    }
}

pub async fn avvia_circuito(
    nodi: Vec<NodoRete>,
    circuiti: usize,
    salti: usize,
    socks: SocketAddr,
) -> Risultato<()> {
    if !(SALTI_MIN..=SALTI_MAX).contains(&salti) {
        return Err(format!("--salti deve essere fra {SALTI_MIN} e {SALTI_MAX}").into());
    }
    if !(CIRCUITI_MIN..=CIRCUITI_MAX).contains(&circuiti) {
        return Err(format!("--circuiti deve essere fra {CIRCUITI_MIN} e {CIRCUITI_MAX}").into());
    }
    if nodi.len() < salti {
        return Err(format!("servono almeno {salti} nodi nell'elenco").into());
    }

    // Guard fisso e condiviso: tutti i circuiti partono da qui.
    let (guardiano, _, impronta_guard) = nodi[0];
    let locale: SocketAddr = if guardiano.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse()?;
    let mut endpoint = quinn::Endpoint::client(locale)?;
    endpoint.set_default_client_config(match impronta_guard {
        Some(fp) => crate::tls::config_client(fp)?,
        None => crate::tls::config_client_insicuro()?,
    });
    let connessione = endpoint.connect(guardiano, "nyctalus.invalid")?.await?;

    let percorsi = seleziona_percorsi(&nodi, circuiti, salti)?;
    for (i, p) in percorsi.iter().enumerate() {
        let strada: Vec<String> = p.iter().map(|n| n.0.to_string()).collect();
        println!("  circuito {i}: {}", strada.join(" → "));
    }
    let mut circuiti_attivi = Vec::with_capacity(percorsi.len());
    for percorso in &percorsi {
        circuiti_attivi.push(Circuito::apri(&connessione, percorso).await?);
    }

    let ascoltatore = TcpListener::bind(socks).await?;
    println!(
        "Proxy SOCKS5 su {socks} — ragnatela a {} circuiti (guard condiviso, {salti} salti)",
        circuiti_attivi.len()
    );
    println!("Prova:  curl --socks5-hostname {socks} http://example.com\n");

    let mut rotazione = 0usize;
    loop {
        let (cliente, _) = ascoltatore.accept().await?;
        let circuito = circuiti_attivi[rotazione % circuiti_attivi.len()].clone();
        rotazione = rotazione.wrapping_add(1);
        tokio::spawn(async move {
            let (flusso, rx_in) = circuito.registra_flusso();
            if let Err(e) = gestisci_flusso(cliente, flusso, &circuito.tx_cella, rx_in).await {
                eprintln!("flusso {flusso} chiuso: {e}");
            }
            circuito.flussi.lock().unwrap().remove(&flusso);
        });
    }
}

/// Numero di salti (nodi nel percorso): minimo e massimo consentiti.
/// Il massimo è il limite di Sphinx; sotto i 3 salti non c'è vero anonimato.
pub const SALTI_MIN: usize = 1;
pub const SALTI_MAX: usize = nyctalus_core::circuito::MAX_NODI; // 5
/// Numero di circuiti (strade parallele): minimo e massimo consentiti.
pub const CIRCUITI_MIN: usize = 1;
pub const CIRCUITI_MAX: usize = 8;

/// Sceglie `circuiti` percorsi che partono tutti dal guard (nodi[0]). I nodi
/// non-guard vengono distribuiti fra i circuiti in modo che prendano **strade
/// diverse** finché ci sono nodi a sufficienza; esauriti, si rimescola e si
/// riusa.
fn seleziona_percorsi(nodi: &[NodoRete], circuiti: usize, salti: usize) -> Risultato<Vec<Vec<NodoRete>>> {
    let guard = nodi[0];
    let per_circuito = salti - 1; // salti oltre il guard
    let mut candidati: Vec<usize> = (1..nodi.len()).collect();
    if per_circuito > 0 && candidati.is_empty() {
        return Err("servono più nodi oltre al guard".into());
    }
    mescola(&mut candidati);

    let mut percorsi = Vec::with_capacity(circuiti);
    let mut i = 0usize;
    for _ in 0..circuiti {
        let mut percorso = vec![guard];
        for _ in 0..per_circuito {
            if i >= candidati.len() {
                mescola(&mut candidati);
                i = 0;
            }
            percorso.push(nodi[candidati[i]]);
            i += 1;
        }
        percorsi.push(percorso);
    }
    Ok(percorsi)
}

/// Mescolamento Fisher-Yates con sorgente casuale di sistema.
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

    // Tramite bidirezionale, multiplexato sul circuito.
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
