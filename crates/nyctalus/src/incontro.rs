// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Punto d'incontro e servizi interni `.nyct` (SPECIFICA §6) — prima versione.
//!
//! Modello (prototipo, **rendezvous a salto singolo**):
//! - un **servizio** (`site`) si registra presso un nodo *punto d'incontro* con
//!   la propria chiave pubblica (il suo `.nyct`) e tiene aperta la connessione;
//! - un **client** (`browse`, o il proxy SOCKS) chiede al nodo di essere
//!   collegato a quel `.nyct`; il nodo apre un nuovo stream verso il servizio e
//!   fa da **ponte** (relay) fra i due senza vedere il contenuto;
//! - client e servizio fanno la **stretta Noise NK end-to-end** (l'indirizzo
//!   `.nyct` *è* la chiave del servizio): il nodo inoltra solo byte cifrati.
//!   Sopra la sessione viaggia HTTP, e il servizio serve una cartella.
//!
//! Limite di questa versione: client e servizio raggiungono il punto d'incontro
//! con una connessione **diretta** (come `single-hop`), quindi quel nodo vede i
//! loro IP. Farli arrivare con un **circuito a più salti** — così nemmeno il
//! punto d'incontro conosce gli IP — è il passo successivo. Già ora, però, il
//! client non conosce l'IP del servizio e viceversa: lo sa solo il nodo.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use nyctalus_core::indirizzo;
use nyctalus_core::stretta::{self, Identita, Mittente};

use crate::circuito_rete::{leggi_quadro, scrivi_quadro};
use crate::{Risultato, in_esadecimale};

/// Marca che distingue uno stream di controllo del punto d'incontro da un
/// pacchetto di apertura Sphinx di un circuito.
pub const MAGIA: &[u8; 8] = b"NYCTRDV1";
const REGISTRA: u8 = 0x01; // servizio → nodo: «tieni la mia connessione»
const COLLEGA: u8 = 0x02; // client → nodo: «collegami a questo .nyct»
const PEZZO: usize = 16 * 1024;

// --- registro dei servizi presenti su QUESTO nodo punto d'incontro ----------

fn registro() -> &'static Mutex<HashMap<[u8; 32], quinn::Connection>> {
    static R: OnceLock<Mutex<HashMap<[u8; 32], quinn::Connection>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Lato **nodo**: gestisce uno stream il cui primo quadro inizia con [`MAGIA`].
/// Chiamato da `nodo::gestisci_circuito` quando riconosce il controllo.
pub async fn gestisci(
    connessione: quinn::Connection,
    mut invio: quinn::SendStream,
    ricezione: quinn::RecvStream,
    primo: Vec<u8>,
) -> Risultato<()> {
    if primo.len() < MAGIA.len() + 1 + 32 {
        return Err("rendezvous: control frame too short".into());
    }
    let tipo = primo[MAGIA.len()];
    let mut chiave = [0u8; 32];
    chiave.copy_from_slice(&primo[MAGIA.len() + 1..MAGIA.len() + 1 + 32]);

    match tipo {
        REGISTRA => {
            registro().lock().unwrap().insert(chiave, connessione.clone());
            scrivi_quadro(&mut invio, b"OK").await?;
            // Tieni il servizio registrato finché la sua connessione regge.
            connessione.closed().await;
            registro().lock().unwrap().remove(&chiave);
            Ok(())
        }
        COLLEGA => {
            let servizio = registro().lock().unwrap().get(&chiave).cloned();
            let Some(servizio) = servizio else {
                scrivi_quadro(&mut invio, b"NO").await?;
                return Ok(());
            };
            // Apri un nuovo stream verso il servizio e fai da ponte opaco.
            let (serv_invio, serv_ricezione) = servizio.open_bi().await?;
            scrivi_quadro(&mut invio, b"OK").await?;
            ponte(ricezione, invio, serv_ricezione, serv_invio).await
        }
        _ => Err("rendezvous: unknown control type".into()),
    }
}

/// Relay opaco bidirezionale fra il client e il servizio: byte grezzi, così i
/// quadri cifrati end-to-end passano senza che il nodo li interpreti.
async fn ponte(
    mut cl_ricezione: quinn::RecvStream,
    mut cl_invio: quinn::SendStream,
    mut sr_ricezione: quinn::RecvStream,
    mut sr_invio: quinn::SendStream,
) -> Risultato<()> {
    let verso_servizio = async {
        let _ = tokio::io::copy(&mut cl_ricezione, &mut sr_invio).await;
        let _ = sr_invio.finish();
    };
    let verso_client = async {
        let _ = tokio::io::copy(&mut sr_ricezione, &mut cl_invio).await;
        let _ = cl_invio.finish();
    };
    tokio::join!(verso_servizio, verso_client);
    Ok(())
}

// --- lato servizio: `nyctalus site` -----------------------------------------

/// Ospita la cartella `dir` come sito `.nyct` attraverso il punto d'incontro.
pub async fn site(
    dir: PathBuf,
    file_identita: Option<PathBuf>,
    punto: SocketAddr,
    impronta: [u8; 32],
    offusca: Option<[u8; 32]>,
) -> Risultato<()> {
    if !dir.is_dir() {
        return Err(format!("{} is not a directory", dir.display()).into());
    }
    let identita = match &file_identita {
        Some(p) => carica_o_crea_identita(p)?,
        None => Identita::genera()?,
    };
    let indirizzo_nyct = indirizzo::da_chiave(&identita.pubblica());

    let conn = connetti_al_punto(punto, impronta, offusca).await?;
    let (mut invio, mut ricezione) = conn.open_bi().await?;
    let mut controllo = MAGIA.to_vec();
    controllo.push(REGISTRA);
    controllo.extend_from_slice(&identita.pubblica());
    scrivi_quadro(&mut invio, &controllo).await?;
    match leggi_quadro(&mut ricezione).await? {
        Some(r) if r == b"OK" => {}
        _ => return Err("rendezvous refused the registration".into()),
    }

    println!("Nyctalus site online.");
    println!("  address:    {indirizzo_nyct}");
    println!("  serving:    {}", dir.display());
    println!("  rendezvous: {punto}");
    println!("\nVisit it with:\n  nyctalus browse {indirizzo_nyct} --rendezvous {punto} --fingerprint {}", in_esadecimale(&impronta));

    let identita = Arc::new(identita);
    let dir = Arc::new(dir);
    // Il nodo apre uno stream per ogni client: lo serviamo in parallelo.
    while let Ok((c_invio, c_ricezione)) = conn.accept_bi().await {
        let identita = identita.clone();
        let dir = dir.clone();
        tokio::spawn(async move {
            if let Err(e) = servi_client(c_invio, c_ricezione, &identita, &dir).await {
                eprintln!("client error: {e}");
            }
        });
    }
    Ok(())
}

async fn servi_client(
    mut invio: quinn::SendStream,
    mut ricezione: quinn::RecvStream,
    identita: &Identita,
    dir: &Path,
) -> Risultato<()> {
    // Stretta Noise NK: il servizio è il destinatario (la sua chiave = il .nyct).
    let primo = leggi_quadro(&mut ricezione).await?.ok_or("no handshake")?;
    let (risposta, segreti) = stretta::rispondi(identita, &primo)?;
    scrivi_quadro(&mut invio, &risposta).await?;
    let mut rx = Record::nuovo(&segreti.verso_destinatario);
    let mut tx = Record::nuovo(&segreti.verso_mittente);

    // Una richiesta HTTP (un record), una risposta.
    let richiesta = rx.apri(&leggi_quadro(&mut ricezione).await?.ok_or("no request")?)?;
    let percorso = percorso_richiesto(&richiesta);
    let (stato, tipo, corpo) = apri_risorsa(dir, &percorso);
    let testa = format!(
        "HTTP/1.0 {stato}\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        corpo.len()
    );
    let mut risposta_http = testa.into_bytes();
    risposta_http.extend_from_slice(&corpo);
    for pezzo in risposta_http.chunks(PEZZO) {
        scrivi_quadro(&mut invio, &tx.sigilla(pezzo)).await?;
    }
    let _ = invio.finish();
    Ok(())
}

// --- lato client: `nyctalus browse` -----------------------------------------

/// Scarica una risorsa da un sito `.nyct` attraverso il punto d'incontro e la
/// stampa sullo standard output.
pub async fn browse(
    indirizzo_nyct: &str,
    percorso: &str,
    punto: SocketAddr,
    impronta: [u8; 32],
    offusca: Option<[u8; 32]>,
) -> Risultato<()> {
    let chiave = indirizzo::in_chiave(indirizzo_nyct)?;
    let conn = connetti_al_punto(punto, impronta, offusca).await?;
    let (mut invio, mut ricezione) = conn.open_bi().await?;

    // Chiedi al nodo di collegarti al servizio.
    let mut controllo = MAGIA.to_vec();
    controllo.push(COLLEGA);
    controllo.extend_from_slice(&chiave);
    scrivi_quadro(&mut invio, &controllo).await?;
    match leggi_quadro(&mut ricezione).await? {
        Some(r) if r == b"OK" => {}
        Some(r) if r == b"NO" => return Err("the service is not registered at this rendezvous".into()),
        _ => return Err("rendezvous closed the stream".into()),
    }

    // Stretta Noise NK end-to-end col servizio (il client è il mittente).
    let (mittente, primo) = Mittente::inizia(&chiave)?;
    scrivi_quadro(&mut invio, &primo).await?;
    let risposta = leggi_quadro(&mut ricezione).await?.ok_or("no handshake reply")?;
    let segreti = mittente.completa(&risposta)?;
    let mut tx = Record::nuovo(&segreti.verso_destinatario);
    let mut rx = Record::nuovo(&segreti.verso_mittente);

    // Richiesta HTTP e risposta (sequenza di record).
    let richiesta = format!(
        "GET {percorso} HTTP/1.0\r\nHost: {indirizzo_nyct}\r\nConnection: close\r\n\r\n"
    );
    scrivi_quadro(&mut invio, &tx.sigilla(richiesta.as_bytes())).await?;
    let _ = invio.finish();

    let mut corpo = Vec::new();
    while let Some(q) = leggi_quadro(&mut ricezione).await? {
        corpo.extend_from_slice(&rx.apri(&q)?);
    }
    use std::io::Write as _;
    std::io::stdout().write_all(&corpo)?;
    std::io::stdout().flush()?;
    Ok(())
}

// --- utilità ----------------------------------------------------------------

async fn connetti_al_punto(
    punto: SocketAddr,
    impronta: [u8; 32],
    offusca: Option<[u8; 32]>,
) -> Risultato<quinn::Connection> {
    let locale: SocketAddr = if punto.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse()?;
    let endpoint = crate::trasporto::endpoint_client(locale, offusca)?;
    let config = crate::tls::config_client(impronta)?;
    Ok(endpoint.connect_with(config, punto, crate::tls::NOME_SERVER)?.await?)
}

fn carica_o_crea_identita(percorso: &Path) -> Risultato<Identita> {
    use std::io::Write;
    if percorso.exists() {
        return Ok(Identita::da_byte(&std::fs::read(percorso)?)?);
    }
    let identita = Identita::genera()?;
    let mut opzioni = std::fs::OpenOptions::new();
    opzioni.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opzioni, 0o600);
    opzioni.open(percorso)?.write_all(&identita.in_byte())?;
    println!("New site identity saved to {}", percorso.display());
    Ok(identita)
}

/// Estrae il percorso da una riga di richiesta `GET /qualcosa HTTP/1.0`.
fn percorso_richiesto(richiesta: &[u8]) -> String {
    let testo = String::from_utf8_lossy(richiesta);
    let prima_riga = testo.lines().next().unwrap_or("");
    let mut campi = prima_riga.split_whitespace();
    let _metodo = campi.next();
    campi.next().unwrap_or("/").to_string()
}

/// Apre una risorsa dentro `dir` in modo sicuro (niente uscita dalla cartella).
/// Restituisce `(stato, content-type, corpo)`.
fn apri_risorsa(dir: &Path, percorso: &str) -> (&'static str, &'static str, Vec<u8>) {
    let grezzo = percorso.split('?').next().unwrap_or("/");
    let mut relativo = grezzo.trim_start_matches('/').to_string();
    if relativo.is_empty() {
        relativo = "index.html".to_string();
    }
    // Blocca i tentativi di traversata: niente componenti ".." o percorsi assoluti.
    let sicuro = std::path::Path::new(&relativo)
        .components()
        .all(|c| matches!(c, std::path::Component::Normal(_)));
    if !sicuro {
        return ("403 Forbidden", "text/plain; charset=utf-8", b"403 Forbidden".to_vec());
    }
    let mut completo = dir.join(&relativo);
    if completo.is_dir() {
        completo = completo.join("index.html");
    }
    match std::fs::read(&completo) {
        Ok(byte) => ("200 OK", tipo_contenuto(&completo), byte),
        Err(_) => ("404 Not Found", "text/plain; charset=utf-8", b"404 Not Found".to_vec()),
    }
}

fn tipo_contenuto(percorso: &Path) -> &'static str {
    match percorso.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        "pdf" => "application/pdf",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// Strato di record end-to-end: ChaCha20-Poly1305 con nonce a contatore (una
/// chiave per direzione, dai segreti della sessione Noise). I quadri su uno
/// stream QUIC sono ordinati, quindi i contatori restano allineati.
struct Record {
    aead: ChaCha20Poly1305,
    contatore: u64,
}

impl Record {
    fn nuovo(chiave: &[u8; 32]) -> Self {
        Self { aead: ChaCha20Poly1305::new(Key::from_slice(chiave)), contatore: 0 }
    }

    fn nonce(&mut self) -> Nonce {
        let mut byte = [0u8; 12];
        byte[..8].copy_from_slice(&self.contatore.to_le_bytes());
        self.contatore += 1;
        Nonce::from(byte)
    }

    fn sigilla(&mut self, chiaro: &[u8]) -> Vec<u8> {
        let nonce = self.nonce();
        self.aead.encrypt(&nonce, chiaro).expect("ChaCha20-Poly1305 in memoria non fallisce")
    }

    fn apri(&mut self, cifrato: &[u8]) -> Risultato<Vec<u8>> {
        let nonce = self.nonce();
        self.aead.decrypt(&nonce, cifrato).map_err(|_| "record authentication failed".into())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn record_andata_e_ritorno() {
        let chiave = [7u8; 32];
        let mut tx = Record::nuovo(&chiave);
        let mut rx = Record::nuovo(&chiave);
        let a = tx.sigilla(b"prima");
        let b = tx.sigilla(b"seconda");
        assert_eq!(rx.apri(&a).unwrap(), b"prima");
        assert_eq!(rx.apri(&b).unwrap(), b"seconda");
    }

    #[test]
    fn record_rifiuta_manomissioni() {
        let mut tx = Record::nuovo(&[1u8; 32]);
        let mut rx = Record::nuovo(&[1u8; 32]);
        let mut c = tx.sigilla(b"ciao");
        c[0] ^= 1;
        assert!(rx.apri(&c).is_err());
    }

    #[test]
    fn percorso_predefinito_e_index() {
        assert_eq!(percorso_richiesto(b"GET / HTTP/1.0\r\n"), "/");
        assert_eq!(percorso_richiesto(b"GET /a/b.html HTTP/1.1\r\n"), "/a/b.html");
    }

    #[test]
    fn blocca_la_traversata() {
        let dir = std::env::temp_dir();
        let (stato, _, _) = apri_risorsa(&dir, "/../etc/passwd");
        assert_eq!(stato, "403 Forbidden");
    }
}
