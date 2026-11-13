// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Punto d'incontro e servizi interni `.nyct` (SPECIFICA §6).
//!
//! Un **servizio** (`site`) si registra presso un nodo *punto d'incontro* con la
//! propria chiave pubblica (il suo `.nyct`); un **client** (`browse` o il proxy
//! SOCKS) chiede al nodo di essere collegato a quel `.nyct` e il nodo fa da
//! **ponte opaco** fra i due. Client e servizio fanno la **stretta Noise NK
//! end-to-end** (l'indirizzo `.nyct` *è* la chiave del servizio): il nodo
//! inoltra solo byte cifrati. Sopra la sessione viaggia HTTP e il servizio serve
//! una cartella.
//!
//! Due modi di raggiungere il punto d'incontro, dietro la stessa astrazione
//! [`Canale`]:
//! - **diretto** (`--rendezvous` a mano, un solo nodo): connessione QUIC diretta;
//!   quel nodo vede gli IP. Utile per le prove.
//! - **onion** (automatico, dall'elenco firmato): si raggiunge il punto
//!   d'incontro con un **circuito a più salti**, come l'uscita verso Internet.
//!   Così nemmeno il punto d'incontro conosce gli IP di client e servizio: vede
//!   solo l'ultimo relè. L'uscita del circuito riconosce il controllo (marca
//!   [`MAGIA`] dopo aver tolto il proprio strato) e **cuce** i due circuiti.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use nyctalus_core::circuito::{apri_circuito, NodoPercorso};
use nyctalus_core::socks5::{self, Esito};
use nyctalus_core::stretta::{self, Identita, Mittente, SegretiSessione};
use nyctalus_core::{cipolla, indirizzo, rendezvous};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot};

use crate::avvia_circuito::NodoRete;
use crate::circuito_rete::{codifica_nodo, leggi_quadro, scrivi_quadro, BIT_RISPOSTA};
use crate::Risultato;

/// Marca che, tolto lo strato del nodo, distingue un controllo del punto
/// d'incontro da un LINK conflux (o, su connessione diretta, dal primo quadro).
pub const MAGIA: &[u8; 8] = b"NYCTRDV1";
const REGISTRA: u8 = 0x01; // servizio → nodo: «tieni il mio canale»
const COLLEGA: u8 = 0x02; // client → nodo: «collegami a questo .nyct»
const MAX_INTESTAZIONE: usize = 64 * 1024; // tetto anti-DoS sull'intestazione HTTP

/// Taglia fissa di un quadro del tunnel rendezvous in modo onion: la stessa dei
/// quadri del circuito, così i circuiti del rendezvous non si distinguono da
/// quelli dati (punto H2). Ogni payload è incapsulato `[lunghezza u16][dati][0…]`.
const TAGLIA_TUNNEL: usize = crate::conflux::LUNGHEZZA_UNITA;
/// Byte di payload utili in un quadro onion del tunnel (dopo il prefisso lunghezza).
const CAPACITA_TUNNEL: usize = TAGLIA_TUNNEL - 2;
/// Dimensione massima di un pezzo di dati applicativi: sigillato (+16 del tag)
/// deve stare nel payload di un quadro del tunnel.
const PEZZO: usize = CAPACITA_TUNNEL - 16;

/// Un nodo utilizzabile come punto d'incontro / tappa: indirizzo, chiave X25519
/// e impronta TLS. (Per il `--rendezvous` manuale la chiave X25519 è a zero: si
/// usa solo il modo diretto, che non apre circuiti.)
pub type Punto = NodoRete;

/// Cosa serve il sito `.nyct`: una cartella di file (server incorporato) oppure
/// un **web server esterno** (Nginx/Apache…) a cui il demone fa da proxy.
pub enum ModoServizio {
    Cartella(PathBuf),
    Backend(SocketAddr),
}

// ======================================================================
//  Canale: trasporto a quadri verso/dal punto d'incontro
// ======================================================================

/// Canale a quadri verso il punto d'incontro, in due forme intercambiabili.
enum Canale {
    /// Connessione QUIC diretta (un salto). `_conn`/`_endpoint` tengono vivi
    /// connessione ed endpoint quando il canale li possiede; sono `None` per gli
    /// stream presi da `accept_bi` (li tiene il loop del servizio).
    Diretto {
        invio: quinn::SendStream,
        ricezione: quinn::RecvStream,
        _conn: Option<quinn::Connection>,
        _endpoint: Option<quinn::Endpoint>,
    },
    /// Circuito onion a più salti: ogni quadro è avvolto/sbucciato con le chiavi
    /// di salto (nonce = contatore per direzione, come nel resto del circuito).
    Onion {
        invio: quinn::SendStream,
        ricezione: quinn::RecvStream,
        chiavi: Vec<[u8; 32]>,
        n_out: u64,
        n_in: u64,
        _conn: quinn::Connection,
        _endpoint: quinn::Endpoint,
    },
}

/// Incapsula un payload in un quadro del tunnel a **taglia fissa**
/// `[lunghezza u16 LE][payload][riempimento a zero]`, così tutti i quadri onion
/// del rendezvous hanno la stessa dimensione (come i quadri del circuito).
pub(crate) fn impacchetta_tunnel(payload: &[u8]) -> Vec<u8> {
    debug_assert!(payload.len() <= CAPACITA_TUNNEL);
    let mut v = Vec::with_capacity(TAGLIA_TUNNEL);
    v.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    v.extend_from_slice(payload);
    v.resize(TAGLIA_TUNNEL, 0);
    v
}

/// Estrae il payload da un quadro del tunnel a taglia fissa.
pub(crate) fn spacchetta_tunnel(quadro: &[u8]) -> Risultato<Vec<u8>> {
    if quadro.len() < 2 {
        return Err("tunnel frame too short".into());
    }
    let lunghezza = u16::from_le_bytes([quadro[0], quadro[1]]) as usize;
    if 2 + lunghezza > quadro.len() {
        return Err("tunnel frame malformed".into());
    }
    Ok(quadro[2..2 + lunghezza].to_vec())
}

impl Canale {
    async fn invia(&mut self, carico: &[u8]) -> Risultato<()> {
        match self {
            Canale::Diretto { invio, .. } => scrivi_quadro(invio, carico).await,
            Canale::Onion { invio, chiavi, n_out, .. } => {
                let avvolto = cipolla::avvolgi(*n_out, &impacchetta_tunnel(carico), chiavi);
                *n_out += 1;
                scrivi_quadro(invio, &avvolto).await
            }
        }
    }

    async fn ricevi(&mut self) -> Risultato<Option<Vec<u8>>> {
        match self {
            Canale::Diretto { ricezione, .. } => leggi_quadro(ricezione).await,
            Canale::Onion { ricezione, chiavi, n_in, .. } => match leggi_quadro(ricezione).await? {
                None => Ok(None),
                Some(mut payload) => {
                    for chiave in chiavi.iter() {
                        cipolla::sbuccia(BIT_RISPOSTA | *n_in, chiave, &mut payload);
                    }
                    *n_in += 1;
                    Ok(Some(spacchetta_tunnel(&payload)?))
                }
            },
        }
    }

    fn finish(&mut self) {
        match self {
            Canale::Diretto { invio, .. } => {
                let _ = invio.finish();
            }
            Canale::Onion { invio, .. } => {
                let _ = invio.finish();
            }
        }
    }

    /// Divide il canale in metà di scrittura e lettura indipendenti, per un
    /// ponte bidirezionale concorrente (due task, niente `select!` su una
    /// lettura di quadro che non è cancellation-safe).
    fn dividi(self) -> (ScritturaCanale, LetturaCanale) {
        match self {
            Canale::Diretto { invio, ricezione, _conn, _endpoint } => (
                ScritturaCanale { invio, onion: None, _conn, _endpoint },
                LetturaCanale { ricezione, onion: None },
            ),
            Canale::Onion { invio, ricezione, chiavi, n_out, n_in, _conn, _endpoint } => (
                ScritturaCanale {
                    invio,
                    onion: Some((chiavi.clone(), n_out)),
                    _conn: Some(_conn),
                    _endpoint: Some(_endpoint),
                },
                LetturaCanale { ricezione, onion: Some((chiavi, n_in)) },
            ),
        }
    }
}

/// Metà di scrittura di un [`Canale`].
struct ScritturaCanale {
    invio: quinn::SendStream,
    onion: Option<(Vec<[u8; 32]>, u64)>,
    _conn: Option<quinn::Connection>,
    _endpoint: Option<quinn::Endpoint>,
}

/// Metà di lettura di un [`Canale`].
struct LetturaCanale {
    ricezione: quinn::RecvStream,
    onion: Option<(Vec<[u8; 32]>, u64)>,
}

impl ScritturaCanale {
    async fn invia(&mut self, carico: &[u8]) -> Risultato<()> {
        match &mut self.onion {
            None => scrivi_quadro(&mut self.invio, carico).await,
            Some((chiavi, n)) => {
                let avvolto = cipolla::avvolgi(*n, &impacchetta_tunnel(carico), chiavi);
                *n += 1;
                scrivi_quadro(&mut self.invio, &avvolto).await
            }
        }
    }

    fn finish(&mut self) {
        let _ = self.invio.finish();
    }
}

impl LetturaCanale {
    async fn ricevi(&mut self) -> Risultato<Option<Vec<u8>>> {
        match &mut self.onion {
            None => leggi_quadro(&mut self.ricezione).await,
            Some((chiavi, n)) => match leggi_quadro(&mut self.ricezione).await? {
                None => Ok(None),
                Some(mut payload) => {
                    for chiave in chiavi.iter() {
                        cipolla::sbuccia(BIT_RISPOSTA | *n, chiave, &mut payload);
                    }
                    *n += 1;
                    Ok(Some(spacchetta_tunnel(&payload)?))
                }
            },
        }
    }
}

/// Apre un canale **diretto** verso un nodo (QUIC, un salto).
async fn canale_diretto(nodo: &NodoRete, offusca: Option<[u8; 32]>) -> Risultato<Canale> {
    let (indirizzo, _pub, impronta) = nodo;
    let impronta = impronta.ok_or("rendezvous node without TLS fingerprint")?;
    let locale: SocketAddr = if indirizzo.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" }.parse()?;
    let endpoint = crate::trasporto::endpoint_client(locale, offusca)?;
    let config = crate::tls::config_client(impronta)?;
    let conn = endpoint.connect_with(config, *indirizzo, &crate::tls::nome_per(indirizzo))?.await?;
    let (invio, ricezione) = crate::trasporto::apri_flusso(&conn, offusca).await?;
    Ok(Canale::Diretto { invio, ricezione, _conn: Some(conn), _endpoint: Some(endpoint) })
}

/// Ritardo medio per salto della chat da mixnet (ms): `NYCTALUS_MIX_MS` (di serie 800).
fn ritardo_mix_medio() -> f64 {
    std::env::var("NYCTALUS_MIX_MS").ok().and_then(|v| v.parse::<f64>().ok()).filter(|m| (1.0..=10_000.0).contains(m)).unwrap_or(800.0)
}

/// Un ritardo esponenziale (Loopix) per un salto, in ms, limitato a 4× la media.
fn ritardo_mix_ms() -> u16 {
    let media = ritardo_mix_medio();
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).expect("generatore casuale di sistema");
    let u = (u64::from_le_bytes(b) as f64 / u64::MAX as f64).clamp(1e-12, 1.0);
    (-u.ln() * media).min(media * 4.0).clamp(1.0, 20_000.0) as u16
}

/// Apre un canale **onion** costruendo un circuito il cui ultimo nodo è il punto
/// d'incontro (`percorso` = guard + intermedi + punto d'incontro).
async fn canale_onion(percorso: &[NodoRete], offusca: Option<[u8; 32]>, mix: bool) -> Risultato<Canale> {
    let mut nodi: Vec<NodoPercorso> = percorso
        .iter()
        .map(|(a, pubblica, _)| Ok(NodoPercorso { indirizzo: codifica_nodo(*a, pubblica)?, pubblica: *pubblica }))
        .collect::<Risultato<_>>()?;
    if mix {
        // Chat da mixnet: l'istruzione per il nodo i-1 viaggia nell'indirizzo del nodo i (solo
        // il nodo i-1 la legge). Ritardi esponenziali scelti qui, come in Loopix.
        for n in nodi.iter_mut().skip(1) {
            n.indirizzo = crate::circuito_rete::con_ritardo(n.indirizzo, ritardo_mix_ms());
        }
    }
    let circuito = apri_circuito(&nodi, b"nyctalus rendezvous")?;

    let (guard, _, impronta_guard) = percorso[0];
    // all'entrata con la chiave del ponte, se è un ponte nascosto; UDP o, se bloccato, TCP
    let chiave_entrata = crate::percorsi::chiave_entrata(&percorso[0], offusca);
    let config = match impronta_guard {
        Some(fp) => crate::tls::config_client(fp)?,
        None => crate::tls::config_client_insicuro()?,
    };
    let (endpoint, conn) = match crate::trasporto::collega_entrata(guard, chiave_entrata, config, impronta_guard).await {
        Ok(c) => c,
        Err(e) => {
            // l'entrata non risponde: il prossimo percorso parte dalla guardia primaria seguente
            crate::percorsi::segna_guardia_irraggiungibile(&percorso[0].1);
            return Err(e);
        }
    };
    let (mut invio, ricezione) = crate::trasporto::apri_flusso(&conn, chiave_entrata).await?;
    // 1º quadro = pacchetto di apertura Sphinx; poi i quadri onion (n da 0).
    scrivi_quadro(&mut invio, &circuito.pacchetto).await?;
    Ok(Canale::Onion {
        invio,
        ricezione,
        chiavi: circuito.chiavi_salto,
        n_out: 0,
        n_in: 0,
        _conn: conn,
        _endpoint: endpoint,
    })
}

/// Percorsi diversi da provare verso lo stesso punto d'incontro prima di passare al seguente.
const PERCORSI_PER_PUNTO: usize = 2;

/// Canale onion verso `elenco[idx]` (punto d'incontro). Se a non rispondere è l'ENTRATA (una
/// guardia primaria, appena scartata), si riprova lo STESSO punto d'incontro dalla guardia
/// seguente: la colpa non è sua, e servizio e visitatore devono trovarsi sullo stesso.
async fn canale_verso(
    elenco: &[NodoRete],
    idx: usize,
    salti: usize,
    offusca: Option<[u8; 32]>,
    mix: bool,
) -> Risultato<(Canale, Vec<NodoRete>)> {
    let mut tentativo = 0;
    loop {
        tentativo += 1;
        let percorso = percorso_onion(elenco, idx, salti)?;
        match canale_onion(&percorso, offusca, mix).await {
            Ok(c) => return Ok((c, percorso)),
            Err(e) => {
                let entrata = percorso[0].1;
                if tentativo < crate::percorsi::GUARDIE_PRIMARIE
                    && percorso.len() > 1
                    && crate::percorsi::guardia_scartata(&entrata)
                    && crate::percorsi::guardia(elenco).1 != entrata
                {
                    continue;
                }
                return Err(e);
            }
        }
    }
}

/// Client serviti insieme da un servizio .nyct (oltre, i nuovi aspettano che se ne liberi uno).
const CLIENT_MAX: usize = 256;

/// Percorso onion per raggiungere `elenco[rendezvous]` come uscita: si entra dalla
/// **guardia** persistente della rete (o, se è parente del punto d'incontro, da un altro
/// nodo), poi intermedi a caso, mai parenti fra loro ([`crate::percorsi`]). Fino a
/// `salti-1` nodi prima del punto d'incontro.
fn percorso_onion(elenco: &[NodoRete], rendezvous: usize, salti: usize) -> Risultato<Vec<NodoRete>> {
    // in modalità rigida, se la regola è impossibile, errore: meglio non collegarsi che
    // collegarsi con nodi parenti
    crate::percorsi::percorso_verso(elenco, elenco[rendezvous], salti)
}

fn controllo(tipo: u8, chiave: &[u8; 32]) -> Vec<u8> {
    let mut c = MAGIA.to_vec();
    c.push(tipo);
    c.extend_from_slice(chiave);
    c
}

fn adesso_s() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// REGISTRA **provata** (dalla 0.3.19): dopo la chiave, l'ora (8 byte), un numero casuale (8 byte:
/// due registrazioni nello stesso secondo hanno prove diverse) e la prova (32 byte) che
/// la registrazione viene da chi ha la chiave privata del servizio, legata al nodo `nodo`.
fn registrazione(identita: &Identita, nodo: &[u8; 32]) -> Vec<u8> {
    let ora = adesso_s();
    let mut c = controllo(REGISTRA, &identita.pubblica());
    // solo per le prove di compatibilità: registrarsi come un servizio di prima della 0.3.19
    if std::env::var("NYCTALUS_REGISTRA_VECCHIA").is_ok_and(|v| v == "1") {
        return c;
    }
    let mut unico = [0u8; 8];
    getrandom::fill(&mut unico).expect("generatore casuale di sistema");
    c.extend_from_slice(&ora.to_be_bytes());
    c.extend_from_slice(&unico);
    c.extend_from_slice(&identita.prova_registrazione(nodo, ora, &unico));
    c
}

/// Tolleranza sull'ora della prova (orologi non perfetti) e memoria delle prove già viste.
const FINESTRA_PROVA_S: u64 = 600;

/// Prova di una REGISTRA: ora, numero casuale, MAC.
type Prova = (u64, [u8; 8], [u8; 32]);

/// Esito del controllo di una REGISTRA.
#[derive(Debug, PartialEq, Eq)]
enum Ammissione {
    /// prova valida: la registrazione è del servizio
    Provata,
    /// senza prova (servizio vecchio): solo se il nodo è in modo compatibile
    /// (`NYCTALUS_REGISTRA_COMPATIBILE=1`) e il servizio non ne ha di provate
    NonProvata,
    /// prova sbagliata, scaduta o già usata, oppure senza prova per un servizio che prova
    Rifiutata,
}

/// Chiavi con registrazioni provate di recente (per rifiutare quelle senza prova) e prove già viste.
fn memoria_prove() -> &'static Mutex<(HashMap<[u8; 32], u64>, HashMap<[u8; 32], u64>)> {
    static M: OnceLock<Mutex<(HashMap<[u8; 32], u64>, HashMap<[u8; 32], u64>)>> = OnceLock::new();
    M.get_or_init(|| Mutex::new((HashMap::new(), HashMap::new())))
}

/// Di serie il nodo accetta SOLO registrazioni provate: altrimenti un impostore, rifiutato dove
/// il servizio vero si è registrato, potrebbe registrarsi senza prova a un altro punto d'incontro
/// candidato. `NYCTALUS_REGISTRA_COMPATIBILE=1` (scelta di chi gestisce il nodo, solo durante il
/// passaggio alla 0.3.19) ammette anche i servizi vecchi, finché quel servizio non ne ha di provate.
fn compatibile() -> bool {
    static C: OnceLock<bool> = OnceLock::new();
    *C.get_or_init(|| std::env::var("NYCTALUS_REGISTRA_COMPATIBILE").is_ok_and(|v| v == "1"))
}

fn ammetti(segreto_nodo: &[u8; 32], chiave: &[u8; 32], prova: Option<Prova>) -> Ammissione {
    ammetti_con(compatibile(), segreto_nodo, chiave, prova)
}

fn ammetti_con(compatibile: bool, segreto_nodo: &[u8; 32], chiave: &[u8; 32], prova: Option<Prova>) -> Ammissione {
    let ora = adesso_s();
    let mut m = memoria_prove().lock().unwrap();
    let (provate, viste) = &mut *m;
    // pulizia: dimentica ciò che è più vecchio della finestra (e del periodo di una registrazione)
    if viste.len() > 100_000 {
        viste.retain(|_, t| ora.saturating_sub(*t) <= 2 * FINESTRA_PROVA_S);
    }
    provate.retain(|_, t| ora.saturating_sub(*t) <= 3600);
    match prova {
        None => {
            if !compatibile || provate.contains_key(chiave) {
                Ammissione::Rifiutata
            } else {
                Ammissione::NonProvata
            }
        }
        Some((t, u, p)) => {
            if ora.abs_diff(t) > FINESTRA_PROVA_S
                || viste.contains_key(&p)
                || !stretta::verifica_registrazione(segreto_nodo, chiave, t, &u, &p)
            {
                return Ammissione::Rifiutata;
            }
            viste.insert(p, ora);
            provate.insert(*chiave, ora);
            Ammissione::Provata
        }
    }
}

/// Lato client: apre un canale **collegato al servizio** `chiave`. In manuale
/// usa il nodo diretto; altrimenti prova i punti d'incontro candidati
/// (nell'ordine deterministico) con un circuito a più salti finché trova il
/// servizio. Il canale restituito è già oltre il controllo COLLEGA→OK.
async fn collega_al_servizio(
    chiave: &[u8; 32],
    elenco: &[NodoRete],
    manuale: Option<NodoRete>,
    offusca: Option<[u8; 32]>,
    salti: usize,
    mix: bool,
) -> Risultato<Canale> {
    if let Some(nodo) = manuale {
        let mut canale = canale_diretto(&nodo, offusca).await?;
        canale.invia(&controllo(COLLEGA, chiave)).await?;
        return match canale.ricevi().await? {
            Some(r) if r == b"OK" => Ok(canale),
            Some(r) if r == b"NO" => Err("service not registered at this rendezvous".into()),
            _ => Err("rendezvous closed the stream".into()),
        };
    }
    if elenco.is_empty() {
        return Err("no rendezvous node available (empty signed directory?)".into());
    }
    let epoca = rendezvous::epoca_attuale();
    let mut ultimo = String::from("no rendezvous candidates");
    for idx in rendezvous::ordine_con_ricambio(chiave, elenco.len(), epoca) {
        let nodo = elenco[idx].0;
        // un circuito che cade può avere un nodo INTERMEDIO spento (ancora nell'elenco): prima di
        // dare la colpa al punto d'incontro lo si riprova con un altro percorso (come Tor)
        for giro in 0..PERCORSI_PER_PUNTO {
            let mut canale = match canale_verso(elenco, idx, salti, offusca, mix).await {
                Ok((c, _)) => c,
                Err(e) => {
                    ultimo = format!("rendezvous {nodo}: {e}");
                    crate::diagnosi(|| format!("client: {ultimo} (percorso {})", giro + 1));
                    continue;
                }
            };
            if canale.invia(&controllo(COLLEGA, chiave)).await.is_err() {
                ultimo = format!("rendezvous {nodo} closed the stream");
                continue;
            }
            match canale.ricevi().await {
                Ok(Some(r)) if r == b"OK" => return Ok(canale),
                Ok(Some(r)) if r == b"NO" => {
                    // risposta del punto d'incontro: il servizio lì non c'è, si passa al prossimo
                    ultimo = format!("service not registered at {nodo}");
                    crate::diagnosi(|| format!("client: {ultimo}"));
                    break;
                }
                _ => ultimo = format!("rendezvous {nodo} closed the stream"),
            }
            crate::diagnosi(|| format!("client: {ultimo} (percorso {})", giro + 1));
        }
    }
    Err(format!("could not reach the service ({ultimo})").into())
}

// ======================================================================
//  Lato nodo: splice del punto d'incontro
// ======================================================================
//
//  Due registri separati, perché i due modi hanno trasporti diversi:
//  - diretto: si cuciono due stream QUIC (byte grezzi);
//  - onion: si cuciono due circuiti (plaintext peeling/wrapping via canali).

// --- modo diretto -----------------------------------------------------

fn registro_diretto() -> &'static Mutex<HashMap<[u8; 32], quinn::Connection>> {
    static R: OnceLock<Mutex<HashMap<[u8; 32], quinn::Connection>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Lato **nodo**, connessione diretta: il primo quadro (grezzo) inizia con
/// [`MAGIA`]. Chiamato da `nodo::gestisci_circuito`.
pub async fn gestisci(
    connessione: quinn::Connection,
    mut invio: quinn::SendStream,
    ricezione: quinn::RecvStream,
    primo: Vec<u8>,
    segreto_nodo: &[u8; 32],
) -> Risultato<()> {
    let (tipo, chiave, prova) = analizza_controllo_provato(&primo)?;
    match tipo {
        REGISTRA => {
            if ammetti(segreto_nodo, &chiave, prova) == Ammissione::Rifiutata {
                scrivi_quadro(&mut invio, b"NO").await?;
                return Ok(());
            }
            registro_diretto().lock().unwrap().insert(chiave, connessione.clone());
            scrivi_quadro(&mut invio, b"OK").await?;
            connessione.closed().await;
            registro_diretto().lock().unwrap().remove(&chiave);
            Ok(())
        }
        COLLEGA => {
            let servizio = registro_diretto().lock().unwrap().get(&chiave).cloned();
            let Some(servizio) = servizio else {
                scrivi_quadro(&mut invio, b"NO").await?;
                return Ok(());
            };
            let (serv_invio, serv_ricezione) = servizio.open_bi().await?;
            scrivi_quadro(&mut invio, b"OK").await?;
            ponte_grezzo(ricezione, invio, serv_ricezione, serv_invio).await
        }
        _ => Err("rendezvous: unknown control type".into()),
    }
}

/// Relay opaco bidirezionale fra due stream QUIC (byte grezzi).
async fn ponte_grezzo(
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

// --- modo onion -------------------------------------------------------

/// Appuntamento all'uscita: il servizio lascia qui con cosa raggiungerlo
/// (`verso_servizio`) e un recapito (`consegna`) su cui il client gli passa il
/// proprio canale di ritorno.
struct Incontro {
    verso_servizio: mpsc::Sender<Vec<u8>>,
    consegna: oneshot::Sender<mpsc::Sender<Vec<u8>>>,
    /// registrazione provata (dalla 0.3.19) o di un servizio vecchio
    provata: bool,
}

/// Coda di servizi in attesa per ogni chiave: un servizio può registrare più
/// circuiti (pool) per servire client in parallelo; ogni client ne consuma uno.
fn registro_onion() -> &'static Mutex<HashMap<[u8; 32], std::collections::VecDeque<Incontro>>> {
    static R: OnceLock<Mutex<HashMap<[u8; 32], std::collections::VecDeque<Incontro>>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Lato **uscita del circuito**: il primo quadro onion, tolto lo strato, inizia
/// con [`MAGIA`]. Registra il servizio o collega il client, cucendo i due
/// circuiti. `chiave` è la chiave di salto di quest'uscita; `primo` è il
/// controllo già sbucciato (nonce 0). I quadri onion successivi in andata usano
/// nonce da 1; quelli di ritorno nonce da 0 (il primo è "OK"/"NO").
pub async fn uscita_rendezvous(
    invio: quinn::SendStream,
    mut ricezione: quinn::RecvStream,
    chiave: [u8; 32],
    primo: Vec<u8>,
    segreto_nodo: &[u8; 32],
) -> Risultato<()> {
    let (tipo, key, prova) = analizza_controllo_provato(&spacchetta_tunnel(&primo)?)?;
    match tipo {
        REGISTRA => {
            let ammissione = ammetti(segreto_nodo, &key, prova);
            if ammissione == Ammissione::Rifiutata {
                scrittore_ritorno_singolo(invio, chiave, b"NO".to_vec()).await;
                return Ok(());
            }
            let (tx_vs, rx_vs) = mpsc::channel::<Vec<u8>>(256);
            let (consegna_tx, consegna_rx) = oneshot::channel::<mpsc::Sender<Vec<u8>>>();
            {
                let mut reg = registro_onion().lock().unwrap();
                let coda = reg.entry(key).or_default();
                // la prima registrazione provata scarta quelle senza prova (forse di un impostore)
                if ammissione == Ammissione::Provata {
                    coda.retain(|i| i.provata);
                }
                coda.push_back(Incontro { verso_servizio: tx_vs, consegna: consegna_tx, provata: ammissione == Ammissione::Provata });
            }

            // Verso il servizio: "OK" SUBITO (prima partiva solo all'arrivo del primo client, e il
            // servizio non sapeva di essere registrato), poi i plaintext client→servizio da rx_vs.
            let scrittore = tokio::spawn(scrittore_ritorno(invio, chiave, b"OK".to_vec(), rx_vs));
            // Attende che un client si colleghi e consegni il suo canale — ma intanto
            // tiene d'occhio il circuito del servizio: se cade (servizio spento), la
            // registrazione si scarta invece di restare in coda «morta».
            let consegnato = tokio::select! {
                c = consegna_rx => c.ok(),
                _ = attendi_chiusura(&mut ricezione) => None,
            };
            match consegnato {
                Some(verso_client) => {
                    // Dal servizio: plaintext servizio→client verso `verso_client`.
                    let lettore = lettore_avanti(ricezione, chiave, 1, verso_client);
                    let (_s, _l) = tokio::join!(scrittore, lettore);
                }
                None => {
                    // Nessun client (o circuito del servizio caduto): chiudi pulito.
                    scrittore.abort();
                }
            }
            Ok(())
        }
        COLLEGA => {
            // La prima registrazione VIVA del servizio: quelle il cui circuito è
            // caduto (consegna rifiutata) si scartano e si passa alla successiva.
            let (tx_vc, rx_vc) = mpsc::channel::<Vec<u8>>(256);
            let mut tx_vc = Some(tx_vc);
            let mut trovato = None;
            loop {
                let incontro = {
                    let mut reg = registro_onion().lock().unwrap();
                    let coda = reg.get_mut(&key).and_then(|q| q.pop_front());
                    if reg.get(&key).is_some_and(|q| q.is_empty()) {
                        reg.remove(&key);
                    }
                    coda
                };
                let Some(incontro) = incontro else { break };
                match incontro.consegna.send(tx_vc.take().expect("canale ancora da consegnare")) {
                    Ok(()) => {
                        trovato = Some(incontro.verso_servizio);
                        break;
                    }
                    Err(indietro) => tx_vc = Some(indietro), // registrazione morta: la prossima
                }
            }
            let Some(verso_servizio) = trovato else {
                scrittore_ritorno_singolo(invio, chiave, b"NO".to_vec()).await;
                return Ok(());
            };
            // Verso il client: "OK", poi i plaintext servizio→client da rx_vc.
            let scrittore = scrittore_ritorno(invio, chiave, b"OK".to_vec(), rx_vc);
            // Dal client: plaintext client→servizio verso il canale del servizio.
            let lettore = lettore_avanti(ricezione, chiave, 1, verso_servizio);
            tokio::join!(scrittore, lettore);
            Ok(())
        }
        _ => Err("rendezvous: unknown control type".into()),
    }
}

/// Torna quando il circuito di una registrazione in attesa si chiude o cade. Il
/// servizio, dopo REGISTRA, non manda nulla finché non arriva un client: se arriva
/// qualcosa (o niente più), per la registrazione in attesa vale come chiusura.
async fn attendi_chiusura(ricezione: &mut quinn::RecvStream) {
    let mut un_byte = [0u8; 1];
    let _ = ricezione.read(&mut un_byte).await;
}

/// Scrive sul ritorno del circuito: prima `iniziale`, poi i plaintext da `rx`,
/// avvolgendo ognuno con lo strato di quest'uscita (nonce di ritorno crescente).
async fn scrittore_ritorno(
    mut invio: quinn::SendStream,
    chiave: [u8; 32],
    iniziale: Vec<u8>,
    mut rx: mpsc::Receiver<Vec<u8>>,
) {
    let mut n: u64 = 0;
    // "OK"/"NO" li genera l'uscita, quindi vanno incapsulati qui (i quadri
    // relayati da `rx` sono già incapsulati dal mittente: si passano com'è).
    let mut buf = impacchetta_tunnel(&iniziale);
    cipolla::sbuccia(BIT_RISPOSTA | n, &chiave, &mut buf);
    n += 1;
    if scrivi_quadro(&mut invio, &buf).await.is_err() {
        return;
    }
    while let Some(mut p) = rx.recv().await {
        cipolla::sbuccia(BIT_RISPOSTA | n, &chiave, &mut p);
        n += 1;
        if scrivi_quadro(&mut invio, &p).await.is_err() {
            break;
        }
    }
    let _ = invio.finish();
}

/// Scrive un solo quadro di ritorno (usato per "NO").
async fn scrittore_ritorno_singolo(mut invio: quinn::SendStream, chiave: [u8; 32], carico: Vec<u8>) {
    let mut buf = impacchetta_tunnel(&carico);
    cipolla::sbuccia(BIT_RISPOSTA, &chiave, &mut buf);
    let _ = scrivi_quadro(&mut invio, &buf).await;
    let _ = invio.finish();
}

/// Legge i quadri in andata dal circuito, toglie lo strato di quest'uscita
/// (nonce crescente da `n`) e passa il plaintext a `tx`.
async fn lettore_avanti(
    mut ricezione: quinn::RecvStream,
    chiave: [u8; 32],
    mut n: u64,
    tx: mpsc::Sender<Vec<u8>>,
) {
    while let Ok(Some(mut p)) = leggi_quadro(&mut ricezione).await {
        cipolla::sbuccia(n, &chiave, &mut p);
        n += 1;
        if tx.send(p).await.is_err() {
            break;
        }
    }
}

#[cfg(test)]
fn analizza_controllo(primo: &[u8]) -> Risultato<(u8, [u8; 32])> {
    analizza_controllo_provato(primo).map(|(t, k, _)| (t, k))
}

/// Come `analizza_controllo`, con l'eventuale prova di una REGISTRA (ora, prova).
pub(crate) fn analizza_controllo_provato(primo: &[u8]) -> Risultato<(u8, [u8; 32], Option<Prova>)> {
    let base = MAGIA.len() + 1 + 32;
    if primo.len() < base || !primo.starts_with(MAGIA) {
        return Err("rendezvous: malformed control frame".into());
    }
    let tipo = primo[MAGIA.len()];
    let mut chiave = [0u8; 32];
    chiave.copy_from_slice(&primo[MAGIA.len() + 1..base]);
    let prova = (primo.len() >= base + 48).then(|| {
        let ora = u64::from_be_bytes(primo[base..base + 8].try_into().expect("8 byte"));
        let unico: [u8; 8] = primo[base + 8..base + 16].try_into().expect("8 byte");
        let mut p = [0u8; 32];
        p.copy_from_slice(&primo[base + 16..base + 48]);
        (ora, unico, p)
    });
    Ok((tipo, chiave, prova))
}

// ======================================================================
//  Lato servizio: `nyctalus site`
// ======================================================================

#[allow(clippy::too_many_arguments)]
pub async fn site(
    modo: ModoServizio,
    file_identita: Option<PathBuf>,
    identita_chat: Option<SocketAddr>,
    elenco: Vec<NodoRete>,
    manuale: Option<NodoRete>,
    offusca: Option<[u8; 32]>,
    salti: usize,
    pool: usize,
) -> Risultato<()> {
    let descrizione = match &modo {
        ModoServizio::Cartella(dir) => {
            if !dir.is_dir() {
                return Err(format!("{} is not a directory", dir.display()).into());
            }
            format!("folder {}", dir.display())
        }
        ModoServizio::Backend(addr) => format!("backend {addr}"),
    };
    let identita = match (identita_chat, &file_identita) {
        (Some(chat), _) => identita_dalla_chat(chat).await?,
        (None, Some(p)) => carica_o_crea_identita(p)?,
        (None, None) => Identita::genera()?,
    };
    let indirizzo_nyct = indirizzo::da_chiave(&identita.pubblica());
    // «online» SOLO dopo la prima registrazione riuscita a un punto d'incontro: prima chi visitava
    // subito il sito poteva trovarlo «non registrato» (scoperto con la prova 33, errore intermittente)
    println!("Nyctalus site starting (registering at a rendezvous node)…");
    println!("  address:  {indirizzo_nyct}");
    println!("  serving:  {descrizione}");
    println!(
        "  mode:     {}",
        if manuale.is_some() { "direct rendezvous (single hop)" } else { "onion rendezvous (multi-hop)" }
    );
    println!("\nVisit it with:\n  nyctalus browse {indirizzo_nyct}\n");

    let identita = Arc::new(identita);
    let modo = Arc::new(modo);

    // Modo diretto (un nodo, concorrente): una connessione, più client in
    // parallelo (il nodo apre uno stream per client).
    if let Some(nodo) = manuale {
        return site_diretto(nodo, identita, modo, offusca).await;
    }

    // Modo onion: il servizio serve un client per circuito. Un pool di registrazioni
    // concorrenti ASPETTA i client: appena uno arriva, viene servito in un compito a parte e il
    // posto si ri-registra SUBITO per il prossimo. (Prima il posto restava occupato finché il
    // client non se ne andava: con sessioni lunghe, come la chat, dal 5° client in poi nessuno
    // entrava più. L'ha scoperto la simulazione in Shadow, PIANO-ANONIMATO punto K.)
    // Tetto ai client serviti insieme, perché nessuno possa esaurire le risorse del servizio.
    let pool = pool.clamp(1, 32);
    let annunciato = Arc::new(std::sync::Once::new());
    let elenco = Arc::new(elenco);
    let attivi = Arc::new(tokio::sync::Semaphore::new(CLIENT_MAX));
    let mut task = Vec::new();
    for _ in 0..pool {
        let identita = identita.clone();
        let modo = modo.clone();
        let elenco = elenco.clone();
        let attivi = attivi.clone();
        let annunciato = annunciato.clone();
        task.push(tokio::spawn(async move {
            loop {
                let Ok(posto) = attivi.clone().acquire_owned().await else { return };
                match registra_onion_e_canale(&identita, &elenco, offusca, salti).await {
                    Ok(mut canale) => {
                        annunciato.call_once(|| println!("Nyctalus site online (registered at a rendezvous node)."));
                        // il posto resta in attesa finché arriva un client (il suo primo messaggio):
                        // solo allora si ri-registra per il prossimo. Se la registrazione muore prima
                        // (punto d'incontro caduto), si ri-registra subito.
                        let primo = match canale.ricevi().await {
                            Ok(Some(p)) => p,
                            _ => {
                                drop(posto);
                                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                                continue;
                            }
                        };
                        let (identita, modo) = (identita.clone(), modo.clone());
                        tokio::spawn(async move {
                            let _posto = posto; // libero quando il client se ne va
                            crate::diagnosi(|| "servizio: arriva un client".into());
                            if let Err(e) = servi_client_da(canale, primo, &identita, &modo).await {
                                eprintln!("client error: {e}");
                            }
                            crate::diagnosi(|| "servizio: client finito".into());
                        });
                    }
                    Err(e) => {
                        drop(posto);
                        eprintln!("could not register at a rendezvous node: {e}");
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
            }
        }));
    }
    for t in task {
        let _ = t.await;
    }
    Ok(())
}

/// Modo diretto: tiene una connessione al nodo e serve un client per stream.
async fn site_diretto(
    nodo: NodoRete,
    identita: Arc<Identita>,
    modo: Arc<ModoServizio>,
    offusca: Option<[u8; 32]>,
) -> Risultato<()> {
    let (conn, _endpoint) = match canale_diretto(&nodo, offusca).await? {
        Canale::Diretto { mut invio, mut ricezione, _conn: Some(conn), _endpoint } => {
            // Registra questa connessione come servizio.
            scrivi_quadro(&mut invio, &registrazione(&identita, &nodo.1)).await?;
            match leggi_quadro(&mut ricezione).await? {
                Some(r) if r == b"OK" => println!("Nyctalus site online (registered at the rendezvous node)."),
                _ => return Err("registration refused".into()),
            }
            (conn, _endpoint)
        }
        _ => return Err("unexpected channel kind".into()),
    };
    while let Ok((invio, ricezione)) = conn.accept_bi().await {
        let identita = identita.clone();
        let modo = modo.clone();
        tokio::spawn(async move {
            let canale = Canale::Diretto { invio, ricezione, _conn: None, _endpoint: None };
            if let Err(e) = servi_client(canale, &identita, &modo).await {
                eprintln!("client error: {e}");
            }
        });
    }
    drop(_endpoint);
    Ok(())
}

/// Lato servizio (onion): costruisce un circuito verso un punto d'incontro
/// candidato e si registra; restituisce il canale oltre REGISTRA→OK.
async fn registra_onion_e_canale(
    identita: &Identita,
    elenco: &[NodoRete],
    offusca: Option<[u8; 32]>,
    salti: usize,
) -> Risultato<Canale> {
    let chiave_pub = &identita.pubblica();
    if elenco.is_empty() {
        return Err("empty signed directory".into());
    }
    let epoca = rendezvous::epoca_attuale();
    let mut ultimo = String::from("no candidates");
    for idx in rendezvous::ordine(chiave_pub, elenco.len(), epoca) {
        let nodo = elenco[idx].0;
        // come per i visitatori: un circuito caduto si riprova con un altro percorso, così
        // servizio e visitatori restano sullo stesso punto d'incontro anche con nodi spenti
        for giro in 0..PERCORSI_PER_PUNTO {
            let (mut canale, percorso) = match canale_verso(elenco, idx, salti, offusca, false).await {
                Ok(c) => c,
                Err(e) => {
                    ultimo = format!("rendezvous {nodo}: {e}");
                    crate::diagnosi(|| format!("servizio: {ultimo} (percorso {})", giro + 1));
                    continue;
                }
            };
            if canale.invia(&registrazione(identita, &percorso.last().unwrap().1)).await.is_err() {
                ultimo = format!("rendezvous {nodo} closed the stream");
                continue;
            }
            match canale.ricevi().await {
                Ok(Some(r)) if r == b"OK" => {
                    crate::diagnosi(|| format!("servizio: registrato al punto d'incontro {nodo}"));
                    return Ok(canale);
                }
                Ok(Some(_)) => {
                    ultimo = format!("rendezvous {nodo} refused the registration");
                    crate::diagnosi(|| format!("servizio: {ultimo}"));
                    break;
                }
                _ => {
                    ultimo = format!("rendezvous {nodo} closed the stream");
                    crate::diagnosi(|| format!("servizio: {ultimo} (percorso {})", giro + 1));
                }
            }
        }
    }
    Err(ultimo.into())
}

async fn servi_client(mut canale: Canale, identita: &Identita, modo: &ModoServizio) -> Risultato<()> {
    // Stretta Noise NK: il servizio è il destinatario (la sua chiave = il .nyct).
    let primo = canale.ricevi().await?.ok_or("no handshake")?;
    servi_client_da(canale, primo, identita, modo).await
}

/// Come `servi_client`, con il primo messaggio del client già letto.
async fn servi_client_da(mut canale: Canale, primo: Vec<u8>, identita: &Identita, modo: &ModoServizio) -> Risultato<()> {
    let (risposta, segreti) = stretta::rispondi(identita, &primo)?;
    canale.invia(&risposta).await?;
    match modo {
        ModoServizio::Cartella(dir) => servi_cartella(&mut canale, &segreti, dir).await,
        ModoServizio::Backend(addr) => servi_backend(canale, &segreti, *addr).await,
    }
}

/// Server di file incorporato: serve una cartella, in keep-alive, sulla sessione
/// cifrata. Consuma l'eventuale corpo della richiesta (Content-Length) così il
/// parsing resta allineato anche con richieste che hanno un corpo (es. POST).
async fn servi_cartella(canale: &mut Canale, segreti: &SegretiSessione, dir: &Path) -> Risultato<()> {
    let mut rx = Record::nuovo(&segreti.verso_destinatario);
    let mut tx = Record::nuovo(&segreti.verso_mittente);
    let mut buffer: Vec<u8> = Vec::new();
    loop {
        let fine = loop {
            if let Some(p) = trova_fine_intestazione(&buffer) {
                break p;
            }
            if buffer.len() > MAX_INTESTAZIONE {
                return Err("request header too large".into());
            }
            match canale.ricevi().await? {
                Some(q) => buffer.extend_from_slice(&rx.apri(&q)?),
                None => return Ok(()),
            }
        };
        let intestazione: Vec<u8> = buffer.drain(..fine).collect();
        let percorso = percorso_richiesto(&intestazione);
        let keep_alive = vuole_keep_alive(&intestazione);

        // Scarta il corpo della richiesta, se c'è, per non disallineare il flusso.
        let mut da_scartare = lunghezza_corpo(&intestazione).unwrap_or(0);
        let gia = buffer.len().min(da_scartare);
        buffer.drain(..gia);
        da_scartare -= gia;
        while da_scartare > 0 {
            match canale.ricevi().await? {
                Some(q) => {
                    let d = rx.apri(&q)?;
                    if d.len() <= da_scartare {
                        da_scartare -= d.len();
                    } else {
                        buffer.extend_from_slice(&d[da_scartare..]);
                        da_scartare = 0;
                    }
                }
                None => return Ok(()),
            }
        }

        let (stato, tipo, corpo) = apri_risorsa(dir, &percorso);
        let chiusura = if keep_alive { "keep-alive" } else { "close" };
        let testa = format!(
            "HTTP/1.1 {stato}\r\nContent-Type: {tipo}\r\nContent-Length: {}\r\nConnection: {chiusura}\r\n\r\n",
            corpo.len()
        );
        let mut risposta_http = testa.into_bytes();
        risposta_http.extend_from_slice(&corpo);
        for pezzo in risposta_http.chunks(PEZZO) {
            canale.invia(&tx.sigilla(pezzo)).await?;
        }
        if !keep_alive {
            canale.finish();
            // Tiene viva la connessione finché il client non chiude: così la
            // risposta (e il FIN) arrivano prima che il chiamante scarti la
            // connessione QUIC. Senza questo, i frame in volo verrebbero abortiti.
            while canale.ricevi().await?.is_some() {}
            return Ok(());
        }
    }
}

/// Proxy verso un **web server esterno** (Nginx/Apache…): la sessione cifrata
/// viene collegata byte-per-byte a una connessione TCP locale. Così il demone
/// Nyctalus sta davanti a un sito esistente, che gestisce lui HTTP (GET, POST,
/// keep-alive, pipeline).
async fn servi_backend(canale: Canale, segreti: &SegretiSessione, backend: SocketAddr) -> Risultato<()> {
    let mut rx = Record::nuovo(&segreti.verso_destinatario);
    let mut tx = Record::nuovo(&segreti.verso_mittente);
    let tcp = TcpStream::connect(backend).await?;
    let (mut br, mut bw) = tcp.into_split();
    let (mut scrittura, mut lettura) = canale.dividi();

    let verso_backend = async move {
        while let Some(q) = lettura.ricevi().await? {
            bw.write_all(&rx.apri(&q)?).await?;
        }
        let _ = bw.shutdown().await;
        Risultato::Ok(())
    };
    let verso_client = async move {
        let mut buf = vec![0u8; PEZZO];
        loop {
            let letti = br.read(&mut buf).await?;
            if letti == 0 {
                break;
            }
            scrittura.invia(&tx.sigilla(&buf[..letti])).await?;
        }
        scrittura.finish();
        Risultato::Ok(())
    };
    let (a, b) = tokio::join!(verso_backend, verso_client);
    a?;
    b?;
    Ok(())
}

/// Valore di `Content-Length` in un'intestazione HTTP, se presente.
fn lunghezza_corpo(intestazione: &[u8]) -> Option<usize> {
    let testo = String::from_utf8_lossy(intestazione).to_ascii_lowercase();
    testo
        .lines()
        .find_map(|r| r.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse().ok())
}

// ======================================================================
//  Lato client: `nyctalus browse`
// ======================================================================

pub async fn browse(
    indirizzo_nyct: &str,
    percorso: &str,
    elenco: Vec<NodoRete>,
    manuale: Option<NodoRete>,
    offusca: Option<[u8; 32]>,
    salti: usize,
) -> Risultato<()> {
    let chiave = indirizzo::in_chiave(indirizzo_nyct)?;
    // Stretta Noise NK end-to-end col servizio (il client è il mittente).
    let (mut canale, segreti) = if manuale.is_none() {
        collega_e_stringi(&chiave, &elenco, offusca, salti, false).await?   // salta le registrazioni morte
    } else {
        let mut canale = collega_al_servizio(&chiave, &elenco, manuale, offusca, salti, false).await?;
        let (mittente, primo) = Mittente::inizia(&chiave)?;
        canale.invia(&primo).await?;
        let risposta = canale.ricevi().await?.ok_or("no handshake reply")?;
        let segreti = mittente.completa(&risposta)?;
        (canale, segreti)
    };
    let mut tx = Record::nuovo(&segreti.verso_destinatario);
    let mut rx = Record::nuovo(&segreti.verso_mittente);

    let richiesta =
        format!("GET {percorso} HTTP/1.0\r\nHost: {indirizzo_nyct}\r\nConnection: close\r\n\r\n");
    canale.invia(&tx.sigilla(richiesta.as_bytes())).await?;

    // Leggi le intestazioni, poi il corpo per Content-Length (se c'è) o fino a
    // fine stream: così si termina senza dipendere dal momento della chiusura.
    let mut buffer = Vec::new();
    let fine = loop {
        if let Some(p) = trova_fine_intestazione(&buffer) {
            break p;
        }
        match canale.ricevi().await? {
            Some(q) => buffer.extend_from_slice(&rx.apri(&q)?),
            None => break buffer.len(),
        }
    };
    if let Some(len) = lunghezza_corpo(&buffer[..fine.min(buffer.len())]) {
        let totale = fine + len;
        while buffer.len() < totale {
            match canale.ricevi().await? {
                Some(q) => buffer.extend_from_slice(&rx.apri(&q)?),
                None => break,
            }
        }
        buffer.truncate(totale.min(buffer.len()));
    } else {
        while let Some(q) = canale.ricevi().await? {
            buffer.extend_from_slice(&rx.apri(&q)?);
        }
    }
    canale.finish();
    use std::io::Write as _;
    std::io::stdout().write_all(&buffer)?;
    std::io::stdout().flush()?;
    Ok(())
}

// ======================================================================
//  Integrazione col proxy SOCKS: `.nyct` → rendezvous
// ======================================================================

/// Quante registrazioni prova il client prima di arrendersi (vedi `collega_e_stringi`).
const TENTATIVI_STRETTA: usize = 12;
/// Quanto si aspetta la risposta del servizio alla stretta di mano.
const ATTESA_STRETTA: std::time::Duration = std::time::Duration::from_secs(6);

/// Collega al servizio e fa la stretta di mano Noise NK.
///
/// Il punto d'incontro tiene le registrazioni del servizio in coda e ne dà una a
/// ogni client. Se il servizio è stato riavviato, le sue vecchie registrazioni
/// possono restare lì «morte» (il circuito che le teneva è caduto, ma la caduta non
/// è arrivata al punto d'incontro): il client viene collegato a una di quelle e la
/// stretta non riceve risposta. Allora si scarta quella e si chiede la successiva —
/// ogni tentativo a vuoto dura pochi millisecondi — finché si trova la registrazione
/// viva del servizio.
async fn collega_e_stringi(
    chiave: &[u8; 32],
    elenco: &[NodoRete],
    offusca: Option<[u8; 32]>,
    salti: usize,
    mix: bool,
) -> Risultato<(Canale, SegretiSessione)> {
    let mut ultimo = String::from("nessun tentativo");
    for _ in 0..TENTATIVI_STRETTA {
        // il punto d'incontro non trova proprio il servizio: inutile insistere
        let mut canale = collega_al_servizio(chiave, elenco, None, offusca, salti, mix).await?;
        let (mittente, primo) = Mittente::inizia(chiave)?;
        if canale.invia(&primo).await.is_err() {
            ultimo = "registrazione caduta durante l'invio".into();
            continue;
        }
        match tokio::time::timeout(ATTESA_STRETTA, canale.ricevi()).await {
            Ok(Ok(Some(risposta))) => match mittente.completa(&risposta) {
                Ok(segreti) => return Ok((canale, segreti)),
                Err(e) => return Err(e.into()), // risposta sbagliata: non è il servizio giusto
            },
            Ok(Ok(None)) | Ok(Err(_)) => ultimo = "registrazione morta (nessuna risposta alla stretta)".into(),
            Err(_) => ultimo = "il servizio non ha risposto in tempo".into(),
        }
    }
    Err(format!("servizio non raggiungibile dopo {TENTATIVI_STRETTA} registrazioni: {ultimo}").into())
}

pub async fn tunnel_socks(
    mut cliente: TcpStream,
    indirizzo_nyct: &str,
    elenco: Vec<NodoRete>,
    offusca: Option<[u8; 32]>,
    salti: usize,
    mix: bool,
) -> Risultato<()> {
    let chiave = match indirizzo::in_chiave(indirizzo_nyct) {
        Ok(k) => k,
        Err(_) => {
            cliente.write_all(&socks5::risposta_richiesta(Esito::ConnessioneRifiutata)).await?;
            return Ok(());
        }
    };

    // Collega e fai la stretta prima di confermare al client SOCKS.
    let avviato = collega_e_stringi(&chiave, &elenco, offusca, salti, mix).await;

    let (canale, segreti) = match avviato {
        Ok(v) => v,
        Err(e) => {
            // il motivo nel registro: al client SOCKS arriva solo «rifiutato»
            eprintln!("{indirizzo_nyct}: servizio non raggiunto: {e}");
            cliente.write_all(&socks5::risposta_richiesta(Esito::ConnessioneRifiutata)).await?;
            return Ok(());
        }
    };
    cliente.write_all(&socks5::risposta_richiesta(Esito::Riuscito)).await?;

    let mut tx = Record::nuovo(&segreti.verso_destinatario);
    let mut rx = Record::nuovo(&segreti.verso_mittente);
    let (mut app_lettura, mut app_scrittura) = cliente.into_split();
    let (mut scrittura, mut lettura) = canale.dividi();

    // Ponte bidirezionale: due task indipendenti (cancellation-safe).
    let andata = async move {
        let mut buffer = vec![0u8; PEZZO];
        loop {
            let letti = app_lettura.read(&mut buffer).await?;
            if letti == 0 {
                break;
            }
            scrittura.invia(&tx.sigilla(&buffer[..letti])).await?;
        }
        scrittura.finish();
        Risultato::Ok(())
    };
    let ritorno = async move {
        while let Some(q) = lettura.ricevi().await? {
            app_scrittura.write_all(&rx.apri(&q)?).await?;
        }
        let _ = app_scrittura.shutdown().await;
        Risultato::Ok(())
    };
    let (a, b) = tokio::join!(andata, ritorno);
    a?;
    b?;
    Ok(())
}

// ======================================================================
//  Utilità comuni
// ======================================================================

/// L'identità del servizio chiesta al motore della chat (`IDENTITA` sulla sua porta di controllo,
/// con il segreto `NYCTALUS_CHAT_TOKEN`): con i dati cifrati la chiave privata non sta in chiaro su
/// un file. Se la chat è ancora chiusa (aspetta la passphrase) si riprova finché non si apre.
async fn identita_dalla_chat(chat: SocketAddr) -> Risultato<Identita> {
    use tokio::io::AsyncBufReadExt;
    let segreto = std::env::var("NYCTALUS_CHAT_TOKEN").ok().filter(|s| !s.is_empty());
    let mut avvisato = false;
    loop {
        if let Ok(f) = TcpStream::connect(chat).await {
            let (lettura, mut scrittura) = f.into_split();
            let mut lettore = tokio::io::BufReader::new(lettura);
            let mut pronto = true;
            if let Some(s) = &segreto {
                pronto = scrittura.write_all(format!("AUTH {s}\n").as_bytes()).await.is_ok();
            }
            if pronto && scrittura.write_all(b"IDENTITA\n").await.is_ok() {
                let mut riga = String::new();
                while tokio::time::timeout(std::time::Duration::from_secs(5), lettore.read_line(&mut riga))
                    .await
                    .is_ok_and(|r| r.is_ok_and(|n| n > 0))
                {
                    if let Some(h) = riga.trim_end().strip_prefix("IDENTITA ") {
                        let byte: Option<Vec<u8>> = (0..h.len())
                            .step_by(2)
                            .map(|i| h.get(i..i + 2).and_then(|x| u8::from_str_radix(x, 16).ok()))
                            .collect();
                        return Ok(Identita::da_byte(&byte.ok_or("identità dalla chat non valida")?)?);
                    }
                    if riga.starts_with("BLOCCATA") || riga.starts_with("ERR") {
                        break;
                    }
                    riga.clear();
                }
            }
        }
        if !avvisato {
            println!("Aspetto l'identità dal motore della chat su {chat} (sbloccala con la passphrase)…");
            avvisato = true;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

pub(crate) fn carica_o_crea_identita(percorso: &Path) -> Risultato<Identita> {
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

fn percorso_richiesto(richiesta: &[u8]) -> String {
    let testo = String::from_utf8_lossy(richiesta);
    let prima_riga = testo.lines().next().unwrap_or("");
    let mut campi = prima_riga.split_whitespace();
    let _metodo = campi.next();
    campi.next().unwrap_or("/").to_string()
}

fn trova_fine_intestazione(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
}

fn vuole_keep_alive(intestazione: &[u8]) -> bool {
    let testo = String::from_utf8_lossy(intestazione).to_ascii_lowercase();
    let http11 = testo.lines().next().is_some_and(|r| r.contains("http/1.1"));
    let chiede_chiusura = testo.contains("connection: close");
    let chiede_keep = testo.contains("connection: keep-alive");
    (http11 && !chiede_chiusura) || chiede_keep
}

fn apri_risorsa(dir: &Path, percorso: &str) -> (&'static str, &'static str, Vec<u8>) {
    let grezzo = percorso.split('?').next().unwrap_or("/");
    let mut relativo = grezzo.trim_start_matches('/').to_string();
    if relativo.is_empty() {
        relativo = "index.html".to_string();
    }
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
/// chiave per direzione, dai segreti della sessione Noise).
pub(crate) struct Record {
    aead: ChaCha20Poly1305,
    contatore: u64,
}

impl Record {
    pub(crate) fn nuovo(chiave: &[u8; 32]) -> Self {
        Self { aead: ChaCha20Poly1305::new(&Key::from(*chiave)), contatore: 0 }
    }

    fn nonce(&mut self) -> Nonce {
        let mut byte = [0u8; 12];
        byte[..8].copy_from_slice(&self.contatore.to_le_bytes());
        self.contatore += 1;
        Nonce::from(byte)
    }

    pub(crate) fn sigilla(&mut self, chiaro: &[u8]) -> Vec<u8> {
        let nonce = self.nonce();
        self.aead.encrypt(&nonce, chiaro).expect("ChaCha20-Poly1305 in memoria non fallisce")
    }

    pub(crate) fn apri(&mut self, cifrato: &[u8]) -> Risultato<Vec<u8>> {
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

    #[test]
    fn registrazione_provata_ammessa_falsa_ripetuta_o_senza_prova_rifiutata() {
        let servizio = Identita::genera().unwrap();
        let segreto_nodo = [33u8; 32];
        let nodo = x25519_dalek_pub(&segreto_nodo);
        // un servizio vecchio (senza prova) entra finché il servizio non si registra con la prova
        let altra = Identita::genera().unwrap();
        assert_eq!(ammetti_con(false, &segreto_nodo, &altra.pubblica(), None), Ammissione::Rifiutata, "di serie: senza prova no");
        assert_eq!(ammetti_con(true, &segreto_nodo, &altra.pubblica(), None), Ammissione::NonProvata, "nodo compatibile: sì");
        let c = registrazione(&servizio, &nodo);
        let (_, k, prova) = analizza_controllo_provato(&c).unwrap();
        assert_eq!(k, servizio.pubblica());
        assert_eq!(ammetti(&segreto_nodo, &k, prova), Ammissione::Provata);
        assert_eq!(ammetti(&segreto_nodo, &k, prova), Ammissione::Rifiutata, "la stessa prova due volte: replica");
        assert_eq!(ammetti_con(true, &segreto_nodo, &k, None), Ammissione::Rifiutata, "anche compatibile: senza prova per un servizio che prova = impostore");
        // un impostore senza la chiave privata
        let impostore = Identita::genera().unwrap();
        let mut falsa = registrazione(&impostore, &nodo);
        falsa[MAGIA.len() + 1..MAGIA.len() + 33].copy_from_slice(&servizio.pubblica());
        let (_, k2, p2) = analizza_controllo_provato(&falsa).unwrap();
        assert_eq!(ammetti(&segreto_nodo, &k2, p2), Ammissione::Rifiutata, "prova di un altro");
        // prova scaduta
        let t = adesso_s() - 2 * FINESTRA_PROVA_S;
        let vecchia = (t, [1u8; 8], servizio.prova_registrazione(&nodo, t, &[1u8; 8]));
        // due registrazioni nello stesso secondo (riserva di circuiti): prove diverse, entrambe ammesse
        let (_, _, a) = analizza_controllo_provato(&registrazione(&servizio, &nodo)).unwrap();
        let (_, _, b) = analizza_controllo_provato(&registrazione(&servizio, &nodo)).unwrap();
        assert_eq!(ammetti(&segreto_nodo, &servizio.pubblica(), a), Ammissione::Provata);
        assert_eq!(ammetti(&segreto_nodo, &servizio.pubblica(), b), Ammissione::Provata);
        assert_eq!(ammetti(&segreto_nodo, &servizio.pubblica(), Some(vecchia)), Ammissione::Rifiutata, "fuori tempo");
    }

    fn x25519_dalek_pub(segreto: &[u8; 32]) -> [u8; 32] {
        nyctalus_core::circuito::chiave_pubblica(segreto)
    }

    #[test]
    fn controllo_e_analisi() {
        let k = [9u8; 32];
        let c = controllo(REGISTRA, &k);
        assert_eq!(analizza_controllo(&c).unwrap(), (REGISTRA, k));
        assert!(analizza_controllo(b"corto").is_err());
    }

    #[test]
    fn percorso_onion_finisce_al_rendezvous() {
        // la guardia è condivisa con le prove di `percorsi`: una alla volta
        let _b = crate::percorsi::test::blocco();
        let nodo = |p: u16| -> NodoRete {
            (format!("127.0.0.1:{p}").parse().unwrap(), [p as u8; 32], Some([0u8; 32]))
        };
        let elenco = vec![nodo(1), nodo(2), nodo(3), nodo(4)];
        let p = percorso_onion(&elenco, 2, 3).unwrap();
        assert_eq!(p.last().unwrap().0, elenco[2].0); // ultimo = rendezvous
        assert_eq!(p.len(), 3);
    }
}
