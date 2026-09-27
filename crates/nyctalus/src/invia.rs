//! Lato mittente: legge il file a blocchi, lo spezza in frammenti cifrati e
//! li distribuisce a turno sulle corsie QUIC, senza mai superare la finestra
//! confermata dal ricevitore.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Instant;

use nyctalus_core::flusso::{MittenteFlusso, ParametriFlusso};
use nyctalus_core::stretta::{LUNGHEZZA_MESSAGGIO, Mittente};
use tokio::io::AsyncReadExt;
use tokio::sync::{mpsc, watch};

use crate::{ID_FLUSSO_DATI, Risultato, tls, velocita};

pub async fn invia(
    destinazione: SocketAddr,
    impronta: [u8; 32],
    destinatario: [u8; 32],
    corsie: usize,
    percorso: PathBuf,
) -> Risultato<()> {
    let locale: SocketAddr =
        if destinazione.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse()?;
    let mut endpoint = quinn::Endpoint::client(locale)?;
    endpoint.set_default_client_config(tls::config_client(impronta)?);
    let connessione = endpoint.connect(destinazione, tls::NOME_SERVER)?.await?;
    println!("Collegato con {destinazione}, {corsie} corsie parallele");

    // Stretta di mano Noise NK: solo chi possiede la chiave privata del
    // destinatario riesce a rispondere.
    let (mittente_stretta, primo) = Mittente::inizia(&destinatario)?;
    let (mut invio_stretta, mut ricezione_stretta) = connessione.open_bi().await?;
    invio_stretta.write_all(&primo).await?;
    invio_stretta.finish()?;
    let mut risposta = [0u8; LUNGHEZZA_MESSAGGIO];
    ricezione_stretta.read_exact(&mut risposta).await?;
    let segreti = mittente_stretta.completa(&risposta)?;
    let segreto = segreti.verso_destinatario;
    println!("Stretta di mano completata: destinatario autenticato, forward secrecy attiva");

    // Conferme dal ricevitore: indice del prossimo frammento che aspetta.
    let mut canale_conferme = connessione.accept_uni().await?;
    let (tx_conferme, mut conferme) = watch::channel(0u64);
    tokio::spawn(async move {
        let mut valore = [0u8; 8];
        while canale_conferme.read_exact(&mut valore).await.is_ok() {
            tx_conferme.send_replace(u64::from_le_bytes(valore));
        }
    });

    // Una coda e un compito di scrittura per ogni corsia.
    let mut code = Vec::with_capacity(corsie);
    let mut compiti = Vec::with_capacity(corsie);
    for _ in 0..corsie {
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(64);
        let mut corsia = connessione.open_uni().await?;
        compiti.push(tokio::spawn(async move {
            while let Some(pacchetto) = rx.recv().await {
                corsia.write_all(&pacchetto).await?;
            }
            corsia.finish()?;
            Risultato::Ok(())
        }));
        code.push(tx);
    }

    let parametri = ParametriFlusso::default();
    let mut mittente = MittenteFlusso::nuovo(&segreto, ID_FLUSSO_DATI, parametri);
    let mut file = tokio::fs::File::open(&percorso).await?;
    let mut blocco = vec![0u8; parametri.dimensione_frammento * 64];
    let mut hash = blake3::Hasher::new();
    let (mut byte, mut indice) = (0u64, 0u64);
    let inizio = Instant::now();

    loop {
        let letti = riempi(&mut file, &mut blocco).await?;
        let fine = letti < blocco.len();
        hash.update(&blocco[..letti]);
        byte += letti as u64;
        for pacchetto in mittente.spezza(&blocco[..letti], fine) {
            // Controllo del flusso: mai oltre la finestra del ricevitore.
            conferme
                .wait_for(|&confermato| indice < confermato + parametri.finestra)
                .await
                .map_err(|_| "il ricevitore ha chiuso il canale delle conferme")?;
            code[(indice % corsie as u64) as usize]
                .send(pacchetto)
                .await
                .map_err(|_| "una corsia si è chiusa durante l'invio")?;
            indice += 1;
        }
        if fine {
            break;
        }
    }

    drop(code);
    for compito in compiti {
        compito.await??;
    }
    let totale = indice;
    conferme
        .wait_for(|&confermato| confermato >= totale)
        .await
        .map_err(|_| "il ricevitore non ha confermato la fine del trasferimento")?;
    let secondi = inizio.elapsed().as_secs_f64();
    connessione.close(0u32.into(), b"fine");
    endpoint.wait_idle().await;

    println!("Inviato {} ({byte} byte) in {secondi:.2} s, {}", percorso.display(), velocita(byte, secondi));
    println!("  pacchetti: {totale}");
    println!("  BLAKE3 del file: {}", hash.finalize().to_hex());
    Ok(())
}

/// Riempie `buffer` fino in fondo o fino alla fine del file; restituisce
/// quanti byte ha letto (meno della capienza solo a fine file).
async fn riempi(file: &mut tokio::fs::File, buffer: &mut [u8]) -> Risultato<usize> {
    let mut letti = 0;
    while letti < buffer.len() {
        let n = file.read(&mut buffer[letti..]).await?;
        if n == 0 {
            break;
        }
        letti += n;
    }
    Ok(letti)
}
