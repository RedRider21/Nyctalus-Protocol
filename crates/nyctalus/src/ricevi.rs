//! Lato ricevitore: accetta una connessione, raccoglie i pacchetti da tutte
//! le corsie, li ricompone e conferma l'avanzamento al mittente.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use nyctalus_core::cifratura::lunghezza_pacchetto;
use nyctalus_core::flusso::{ParametriFlusso, RicevitoreFlusso};
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use crate::{ID_FLUSSO_DATI, Risultato, in_esadecimale, tls, velocita};

/// Ogni quanti frammenti consegnati si manda una conferma al mittente.
const PASSO_CONFERME: u64 = 64;

pub async fn ricevi(ascolta: SocketAddr, uscita: PathBuf) -> Risultato<()> {
    let identita = tls::identita_server()?;
    let mut segreto = [0u8; 32];
    getrandom::fill(&mut segreto).map_err(|e| format!("generatore casuale non disponibile: {e}"))?;

    let endpoint = quinn::Endpoint::server(identita.config, ascolta)?;
    println!("In ascolto su {ascolta}. Sull'altro computer esegui:\n");
    println!(
        "  nyctalus invia --a <IP-DI-QUESTO-PC>:{} --impronta {} --segreto {} <FILE>\n",
        ascolta.port(),
        in_esadecimale(&identita.impronta),
        in_esadecimale(&segreto),
    );

    let connessione = endpoint.accept().await.ok_or("endpoint chiuso")?.await?;
    println!("Collegato con {}", connessione.remote_address());

    let parametri = ParametriFlusso::default();
    let lunghezza = lunghezza_pacchetto(parametri.dimensione_frammento);

    // Canale di ritorno per le conferme (controllo del flusso). Il primo
    // valore rende subito visibile il canale al mittente.
    let mut conferme = connessione.open_uni().await?;
    conferme.write_all(&0u64.to_le_bytes()).await?;

    // Ogni corsia aperta dal mittente ha il suo compito di lettura; tutti
    // confluiscono in un'unica coda verso il ricompositore.
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(1024);
    let accettazione = connessione.clone();
    tokio::spawn(async move {
        while let Ok(mut corsia) = accettazione.accept_uni().await {
            let tx = tx.clone();
            tokio::spawn(async move {
                loop {
                    let mut pacchetto = vec![0u8; lunghezza];
                    if corsia.read_exact(&mut pacchetto).await.is_err()
                        || tx.send(pacchetto).await.is_err()
                    {
                        break;
                    }
                }
            });
        }
    });

    let mut ricevitore = RicevitoreFlusso::nuovo(&segreto, ID_FLUSSO_DATI, parametri);
    let mut file = tokio::fs::File::create(&uscita).await?;
    let mut hash = blake3::Hasher::new();
    let (mut byte, mut pacchetti, mut in_anticipo, mut scartati) = (0u64, 0u64, 0u64, 0u64);
    let mut ultima_conferma = 0u64;
    let mut inizio = None;

    while let Some(pacchetto) = rx.recv().await {
        inizio.get_or_insert_with(Instant::now);
        pacchetti += 1;
        match ricevitore.ricevi(&pacchetto) {
            Ok(dati) if dati.is_empty() => in_anticipo += 1,
            Ok(dati) => {
                hash.update(&dati);
                file.write_all(&dati).await?;
                byte += dati.len() as u64;
            }
            Err(errore) => {
                scartati += 1;
                eprintln!("pacchetto scartato: {errore}");
            }
        }
        let prossimo = ricevitore.prossimo_atteso();
        if ricevitore.completo() || prossimo - ultima_conferma >= PASSO_CONFERME {
            conferme.write_all(&prossimo.to_le_bytes()).await?;
            ultima_conferma = prossimo;
        }
        if ricevitore.completo() {
            break;
        }
    }
    file.flush().await?;
    let secondi = inizio.map_or(0.0, |i| i.elapsed().as_secs_f64());

    if !ricevitore.completo() {
        return Err("il mittente si è disconnesso prima della fine: file incompleto".into());
    }
    conferme.finish()?;
    // Lascia al mittente il tempo di leggere l'ultima conferma e chiudere.
    let _ = tokio::time::timeout(Duration::from_secs(5), connessione.closed()).await;

    println!("\nRicevuto {} ({byte} byte) in {secondi:.2} s, {}", uscita.display(), velocita(byte, secondi));
    println!("  pacchetti: {pacchetti} (arrivati in anticipo e messi in attesa: {in_anticipo}, scartati: {scartati})");
    println!("  BLAKE3 del file: {}", hash.finalize().to_hex());
    Ok(())
}
