// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
//! `nyctalus chat`: motore della chat `.nyct`, senza server e senza metadati.
//!
//! Ogni utente è un servizio `.nyct` (il suo indirizzo è il suo Contact ID).
//! Il trasporto riusa la rete esistente:
//!   - **entrata**  il demone `site --backend 127.0.0.1:PORTA` pubblica il nostro
//!     `.nyct` e inoltra le connessioni dei contatti a questa `PORTA`, su cui il
//!     motore ascolta;
//!   - **uscita**   per scrivere a un contatto apriamo una connessione SOCKS5
//!     verso `contatto.nyct` tramite il proxy del circuito (`start`, 127.0.0.1:1080),
//!     che instrada col punto d'incontro e cifra end-to-end fino al servizio.
//!
//! L'handshake di trasporto (Noise NK) autentica **il servizio** verso chi si
//! collega. Per autenticare **chi si collega** (il mittente) eseguiamo, dentro
//! il tunnel già cifrato, un secondo handshake Noise NK *in senso inverso*: il
//! servizio sfida l'indirizzo dichiarato e solo chi possiede quella chiave
//! privata può concluderlo. Così l'identità del contatto è provata
//! crittograficamente, riusando la stessa crypto del resto del protocollo.
//!
//! Un'interfaccia grafica (il client GTK) parla col motore su una porta di
//! **controllo** locale (solo loopback), con un protocollo a righe di testo.
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use nyctalus_core::stretta::{self, Identita, Mittente, SegretiSessione};
use nyctalus_core::{casella, indirizzo};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::incontro::Record;
use crate::{in_esadecimale, Risultato};

/// Avvia il motore della chat: ascolta i contatti in entrata (backend), serve
/// l'interfaccia grafica sulla porta di controllo e compone le connessioni in
/// uscita tramite il proxy SOCKS.
#[allow(clippy::too_many_arguments)] // le opzioni del comando `chat`, una per una
pub async fn chat(
    identita_path: PathBuf,
    backend: SocketAddr,
    controllo: SocketAddr,
    socks: SocketAddr,
    dati: PathBuf,
    max_file: u64,
    segreto: Option<String>,
    rubrica: Option<String>,
    casella: Option<String>,
) -> Risultato<()> {
    std::fs::create_dir_all(&dati)?;
    let identita = carica_identita(&identita_path)?;
    let indirizzo = indirizzo::da_chiave(&identita.pubblica());
    let stato = Arc::new(Stato {
        identita,
        indirizzo: indirizzo.clone(),
        socks,
        dati: dati.clone(),
        contatti: Mutex::new(carica_contatti(&dati)),
        sessioni: Mutex::new(HashMap::new()),
        composizione: tokio::sync::Mutex::new(()),
        prossima_sessione: AtomicU64::new(1),
        iscritti: Mutex::new(Vec::new()),
        in_arrivo: Mutex::new(HashMap::new()),
        max_file,
        segreto,
        visti: Mutex::new(carica_visti(&dati)),
        rubrica: rubrica.map(|a| con_suffisso(&a)),
        casella: casella.map(|a| con_suffisso(&a)),
        irraggiungibili: Mutex::new(HashSet::new()),
        posta_vista: Mutex::new(carica_posta_vista(&dati)),
        in_volo: Mutex::new(HashMap::new()),
        pendenti: Mutex::new(HashMap::new()),
        letti_inviati: Mutex::new(HashMap::new()),
        battito_noto: Mutex::new(HashSet::new()),
        inviti: Mutex::new(HashMap::new()),
    });

    println!("Nyctalus chat");
    println!("Il tuo Contact ID:  {indirizzo}");
    println!("Entrata (backend):  {backend}   ← punta qui il `site --backend`");
    println!("Controllo (UI):     {controllo}");
    println!("Uscita (SOCKS):     {socks}");
    println!("Dati:               {}", dati.display());
    println!("Allegati fino a:    {} MB", max_file / (1024 * 1024));
    if let Some(c) = &stato.casella {
        println!("Casella cifrata:    {c}");
    }

    let ascoltatore = TcpListener::bind(backend).await?;
    let st_ctrl = stato.clone();
    tokio::spawn(async move {
        if let Err(e) = st_ctrl.servi_controllo(controllo).await {
            eprintln!("controllo: {e}");
        }
    });

    // «in linea» senza dover scrivere prima: si bussa ai contatti
    tokio::spawn(stato.clone().sonda_presenza());
    // la posta lasciata nella casella mentre eravamo spenti
    if stato.rubrica.is_some() {
        tokio::spawn(stato.clone().ritira_posta());
    }

    loop {
        let (flusso, _) = ascoltatore.accept().await?;
        let st = stato.clone();
        tokio::spawn(async move {
            if let Err(e) = st.gestisci_entrante(flusso).await {
                eprintln!("sessione in entrata: {e}");
            }
        });
    }
}

/// Stato condiviso del motore.
struct Stato {
    identita: Identita,
    indirizzo: String,                                       // il nostro .nyct
    socks: SocketAddr,
    dati: PathBuf,
    contatti: Mutex<Vec<(String, String)>>,                  // (indirizzo, soprannome hex)
    sessioni: Mutex<HashMap<String, (u64, mpsc::Sender<Vec<u8>>)>>, // indirizzo → (n° sessione, scrittore limitato)
    composizione: tokio::sync::Mutex<()>,                    // un solo collegamento in uscita alla volta
    prossima_sessione: AtomicU64,                            // numera le sessioni (vedi abbandona_in_arrivo)
    iscritti: Mutex<Vec<mpsc::UnboundedSender<String>>>,     // UI in ascolto degli eventi
    in_arrivo: Mutex<HashMap<(String, String), InArrivo>>,   // (mittente, id) → allegato in arrivo
    max_file: u64,                                           // dimensione massima di un allegato
    segreto: Option<String>,                                 // se c'è, la UI deve presentarlo (AUTH)
    visti: Mutex<HashMap<String, u64>>,                      // ultimo accesso visto da noi (solo locale)
    rubrica: Option<String>,                                 // .nyct della rubrica della rete (ritiro della posta)
    casella: Option<String>,                                 // .nyct della casella (deposito anonimo)
    irraggiungibili: Mutex<HashSet<String>>,                 // contatti che non hanno risposto: posta diretta in casella
    posta_vista: Mutex<HashSet<String>>,                     // id delle buste già ricevute (niente doppioni)
    in_volo: Mutex<HashMap<String, Vec<InVolo>>>,            // testi spediti senza ancora «ricevuto»
    pendenti: Mutex<HashMap<String, Vec<Vec<u8>>>>,          // conferme da spedire appena c'è la sessione
    letti_inviati: Mutex<HashMap<String, u64>>,              // ultimo «letto» mandato a ciascun contatto
    battito_noto: Mutex<HashSet<String>>,                    // contatti che rispondono al battito (PING/PONG)
    inviti: Mutex<HashMap<String, (String, u64)>>,           // chiamate che squillano: contatto → (id, quando)
}

/// Un invito a una chiamata vale per tanto (poi chi chiama riaggancia da sé).
const SQUILLO: u64 = 45;

/// Pacchetto audio massimo di una chiamata (Opus da 20 ms sta ben sotto).
const AUDIO_MAX: usize = 1500;

fn id_chiamata_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Battito delle sessioni: ogni quanto si chiede «ci sei?» e dopo quanto silenzio la si chiude.
const BATTITO: std::time::Duration = std::time::Duration::from_secs(25);
const SILENZIO_MAX: std::time::Duration = std::time::Duration::from_secs(75);

/// Un messaggio spedito su una sessione e non ancora confermato («K»): se la sessione cade
/// prima della conferma, il testo va nella casella (il destinatario scarta i doppioni).
struct InVolo {
    sessione: u64,
    ts: u64,
    testo: Vec<u8>,
}

/// Testo massimo di un messaggio lasciato nella casella (gli allegati solo in linea).
const TESTO_CASELLA_MAX: usize = 16 * 1024;
/// Ritiro della posta: primo giro dopo questo ritardo, poi a ogni giro (la rubrica la
/// consegna anche da sola appena risultiamo in linea).
const RITIRO_INIZIO: std::time::Duration = std::time::Duration::from_secs(12);
const RITIRO_GIRO: std::time::Duration = std::time::Duration::from_secs(60);
/// Un testo senza «ricevuto» dopo questo tempo: la sessione è data per morta (il contatto è
/// sparito senza chiuderla, es. modalità aereo) e il testo va nella casella.
const ATTESA_RICEVUTO: std::time::Duration = std::time::Duration::from_secs(45);
/// Tempo massimo per lasciare una busta nella casella (circuito + punto d'incontro).
const DEPOSITO_TEMPO: std::time::Duration = std::time::Duration::from_secs(90);

/// Sonda di presenza: primo giro dopo questo ritardo (la rete deve salire), poi ogni giro.
const SONDA_INIZIO: std::time::Duration = std::time::Duration::from_secs(8);
const SONDA_GIRO: std::time::Duration = std::time::Duration::from_secs(15);
/// Attesa fra due tentativi verso un contatto non raggiungibile: raddoppia fino al massimo.
const SONDA_PAUSA_MIN: u64 = 15;
const SONDA_PAUSA_MAX: u64 = 600;
const SONDA_TEMPO: std::time::Duration = std::time::Duration::from_secs(40);

/// Un allegato che sta arrivando: si scrive in `<nome>.part` e diventa
/// definitivo solo se dimensione e impronta BLAKE3 tornano.
struct InArrivo {
    sessione: u64, // la sessione da cui arriva: se cade quella, si butta
    file: std::fs::File,
    parziale: PathBuf,
    finale: PathBuf,
    nome: String,
    mime: String,
    totale: u64,
    ricevuti: u64,
    impronta: blake3::Hasher,
    ts: u64,
    ultimo_avviso: u64,
}

/// Blocco dati di un allegato (16 KB): sta comodo in un frame da 64 KB.
const BLOCCO: usize = 16 * 1024;
/// Ogni quanto avvisare l'interfaccia sull'avanzamento.
const PASSO_AVANZAMENTO: u64 = 256 * 1024;
/// Allegati contemporanei in arrivo dallo stesso contatto.
const MAX_IN_ARRIVO: usize = 8;
/// Coda dello scrittore di sessione: dà contropressione agli invii lunghi.
const CODA_SESSIONE: usize = 32;

impl Stato {
    // --- eventi verso le interfacce iscritte ---------------------------------
    fn notifica(&self, riga: String) {
        let riga = format!("{riga}\n");
        self.iscritti.lock().unwrap().retain(|s| s.send(riga.clone()).is_ok());
    }

    // --- connessioni in entrata (siamo il servizio) --------------------------
    async fn gestisci_entrante(self: Arc<Self>, mut flusso: TcpStream) -> Risultato<()> {
        // 1) il contatto dichiara il proprio indirizzo
        let saluto = leggi_frame(&mut flusso).await?.ok_or("nessun saluto")?;
        let dichiarato = saluto.strip_prefix(b"H").ok_or("saluto malformato")?;
        let peer = std::str::from_utf8(dichiarato)?.trim().to_string();
        let chiave = indirizzo::in_chiave(&peer).map_err(|_| "indirizzo del contatto non valido")?;

        // 2) lo sfidiamo con un Noise NK verso quella chiave: solo chi possiede
        //    la chiave privata del .nyct dichiarato può rispondere.
        let (mittente, msg1) = Mittente::inizia(&chiave)?;
        scrivi_frame(&mut flusso, &msg1).await?;
        let msg2 = leggi_frame(&mut flusso).await?.ok_or("nessuna prova d'identità")?;
        let segreti = mittente.completa(&msg2)?; // errore = impostore → chiusura

        self.avvia_sessione(flusso, peer, segreti, true);
        Ok(())
    }

    // --- connessione in uscita (ci colleghiamo a un contatto) ----------------
    async fn componi(self: &Arc<Self>, indirizzo_contatto: &str) -> Risultato<()> {
        let host = con_suffisso(indirizzo_contatto);
        indirizzo::in_chiave(&host).map_err(|_| "indirizzo .nyct non valido")?;
        let mut flusso = socks5_connetti(self.socks, &host, 1).await?;

        // 1) ci presentiamo con il nostro indirizzo
        let mut saluto = vec![b'H'];
        saluto.extend_from_slice(self.indirizzo.as_bytes());
        scrivi_frame(&mut flusso, &saluto).await?;

        // 2) rispondiamo alla sfida col nostro keyfile: proviamo di essere noi
        let msg1 = leggi_frame(&mut flusso).await?.ok_or("nessuna sfida dal servizio")?;
        let (msg2, segreti) = stretta::rispondi(&self.identita, &msg1)?;
        scrivi_frame(&mut flusso, &msg2).await?;

        self.avvia_sessione(flusso, host, segreti, false);
        Ok(())
    }

    /// Mette in piedi i due task (lettura/scrittura) di una sessione già
    /// autenticata e registra lo scrittore. `sono_iniziatore` = abbiamo iniziato
    /// l'handshake *interno* (lato servizio), così scegliamo le chiavi giuste.
    fn avvia_sessione(
        self: &Arc<Self>,
        flusso: TcpStream,
        peer: String,
        segreti: SegretiSessione,
        sono_iniziatore: bool,
    ) {
        let (mut lettura, mut scrittura) = flusso.into_split();
        // Nella stretta NK: verso_destinatario = iniziatore→risponditore,
        // verso_mittente = risponditore→iniziatore.
        let (mut tx, mut rx) = if sono_iniziatore {
            (Record::nuovo(&segreti.verso_destinatario), Record::nuovo(&segreti.verso_mittente))
        } else {
            (Record::nuovo(&segreti.verso_mittente), Record::nuovo(&segreti.verso_destinatario))
        };

        let (ch_tx, mut ch_rx) = mpsc::channel::<Vec<u8>>(CODA_SESSIONE);
        let sid = self.prossima_sessione.fetch_add(1, Ordering::Relaxed);
        // le conferme rimaste in attesa partono per prime
        for frame in self.pendenti.lock().unwrap().remove(&peer).unwrap_or_default() {
            let _ = ch_tx.try_send(frame);
        }
        let tx_battito = ch_tx.clone();
        self.sessioni.lock().unwrap().insert(peer.clone(), (sid, ch_tx));
        self.irraggiungibili.lock().unwrap().remove(&peer);
        self.notifica(format!("ST {peer} 1"));
        self.segna_visto(&peer);

        // scrittore: sigilla ogni messaggio in chiaro e lo spedisce
        let scrittore = tokio::spawn(async move {
            while let Some(chiaro) = ch_rx.recv().await {
                let frame = tx.sigilla(&chiaro);
                if scrivi_frame(&mut scrittura, &frame).await.is_err() {
                    break;
                }
            }
        });

        // battito: ogni BATTITO si chiede «ci sei?» (PING); se il contatto risponde di solito
        // (versione col battito) e tace da SILENZIO_MAX, la sessione è morta senza saperlo
        // (telefono spento di colpo, modalità aereo): si chiude, e il contatto non è più «in linea»
        let ultimo = Arc::new(AtomicU64::new(ora()));
        let morta = Arc::new(tokio::sync::Notify::new());
        {
            let (tx_b, ultimo, morta, stato, peer) = (tx_battito, ultimo.clone(), morta.clone(), self.clone(), peer.clone());
            tokio::spawn(async move {
                loop {
                    tokio::time::sleep(BATTITO).await;
                    if tx_b.is_closed() {
                        return;
                    }
                    let _ = tx_b.try_send(b"PING".to_vec());
                    let muta = ora().saturating_sub(ultimo.load(Ordering::Relaxed)) > SILENZIO_MAX.as_secs();
                    if muta && stato.battito_noto.lock().unwrap().contains(&peer) {
                        morta.notify_one();
                        return;
                    }
                }
            });
        }

        // lettore: apre i messaggi e li consegna
        let stato = self.clone();
        tokio::spawn(async move {
            loop {
                let frame = tokio::select! {
                    f = leggi_frame(&mut lettura) => f,
                    _ = morta.notified() => break,   // nessun battito: sessione morta
                };
                ultimo.store(ora(), Ordering::Relaxed);
                match frame {
                    Ok(Some(frame)) => match rx.apri(&frame) {
                        Ok(chiaro) => stato.ricevi_messaggio(&peer, sid, &chiaro),
                        Err(_) => break, // frame corrotto: chiudiamo
                    },
                    _ => break, // EOF o errore
                }
            }
            scrittore.abort();
            // si toglie dalla mappa solo se è ancora questa la sessione registrata: con
            // due collegamenti allo stesso contatto, l'altro resta valido (e «in linea»)
            let era_lei = {
                let mut g = stato.sessioni.lock().unwrap();
                let lei = g.get(&peer).is_some_and(|(n, _)| *n == sid);
                if lei {
                    g.remove(&peer);
                }
                lei
            };
            stato.abbandona_in_arrivo(&peer, sid);
            stato.rimanda_in_casella(&peer, sid);
            if era_lei {
                stato.notifica(format!("ST {peer} 0"));
                stato.segna_visto(&peer);
            }
        });
    }

    /// Messaggi applicativi sulla sessione interna:
    ///   `M <ts> <testo>`                         testo (UTF-8, anche con spazi)
    ///   `F <id> <ts> <dim> <mime> <nome hex>`    inizio di un allegato
    ///   `D <id> <byte…>`                         un blocco dell'allegato
    ///   `E <id> <blake3 hex>`                    fine: impronta di tutto il file
    fn ricevi_messaggio(&self, peer: &str, sid: u64, chiaro: &[u8]) {
        if let Some(r) = chiaro.strip_prefix(b"D ") {
            return self.ricevi_blocco(peer, r);
        }
        if let Some(r) = chiaro.strip_prefix(b"F ") {
            return self.ricevi_intestazione(peer, sid, r);
        }
        if let Some(r) = chiaro.strip_prefix(b"E ") {
            return self.ricevi_fine(peer, r);
        }
        // chiamate: audio «CX <seq 4 byte><opus>» e segnali «CI|CA|CN|CF <id>» (invito,
        // accettata, rifiutata, fine). Il motore li passa all'interfaccia così come sono:
        // la chiamata la gestisce l'interfaccia (microfono, altoparlante).
        if let Some(r) = chiaro.strip_prefix(b"CX ") {
            if r.len() > 4 && r.len() <= 4 + AUDIO_MAX {
                let seq = u32::from_be_bytes([r[0], r[1], r[2], r[3]]);
                self.notifica(format!("AUD {peer} {seq} {}", in_esadecimale(&r[4..])));
            }
            return;
        }
        for (prefisso, evento) in [(&b"CI "[..], "CALLIN"), (b"CA ", "CALLOK"), (b"CN ", "CALLNO"), (b"CF ", "CALLEND")] {
            if let Some(r) = chiaro.strip_prefix(prefisso) {
                if let Some(id) = std::str::from_utf8(r).ok().filter(|i| id_chiamata_valido(i)) {
                    // l'invito si ricorda finché squilla: chi apre l'interfaccia dopo lo vede
                    let mut inviti = self.inviti.lock().unwrap();
                    if evento == "CALLIN" {
                        inviti.insert(peer.to_string(), (id.to_string(), ora()));
                    } else if inviti.get(peer).is_some_and(|(i, _)| i == id) {
                        inviti.remove(peer);
                    }
                    drop(inviti);
                    self.notifica(format!("{evento} {peer} {id}"));
                }
                return;
            }
        }
        // battito: si risponde a PING; un PONG dice che il contatto ha il battito
        if chiaro == b"PING" {
            if let Some((_, s)) = self.sessioni.lock().unwrap().get(peer) {
                let _ = s.try_send(b"PONG".to_vec());
            }
            return;
        }
        if chiaro == b"PONG" {
            self.battito_noto.lock().unwrap().insert(peer.to_string());
            return;
        }
        // spunte: «K ts» = il contatto ha ricevuto, «R ts» = ha letto fino a ts
        if let Some(r) = chiaro.strip_prefix(b"K ") {
            if let Some(ts) = std::str::from_utf8(r).ok().and_then(|t| t.trim().parse::<u64>().ok()) {
                if let Some(v) = self.in_volo.lock().unwrap().get_mut(peer) {
                    v.retain(|m| m.ts != ts);
                }
                self.segna(peer, ts, "2");
            }
            return;
        }
        if let Some(r) = chiaro.strip_prefix(b"R ") {
            if let Some(ts) = std::str::from_utf8(r).ok().and_then(|t| t.trim().parse::<u64>().ok()) {
                self.segna(peer, ts, "3");
            }
            return;
        }
        let Some(resto) = chiaro.strip_prefix(b"M ") else { return };
        let Some(sp) = resto.iter().position(|&b| b == b' ') else { return };
        let ts = std::str::from_utf8(&resto[..sp]).ok().and_then(|t| t.parse::<u64>().ok()).unwrap_or_else(ora);
        let testo = &resto[sp + 1..];
        if self.rubrica.as_deref() == Some(peer) && testo.starts_with(b"CASELLA ") {
            return self.posta_dalla_rubrica(peer, &testo[8..]);
        }
        self.registra(peer, "in", ts, testo);
        if !self.contatti.lock().unwrap().iter().any(|(a, _)| a == peer) {
            self.notifica(format!("NEW {peer}"));
        }
        self.notifica(format!("MSG {peer} {ts} {}", in_esadecimale(testo)));
        if self.rubrica.as_deref() != Some(peer) {
            self.conferma(peer, format!("K {ts}").into_bytes());
        }
    }

    /// Lo scrittore della sessione con un contatto, aprendola se serve. Le aperture
    /// sono una alla volta: due invii simultanei (es. due foto) usano la STESSA
    /// sessione invece di aprirne due.
    async fn sessione(self: &Arc<Self>, addr: &str) -> Risultato<mpsc::Sender<Vec<u8>>> {
        let esistente = || self.sessioni.lock().unwrap().get(addr).map(|(_, s)| s.clone());
        if let Some(s) = esistente() {
            return Ok(s);
        }
        let _turno = self.composizione.lock().await;
        if let Some(s) = esistente() {
            return Ok(s); // aperta da chi era in coda prima di noi
        }
        self.componi(addr).await?;
        Ok(esistente().ok_or("sessione non creata")?)
    }

    /// Invia un messaggio a un contatto, aprendo la sessione se serve.
    async fn invia_a(self: &Arc<Self>, indirizzo_contatto: &str, testo: &[u8]) -> Risultato<()> {
        let addr = con_suffisso(indirizzo_contatto);
        let ts = ora();
        let in_casella = self.casella.is_some() && self.rubrica.as_deref() != Some(addr.as_str()) && addr != self.indirizzo;
        // già noto come non raggiungibile: dritto nella casella, senza far aspettare
        if in_casella
            && !self.sessioni.lock().unwrap().contains_key(&addr)
            && self.irraggiungibili.lock().unwrap().contains(&addr)
        {
            return self.deposita(&addr, testo).await;
        }
        let scrittore = match self.sessione(&addr).await {
            Ok(s) => s,
            Err(e) => {
                self.irraggiungibili.lock().unwrap().insert(addr.clone());
                if !in_casella {
                    return Err(e);
                }
                return self.deposita(&addr, testo).await.map_err(|e2| format!("{e} — casella: {e2}").into());
            }
        };
        let mut chiaro = format!("M {ts} ").into_bytes();
        chiaro.extend_from_slice(testo);
        let sid = self.sessioni.lock().unwrap().get(&addr).map(|(n, _)| *n).unwrap_or(0);
        scrittore.send(chiaro).await.map_err(|_| "sessione chiusa")?;
        if in_casella {
            self.in_volo.lock().unwrap().entry(addr.clone()).or_default().push(InVolo { sessione: sid, ts, testo: testo.to_vec() });
            let stato = self.clone();
            let a = addr.clone();
            tokio::spawn(async move { stato.controlla_ricevuto(a, sid, ts).await });
        }
        self.registra(&addr, "out", ts, testo);
        self.notifica(format!("OUT {addr} {ts} {}", in_esadecimale(testo)));
        self.segna(&addr, ts, "1");
        Ok(())
    }

    // --- allegati: invio -----------------------------------------------------
    /// Spedisce un file a blocchi; la coda limitata della sessione fa da
    /// contropressione (il file non viene caricato tutto in memoria).
    async fn invia_file_a(self: &Arc<Self>, indirizzo_contatto: &str, percorso: &Path, mime: &str) -> Risultato<()> {
        use tokio::io::AsyncReadExt as _;
        let addr = con_suffisso(indirizzo_contatto);
        let meta = tokio::fs::metadata(percorso).await.map_err(|_| "file non trovato")?;
        if !meta.is_file() {
            return Err("non è un file".into());
        }
        let totale = meta.len();
        if totale > self.max_file {
            return Err(format!("file troppo grande (massimo {} MB)", self.max_file / (1024 * 1024)).into());
        }
        let nome = nome_sicuro(&percorso.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
        let mime = mime_sicuro(mime);
        let id = nuovo_id();
        let ts = ora();
        let nome_hex = in_esadecimale(nome.as_bytes());

        let scrittore = self.sessione(&addr).await?;
        scrittore
            .send(format!("F {id} {ts} {totale} {mime} {nome_hex}").into_bytes())
            .await
            .map_err(|_| "sessione chiusa")?;

        let mut f = tokio::fs::File::open(percorso).await?;
        let mut impronta = blake3::Hasher::new();
        let mut buf = vec![0u8; BLOCCO];
        let (mut inviati, mut ultimo) = (0u64, 0u64);
        loop {
            let n = f.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            impronta.update(&buf[..n]);
            let mut blocco = format!("D {id} ").into_bytes();
            blocco.extend_from_slice(&buf[..n]);
            scrittore.send(blocco).await.map_err(|_| "sessione chiusa durante l'invio")?;
            inviati += n as u64;
            if inviati - ultimo >= PASSO_AVANZAMENTO || inviati == totale {
                ultimo = inviati;
                self.notifica(format!("PROG {addr} out {id} {inviati} {totale} {mime} {nome_hex}"));
            }
        }
        if inviati != totale {
            return Err("il file è cambiato durante l'invio".into());
        }
        scrittore
            .send(format!("E {id} {}", impronta.finalize().to_hex()).into_bytes())
            .await
            .map_err(|_| "sessione chiusa")?;

        let percorso_hex = in_esadecimale(percorso.to_string_lossy().as_bytes());
        self.registra_riga(&addr, &format!("fout {ts} {id} {totale} {mime} {nome_hex} {percorso_hex}"));
        self.notifica(format!("FILE {addr} out {ts} {id} {totale} {mime} {nome_hex} {percorso_hex}"));
        self.segna(&addr, ts, "1");
        Ok(())
    }

    // --- allegati: ricezione -------------------------------------------------
    fn ricevi_intestazione(&self, peer: &str, sid: u64, r: &[u8]) {
        let Ok(testo) = std::str::from_utf8(r) else { return };
        let campi: Vec<&str> = testo.split(' ').collect();
        let [id, ts, dim, mime, nome_hex] = campi.as_slice() else { return };
        if !id_valido(id) {
            return;
        }
        let (Ok(ts), Ok(totale)) = (ts.parse::<u64>(), dim.parse::<u64>()) else { return };
        let nome = nome_sicuro(&String::from_utf8_lossy(&da_hex(nome_hex).unwrap_or_default()));
        let mime = mime_sicuro(mime);
        if totale > self.max_file {
            let msg = format!("allegato rifiutato: troppo grande ({} MB)", totale / (1024 * 1024));
            self.notifica(format!("FERR {peer} {id} {}", in_esadecimale(msg.as_bytes())));
            return;
        }
        let mut g = self.in_arrivo.lock().unwrap();
        if g.keys().filter(|(p, _)| p == peer).count() >= MAX_IN_ARRIVO || g.contains_key(&(peer.to_string(), id.to_string())) {
            return;
        }
        let cartella = self.dati.join("allegati").join(cartella_contatto(peer));
        if std::fs::create_dir_all(&cartella).is_err() {
            return;
        }
        let finale = percorso_libero(&cartella, &nome);
        let parziale = finale.with_extension(format!(
            "{}part",
            finale.extension().map(|e| format!("{}.", e.to_string_lossy())).unwrap_or_default()
        ));
        let Ok(file) = std::fs::File::create(&parziale) else { return };
        g.insert(
            (peer.to_string(), id.to_string()),
            InArrivo { sessione: sid, file, parziale, finale, nome, mime, totale, ricevuti: 0, impronta: blake3::Hasher::new(), ts, ultimo_avviso: 0 },
        );
        drop(g);
        let (m, n) = {
            let g = self.in_arrivo.lock().unwrap();
            let a = &g[&(peer.to_string(), id.to_string())];
            (a.mime.clone(), in_esadecimale(a.nome.as_bytes()))
        };
        self.notifica(format!("PROG {peer} in {id} 0 {totale} {m} {n}"));
    }

    fn ricevi_blocco(&self, peer: &str, r: &[u8]) {
        use std::io::Write;
        let Some(sp) = r.iter().position(|&b| b == b' ') else { return };
        let Ok(id) = std::str::from_utf8(&r[..sp]) else { return };
        let dati = &r[sp + 1..];
        let chiave = (peer.to_string(), id.to_string());
        let mut g = self.in_arrivo.lock().unwrap();
        let Some(a) = g.get_mut(&chiave) else { return };
        // più byte di quelli annunciati, o errore di scrittura: si scarta tutto
        if a.ricevuti + dati.len() as u64 > a.totale || a.file.write_all(dati).is_err() {
            let a = g.remove(&chiave).unwrap();
            drop(g);
            let _ = std::fs::remove_file(&a.parziale);
            self.notifica(format!("FERR {peer} {id} {}", in_esadecimale("allegato non valido, scartato".as_bytes())));
            return;
        }
        a.impronta.update(dati);
        a.ricevuti += dati.len() as u64;
        let avviso = a.ricevuti - a.ultimo_avviso >= PASSO_AVANZAMENTO;
        if avviso {
            a.ultimo_avviso = a.ricevuti;
        }
        let (fatti, totale) = (a.ricevuti, a.totale);
        let (m, n) = (a.mime.clone(), in_esadecimale(a.nome.as_bytes()));
        drop(g);
        if avviso {
            self.notifica(format!("PROG {peer} in {id} {fatti} {totale} {m} {n}"));
        }
    }

    fn ricevi_fine(&self, peer: &str, r: &[u8]) {
        let Ok(testo) = std::str::from_utf8(r) else { return };
        let Some((id, attesa)) = testo.split_once(' ') else { return };
        let Some(mut a) = self.in_arrivo.lock().unwrap().remove(&(peer.to_string(), id.to_string())) else { return };
        let _ = std::io::Write::flush(&mut a.file);
        drop(a.file);
        let calcolata = a.impronta.finalize().to_hex();
        if a.ricevuti != a.totale || calcolata.as_str() != attesa.trim() {
            let _ = std::fs::remove_file(&a.parziale);
            self.notifica(format!("FERR {peer} {id} {}", in_esadecimale("allegato corrotto (impronta diversa), scartato".as_bytes())));
            return;
        }
        if std::fs::rename(&a.parziale, &a.finale).is_err() {
            let _ = std::fs::remove_file(&a.parziale);
            return;
        }
        let nome_hex = in_esadecimale(a.nome.as_bytes());
        let percorso_hex = in_esadecimale(a.finale.to_string_lossy().as_bytes());
        self.registra_riga(peer, &format!("fin {} {id} {} {} {nome_hex} {percorso_hex}", a.ts, a.totale, a.mime));
        if !self.contatti.lock().unwrap().iter().any(|(c, _)| c == peer) {
            self.notifica(format!("NEW {peer}"));
        }
        self.notifica(format!("FILE {peer} in {} {id} {} {} {nome_hex} {percorso_hex}", a.ts, a.totale, a.mime));
        self.conferma(peer, format!("K {}", a.ts).into_bytes());
    }

    /// La sessione `sid` è caduta: i SUOI allegati a metà vengono buttati (quelli che
    /// arrivano da un'altra sessione con lo stesso contatto proseguono).
    fn abbandona_in_arrivo(&self, peer: &str, sid: u64) {
        let mut g = self.in_arrivo.lock().unwrap();
        let chiavi: Vec<_> = g.iter().filter(|((p, _), a)| p == peer && a.sessione == sid).map(|(k, _)| k.clone()).collect();
        for k in chiavi {
            if let Some(a) = g.remove(&k) {
                let _ = std::fs::remove_file(&a.parziale);
                self.notifica(format!("FERR {peer} {} {}", k.1, in_esadecimale("trasferimento interrotto".as_bytes())));
            }
        }
    }

    // --- presenza ------------------------------------------------------------
    /// Annota «visto adesso» (sessione aperta o appena chiusa) e lo dice alle UI.
    fn segna_visto(&self, addr: &str) {
        let ts = ora();
        let righe: String = {
            let mut v = self.visti.lock().unwrap();
            v.insert(addr.to_string(), ts);
            v.iter().map(|(a, t)| format!("{a} {t}\n")).collect()
        };
        let _ = std::fs::write(self.dati.join("visti"), righe);
        self.notifica(format!("V {addr} {ts}"));
    }

    /// Bussa ai contatti senza sessione: chi è acceso risulta subito «in linea» (e
    /// lo vede anche lui), senza dover scrivere prima. Chi non risponde viene
    /// riprovato con pause crescenti (15 s → 10 min). Non usa il turno di
    /// `composizione`: non deve far aspettare un messaggio che l'utente sta mandando.
    async fn sonda_presenza(self: Arc<Self>) {
        let mut prossimo: HashMap<String, (std::time::Instant, u64)> = HashMap::new();
        tokio::time::sleep(SONDA_INIZIO).await;
        loop {
            // i contatti, più chi aspetta una nostra conferma (es. ha scritto dalla casella)
            let mut contatti: Vec<String> = self.contatti.lock().unwrap().iter().map(|(a, _)| a.clone()).collect();
            for a in self.pendenti.lock().unwrap().keys() {
                if !contatti.contains(a) {
                    contatti.push(a.clone());
                }
            }
            for a in contatti {
                if a == self.indirizzo || self.sessioni.lock().unwrap().contains_key(&a) {
                    prossimo.remove(&a);
                    continue;
                }
                let adesso = std::time::Instant::now();
                if prossimo.get(&a).is_some_and(|(quando, _)| adesso < *quando) {
                    continue;
                }
                let ok = matches!(tokio::time::timeout(SONDA_TEMPO, self.componi(&a)).await, Ok(Ok(())));
                if ok {
                    prossimo.remove(&a);
                } else {
                    self.irraggiungibili.lock().unwrap().insert(a.clone());
                    let pausa = prossimo.get(&a).map(|(_, p)| (p * 2).min(SONDA_PAUSA_MAX)).unwrap_or(SONDA_PAUSA_MIN);
                    prossimo.insert(a, (adesso + std::time::Duration::from_secs(pausa), pausa));
                }
            }
            tokio::time::sleep(SONDA_GIRO).await;
        }
    }

    // --- casella cifrata -----------------------------------------------------
    /// Lascia un messaggio nella casella per chi non è in linea. La busta (Noise X) la apre
    /// solo il destinatario e dentro c'è la prova di chi l'ha scritta; il deposito passa dai
    /// circuiti verso l'indirizzo della casella SENZA presentarsi, quindi la casella non sa
    /// chi scrive: sa solo che c'è posta per quell'indirizzo.
    async fn deposita(self: &Arc<Self>, addr: &str, testo: &[u8]) -> Risultato<()> {
        let ts = ora();
        self.sigilla_e_deposita(addr, testo, ts).await?;
        self.registra(addr, "out", ts, testo);
        self.notifica(format!("OUT {addr} {ts} {}", in_esadecimale(testo)));
        self.segna(addr, ts, "c");
        Ok(())
    }

    /// Mette nella casella un testo già in cronologia (con la sua ora originale).
    async fn sigilla_e_deposita(self: &Arc<Self>, addr: &str, testo: &[u8], ts: u64) -> Risultato<()> {
        let casella = self.casella.clone().ok_or("nessuna casella in questa rete")?;
        if testo.len() > TESTO_CASELLA_MAX {
            return Err(format!("messaggio troppo lungo per la casella (massimo {} KB)", TESTO_CASELLA_MAX / 1024).into());
        }
        let chiave = indirizzo::in_chiave(addr).map_err(|_| "indirizzo .nyct non valido")?;
        let mut carico = format!("{} {ts} ", nuovo_id()).into_bytes();
        carico.extend_from_slice(testo);
        let busta = casella::sigilla(&self.identita, &chiave, &carico)?;
        tokio::time::timeout(DEPOSITO_TEMPO, consegna_busta(self.socks, &casella, addr, &busta))
            .await
            .map_err(|_| "la casella non risponde")??;
        Ok(())
    }

    /// La sessione `sid` con `peer` è caduta: i testi spediti lì senza «ricevuto» (il contatto
    /// era sparito nel frattempo, es. modalità aereo) vanno nella casella.
    fn rimanda_in_casella(self: &Arc<Self>, peer: &str, sid: u64) {
        let persi: Vec<InVolo> = {
            let mut g = self.in_volo.lock().unwrap();
            let Some(v) = g.get_mut(peer) else { return };
            let (persi, restano): (Vec<InVolo>, Vec<InVolo>) = v.drain(..).partition(|m| m.sessione == sid);
            *v = restano;
            persi
        };
        if persi.is_empty() || self.casella.is_none() {
            return;
        }
        let stato = self.clone();
        let peer = peer.to_string();
        tokio::spawn(async move {
            for m in persi {
                if stato.sigilla_e_deposita(&peer, &m.testo, m.ts).await.is_ok() {
                    stato.segna(&peer, m.ts, "c");
                }
            }
        });
    }

    /// Nessun «ricevuto» entro `ATTESA_RICEVUTO`: se il contatto ha già mostrato di confermare
    /// (versione con le spunte), la sessione è morta senza saperlo → si chiude e i testi in volo
    /// vanno nella casella. Con chi non conferma (versione vecchia) non si fa nulla: sarebbero doppioni.
    async fn controlla_ricevuto(self: Arc<Self>, addr: String, sid: u64, ts: u64) {
        tokio::time::sleep(ATTESA_RICEVUTO).await;
        let ancora = self.in_volo.lock().unwrap().get(&addr).is_some_and(|v| v.iter().any(|m| m.sessione == sid && m.ts == ts));
        if !ancora || !self.conferma_di_solito(&addr) {
            return;
        }
        let era_lei = {
            let mut g = self.sessioni.lock().unwrap();
            let lei = g.get(&addr).is_some_and(|(n, _)| *n == sid);
            if lei {
                g.remove(&addr);                             // la prossima volta se ne apre una nuova
            }
            lei
        };
        if era_lei {
            self.notifica(format!("ST {addr} 0"));
            self.segna_visto(&addr);
        }
        self.rimanda_in_casella(&addr, sid);
    }

    /// Il contatto ha già confermato almeno un messaggio (ha le spunte).
    fn conferma_di_solito(&self, addr: &str) -> bool {
        std::fs::read_to_string(self.dati.join("stati").join(format!("{addr}.log")))
            .unwrap_or_default()
            .lines()
            .any(|l| l.ends_with(" 2") || l.ends_with(" 3"))
    }

    // --- spunte (spedito / consegnato / letto) ----------------------------------
    /// Stato di un nostro messaggio: «1» spedito, «c» nella casella, «2» consegnato, «3» letto
    /// (fino a quell'ora). Resta in `stati/<addr>.log` e arriva alle interfacce come
    /// `SP <addr> <ts> <stato>` (o `CAS <addr> <ts>` per la casella).
    fn segna(&self, addr: &str, ts: u64, stato: &str) {
        use std::io::Write;
        let dir = self.dati.join("stati");
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(format!("{addr}.log"))) {
            let _ = writeln!(f, "{ts} {stato}");
        }
        self.notifica(evento_spunta(addr, ts, stato));
    }

    /// Una conferma («K ts» / «R ts») al contatto: subito se c'è la sessione, altrimenti
    /// appena si riapre.
    fn conferma(&self, peer: &str, frame: Vec<u8>) {
        if let Some((_, s)) = self.sessioni.lock().unwrap().get(peer) {
            if s.try_send(frame.clone()).is_ok() {
                return;
            }
        }
        let mut g = self.pendenti.lock().unwrap();
        let v = g.entry(peer.to_string()).or_default();
        if v.len() < 500 {
            v.push(frame);
        }
    }

    fn leggi_stati(&self, addr: &str) -> Vec<String> {
        std::fs::read_to_string(self.dati.join("stati").join(format!("{}.log", con_suffisso(addr))))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| {
                let (ts, st) = l.split_once(' ')?;
                Some(evento_spunta(&con_suffisso(addr), ts.parse().ok()?, st.trim()))
            })
            .collect()
    }

    /// Chiede alla rubrica la posta lasciata per noi: all'avvio e poi ogni tanto (se siamo in
    /// linea, la rubrica la consegna anche da sola). Non usa il turno di `composizione`.
    async fn ritira_posta(self: Arc<Self>) {
        tokio::time::sleep(RITIRO_INIZIO).await;
        loop {
            if let Some(rub) = self.rubrica.clone() {
                let _ = self.invia_grezzo(&rub, b"CASELLA RITIRA").await;
            }
            tokio::time::sleep(RITIRO_GIRO).await;
        }
    }

    /// Messaggio di servizio (non va in cronologia né all'interfaccia).
    async fn invia_grezzo(self: &Arc<Self>, addr: &str, testo: &[u8]) -> Risultato<()> {
        let esistente = || self.sessioni.lock().unwrap().get(addr).map(|(_, s)| s.clone());
        if esistente().is_none() {
            tokio::time::timeout(SONDA_TEMPO, self.componi(addr)).await.map_err(|_| "tempo scaduto")??;
        }
        let scrittore = esistente().ok_or("sessione non creata")?;
        let mut chiaro = format!("M {} ", ora()).into_bytes();
        chiaro.extend_from_slice(testo);
        scrittore.send(chiaro).await.map_err(|_| "sessione chiusa")?;
        Ok(())
    }

    /// `POSTA <id> <busta hex> [<id> <busta hex> …]` dalla rubrica: si aprono le buste, i
    /// messaggi entrano in cronologia come arrivati dal loro mittente (autenticato dalla
    /// busta) e si conferma il ritiro, così la casella li cancella.
    fn posta_dalla_rubrica(&self, rubrica: &str, resto: &[u8]) {
        let Ok(resto) = std::str::from_utf8(resto) else { return };
        let mut parti = resto.split_ascii_whitespace();
        if parti.next() != Some("POSTA") {
            return;
        }
        let mut ritirati = Vec::new();
        let voci: Vec<&str> = parti.collect();
        for coppia in voci.chunks(2) {
            let [id, hex] = coppia else { break };
            if id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                continue;
            }
            ritirati.push(id.to_string());                   // anche le buste illeggibili: via dalla casella
            let Ok(busta) = da_hex(hex) else { continue };
            let Ok((chiave, carico)) = casella::apri(&self.identita, &busta) else { continue };
            let mittente = indirizzo::da_chiave(&chiave);
            let mut campi = carico.splitn(3, |&b| b == b' ');
            let (Some(mid), Some(ts), Some(testo)) = (campi.next(), campi.next(), campi.next()) else { continue };
            let mid = String::from_utf8_lossy(mid).to_string();
            if !self.posta_vista.lock().unwrap().insert(mid.clone()) {
                continue;                                   // già ricevuta (busta ripetuta)
            }
            self.registra_posta_vista(&mid);
            let ts = std::str::from_utf8(ts).ok().and_then(|t| t.parse::<u64>().ok()).unwrap_or_else(ora);
            self.conferma(&mittente, format!("K {ts}").into_bytes());
            // già arrivato direttamente (la sessione era caduta prima del «ricevuto»)
            let riga = format!("in {ts} {}", in_esadecimale(testo));
            if std::fs::read_to_string(self.file_cronologia(&mittente)).unwrap_or_default().lines().any(|l| l == riga) {
                continue;
            }
            self.registra(&mittente, "in", ts, testo);
            if !self.contatti.lock().unwrap().iter().any(|(a, _)| *a == mittente) {
                self.notifica(format!("NEW {mittente}"));
            }
            self.notifica(format!("MSG {mittente} {ts} {}", in_esadecimale(testo)));
        }
        if let (false, Some((_, s))) = (ritirati.is_empty(), self.sessioni.lock().unwrap().get(rubrica)) {
            let _ = s.try_send(format!("M {} CASELLA RICEVUTI {}", ora(), ritirati.join(" ")).into_bytes());
        }
    }

    fn registra_posta_vista(&self, id: &str) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(self.dati.join("casella-viste")) {
            let _ = writeln!(f, "{id}");
        }
    }

    // --- persistenza ---------------------------------------------------------
    fn registra_riga(&self, addr: &str, riga: &str) {
        use std::io::Write;
        let dir = self.dati.join("cronologia");
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(format!("{addr}.log"))) {
            let _ = writeln!(f, "{riga}");
        }
    }

    fn registra(&self, addr: &str, direzione: &str, ts: u64, testo: &[u8]) {
        use std::io::Write;
        let dir = self.dati.join("cronologia");
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join(format!("{addr}.log"))) {
            let _ = writeln!(f, "{direzione} {ts} {}", in_esadecimale(testo));
        }
    }

    fn leggi_cronologia(&self, addr: &str) -> Vec<String> {
        let percorso = self.dati.join("cronologia").join(format!("{}.log", con_suffisso(addr)));
        match std::fs::read_to_string(percorso) {
            Ok(testo) => testo.lines().map(|l| format!("P {l}\n")).collect(),
            Err(_) => Vec::new(),
        }
    }

    fn file_cronologia(&self, addr: &str) -> PathBuf {
        self.dati.join("cronologia").join(format!("{}.log", con_suffisso(addr)))
    }

    /// Ultima riga della cronologia di un contatto (per l'anteprima nell'elenco).
    fn ultima_riga(&self, addr: &str) -> Option<String> {
        let testo = std::fs::read_to_string(self.file_cronologia(addr)).ok()?;
        testo.lines().rev().find(|l| !l.trim().is_empty()).map(str::to_string)
    }

    /// Toglie dalla cronologia l'ultima occorrenza esatta di `riga`.
    fn togli_riga(&self, addr: &str, riga: &str) -> bool {
        let percorso = self.file_cronologia(addr);
        let Ok(testo) = std::fs::read_to_string(&percorso) else { return false };
        let mut righe: Vec<&str> = testo.lines().collect();
        let Some(i) = righe.iter().rposition(|l| *l == riga) else { return false };
        righe.remove(i);
        let mut nuovo = righe.join("\n");
        if !nuovo.is_empty() {
            nuovo.push('\n');
        }
        std::fs::write(&percorso, nuovo).is_ok()
    }

    fn rimuovi_contatto(&self, addr: &str) {
        use std::io::Write;
        let mut g = self.contatti.lock().unwrap();
        g.retain(|(a, _)| a != addr);
        if let Ok(mut f) = std::fs::File::create(self.dati.join("contatti")) {
            for (a, n) in g.iter() {
                let _ = writeln!(f, "{a} {n}");
            }
        }
    }

    fn aggiungi_contatto(&self, addr: &str, nick_hex: &str) {
        use std::io::Write;
        let mut g = self.contatti.lock().unwrap();
        if let Some(c) = g.iter_mut().find(|(a, _)| a == addr) {
            c.1 = nick_hex.to_string();
        } else {
            g.push((addr.to_string(), nick_hex.to_string()));
        }
        // riscriviamo l'elenco per intero (poche righe)
        if let Ok(mut f) = std::fs::File::create(self.dati.join("contatti")) {
            for (a, n) in g.iter() {
                let _ = writeln!(f, "{a} {n}");
            }
        }
    }

    // --- protocollo di controllo per l'interfaccia ---------------------------
    async fn servi_controllo(self: Arc<Self>, controllo: SocketAddr) -> Risultato<()> {
        let ascoltatore = TcpListener::bind(controllo).await?;
        loop {
            let (flusso, _) = ascoltatore.accept().await?;
            let st = self.clone();
            tokio::spawn(async move {
                let _ = st.gestisci_controllo(flusso).await;
            });
        }
    }

    async fn gestisci_controllo(self: Arc<Self>, flusso: TcpStream) -> Risultato<()> {
        let (lettura, mut scrittura) = flusso.into_split();
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        let scrittore = tokio::spawn(async move {
            while let Some(riga) = rx.recv().await {
                if scrittura.write_all(riga.as_bytes()).await.is_err() {
                    break;
                }
            }
        });

        let mut lettore = BufReader::new(lettura);
        let mut linea = String::new();
        // Con un segreto impostato la prima riga deve essere `AUTH <segreto>`:
        // su Android ogni app può aprire le porte di 127.0.0.1, e così solo chi
        // ha il segreto (la nostra interfaccia) può leggere e mandare messaggi.
        if let Some(atteso) = &self.segreto {
            if lettore.read_line(&mut linea).await? == 0 {
                scrittore.abort();
                return Ok(());
            }
            let dato = linea.trim_end().strip_prefix("AUTH ").unwrap_or("");
            if !uguali_a_tempo_costante(dato.as_bytes(), atteso.as_bytes()) {
                let _ = tx.send("ERR autenticazione richiesta\n".into());
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                scrittore.abort();
                return Ok(());
            }
            let _ = tx.send("AUTHOK\n".into());
        }
        loop {
            linea.clear();
            if lettore.read_line(&mut linea).await? == 0 {
                break;
            }
            self.comando(linea.trim_end(), &tx).await;
        }
        scrittore.abort();
        Ok(())
    }

    async fn comando(self: &Arc<Self>, riga: &str, tx: &mpsc::UnboundedSender<String>) {
        let mut it = riga.splitn(3, ' ');
        match it.next().unwrap_or("") {
            "WHOAMI" => {
                let _ = tx.send(format!("ME {}\n", self.indirizzo));
            }
            "LIST" => {
                let contatti = self.contatti.lock().unwrap().clone();
                for (a, n) in contatti {
                    let _ = tx.send(format!("C {a} {n}\n"));
                    if let Some(ultima) = self.ultima_riga(&a) {
                        let _ = tx.send(format!("L {a} {ultima}\n"));
                    }
                    if let Some(ts) = self.visti.lock().unwrap().get(&a) {
                        let _ = tx.send(format!("V {a} {ts}\n"));
                    }
                }
                let _ = tx.send("ENDC\n".into());
            }
            "SUB" => {
                self.iscritti.lock().unwrap().push(tx.clone());
                let online: Vec<String> = self.sessioni.lock().unwrap().keys().cloned().collect();
                for a in online {
                    let _ = tx.send(format!("ST {a} 1\n"));
                }
                // chiamate che squillano ancora (es. app aperta dalla notifica della chiamata)
                self.inviti.lock().unwrap().retain(|_, (_, t)| ora().saturating_sub(*t) < SQUILLO);
                for (a, (id, _)) in self.inviti.lock().unwrap().iter() {
                    let _ = tx.send(format!("CALLIN {a} {id}\n"));
                }
                let _ = tx.send("SUBOK\n".into());
            }
            "ADD" => {
                let addr = con_suffisso(it.next().unwrap_or(""));
                let nick = it.next().unwrap_or("");
                if indirizzo::in_chiave(&addr).is_err() {
                    let _ = tx.send("ERR indirizzo non valido\n".into());
                    return;
                }
                self.aggiungi_contatto(&addr, nick);
                let _ = tx.send("OK\n".into());
            }
            "HIST" => {
                let addr = it.next().unwrap_or("");
                for riga in self.leggi_cronologia(addr) {
                    let _ = tx.send(riga);
                }
                for riga in self.leggi_stati(addr) {
                    let _ = tx.send(format!("{riga}\n"));
                }
                let _ = tx.send("ENDH\n".into());
            }
            "LETTO" => {
                // l'utente ha visto la conversazione fino a <ts>: lo si dice al contatto («R»)
                let addr = con_suffisso(it.next().unwrap_or(""));
                let ts = it.next().and_then(|t| t.trim().parse::<u64>().ok()).unwrap_or(0);
                let nuovo = ts > 0 && indirizzo::in_chiave(&addr).is_ok() && {
                    let mut g = self.letti_inviati.lock().unwrap();
                    let prima = g.get(&addr).copied().unwrap_or(0);
                    if ts > prima {
                        g.insert(addr.clone(), ts);
                    }
                    ts > prima
                };
                if nuovo && self.rubrica.as_deref() != Some(addr.as_str()) {
                    self.conferma(&addr, format!("R {ts}").into_bytes());
                }
            }
            "SEND" => {
                let addr = it.next().unwrap_or("").to_string();
                let hex = it.next().unwrap_or("");
                match da_hex(hex) {
                    Ok(testo) => {
                        let st = self.clone();
                        let txc = tx.clone();
                        tokio::spawn(async move {
                            match st.invia_a(&addr, &testo).await {
                                Ok(()) => {
                                    let _ = txc.send("OK\n".into());
                                }
                                Err(e) => {
                                    let _ = txc.send(format!("ERR {e}\n"));
                                }
                            }
                        });
                    }
                    Err(_) => {
                        let _ = tx.send("ERR testo non valido\n".into());
                    }
                }
            }
            "CLEAR" => {
                // CLEAR <addr>: cancella la cronologia (gli allegati ricevuti restano su disco)
                let addr = con_suffisso(it.next().unwrap_or(""));
                let _ = std::fs::remove_file(self.file_cronologia(&addr));
                let _ = tx.send("OK\n".into());
            }
            "DEL" => {
                // DEL <addr>: elimina il contatto e la sua cronologia
                let addr = con_suffisso(it.next().unwrap_or(""));
                self.rimuovi_contatto(&addr);
                let _ = std::fs::remove_file(self.file_cronologia(&addr));
                let _ = tx.send("OK\n".into());
            }
            "CALL" => {
                // CALL <addr>: invito a una chiamata (apre la sessione se serve) → CALLID <addr> <id>
                let addr = con_suffisso(it.next().unwrap_or(""));
                let (st, txc) = (self.clone(), tx.clone());
                tokio::spawn(async move {
                    let id = nuovo_id();
                    match st.sessione(&addr).await {
                        Ok(s) if s.send(format!("CI {id}").into_bytes()).await.is_ok() => {
                            let _ = txc.send(format!("CALLID {addr} {id}\n"));
                        }
                        Ok(_) => {
                            let _ = txc.send("ERR sessione chiusa\n".into());
                        }
                        Err(e) => {
                            let _ = txc.send(format!("ERR {e}\n"));
                        }
                    }
                });
            }
            quale @ ("CALLOK" | "CALLNO" | "CALLEND") => {
                // risposta all'invito o fine della chiamata: CALLOK|CALLNO|CALLEND <addr> <id>
                let addr = con_suffisso(it.next().unwrap_or(""));
                let id = it.next().unwrap_or("").trim().to_string();
                let prefisso = match quale { "CALLOK" => "CA", "CALLNO" => "CN", _ => "CF" };
                if self.inviti.lock().unwrap().get(&addr).is_some_and(|(i, _)| *i == id) {
                    self.inviti.lock().unwrap().remove(&addr);
                }
                if !id_chiamata_valido(&id) {
                    let _ = tx.send("ERR chiamata non valida\n".into());
                } else {
                    let (st, txc, frame) = (self.clone(), tx.clone(), format!("{prefisso} {id}").into_bytes());
                    tokio::spawn(async move {
                        match st.sessione(&addr).await {
                            Ok(s) if s.send(frame).await.is_ok() => {
                                let _ = txc.send("OK\n".into());
                            }
                            _ => {
                                let _ = txc.send("ERR contatto non raggiungibile\n".into());
                            }
                        }
                    });
                }
            }
            "AUD" => {
                // AUD <addr> <seq> <opus hex>: un pacchetto audio. Niente risposta (sono 50 al
                // secondo); se la coda della sessione è piena il pacchetto si scarta: in una
                // chiamata un pacchetto in ritardo non serve più.
                let addr = con_suffisso(it.next().unwrap_or(""));
                let resto = it.next().unwrap_or("");
                if let Some((seq, hex)) = resto.trim().split_once(' ') {
                    if let (Ok(seq), Ok(dati)) = (seq.parse::<u32>(), da_hex(hex)) {
                        if !dati.is_empty() && dati.len() <= AUDIO_MAX {
                            if let Some((_, s)) = self.sessioni.lock().unwrap().get(&addr) {
                                let mut frame = b"CX ".to_vec();
                                frame.extend_from_slice(&seq.to_be_bytes());
                                frame.extend_from_slice(&dati);
                                let _ = s.try_send(frame);
                            }
                        }
                    }
                }
            }
            "DELMSG" => {
                // DELMSG <addr> <riga hex>: toglie quel messaggio dalla cronologia (solo da noi)
                let addr = con_suffisso(it.next().unwrap_or(""));
                let riga = da_hex(it.next().unwrap_or("")).map(|b| String::from_utf8_lossy(&b).to_string());
                match riga {
                    Ok(r) if self.togli_riga(&addr, &r) => {
                        let _ = tx.send("OK\n".into());
                    }
                    _ => {
                        let _ = tx.send("ERR messaggio non trovato\n".into());
                    }
                }
            }
            "SENDFILE" => {
                // SENDFILE <addr> <percorso hex> [mime]
                let addr = it.next().unwrap_or("").to_string();
                let resto = it.next().unwrap_or("");
                let (hex, mime) = resto.split_once(' ').unwrap_or((resto, ""));
                let percorso = match da_hex(hex) {
                    Ok(b) => PathBuf::from(String::from_utf8_lossy(&b).to_string()),
                    Err(_) => {
                        let _ = tx.send("ERR percorso non valido\n".into());
                        return;
                    }
                };
                let mime = mime.to_string();
                let st = self.clone();
                let txc = tx.clone();
                tokio::spawn(async move {
                    match st.invia_file_a(&addr, &percorso, &mime).await {
                        Ok(()) => {
                            let _ = txc.send("OK\n".into());
                        }
                        Err(e) => {
                            let msg = in_esadecimale(e.to_string().as_bytes());
                            st.notifica(format!("FERR {} - {msg}", con_suffisso(&addr)));
                            let _ = txc.send(format!("ERR {e}\n"));
                        }
                    }
                });
            }
            _ => {
                let _ = tx.send("ERR comando sconosciuto\n".into());
            }
        }
    }
}

// --- utilità -----------------------------------------------------------------

fn ora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Garantisce il suffisso `.nyct` su un indirizzo (gli utenti lo scrivono già
/// completo, ma restiamo tolleranti).
fn con_suffisso(addr: &str) -> String {
    let a = addr.trim();
    if a.to_ascii_lowercase().ends_with(indirizzo::SUFFISSO) {
        a.to_string()
    } else {
        format!("{a}{}", indirizzo::SUFFISSO)
    }
}

fn carica_identita(percorso: &Path) -> Risultato<Identita> {
    if percorso.exists() {
        return Ok(Identita::da_byte(&std::fs::read(percorso)?)?);
    }
    let id = Identita::genera()?;
    if let Some(p) = percorso.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(percorso, id.in_byte())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(percorso, std::fs::Permissions::from_mode(0o600));
    }
    Ok(id)
}

/// Id delle buste già ricevute (gli ultimi 5000 bastano: la casella le tiene al più 14 giorni).
fn carica_posta_vista(dati: &Path) -> HashSet<String> {
    let testo = std::fs::read_to_string(dati.join("casella-viste")).unwrap_or_default();
    let righe: Vec<&str> = testo.lines().collect();
    righe[righe.len().saturating_sub(5000)..].iter().map(|s| s.to_string()).collect()
}

/// Deposito anonimo: POST della busta all'indirizzo .nyct della casella, attraverso i circuiti.
async fn consegna_busta(socks: SocketAddr, casella: &str, per: &str, busta: &[u8]) -> Risultato<()> {
    let mut flusso = socks5_connetti(socks, casella, 80).await?;
    let testa = format!(
        "POST /deposita?a={per} HTTP/1.0\r\nHost: casella\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\n\r\n",
        busta.len()
    );
    flusso.write_all(testa.as_bytes()).await?;
    flusso.write_all(busta).await?;
    flusso.flush().await?;
    let mut risposta = Vec::new();
    (&mut flusso).take(4096).read_to_end(&mut risposta).await?;
    let prima = String::from_utf8_lossy(&risposta);
    let prima = prima.lines().next().unwrap_or("");
    if prima.split_whitespace().nth(1) == Some("200") {
        Ok(())
    } else {
        let corpo = String::from_utf8_lossy(&risposta);
        let motivo = corpo.split("\r\n\r\n").nth(1).unwrap_or(prima).trim().to_string();
        Err(format!("casella: {motivo}").into())
    }
}

/// Riga d'evento di una spunta: `SP <addr> <ts> 1|2|3` oppure `CAS <addr> <ts>`.
fn evento_spunta(addr: &str, ts: u64, stato: &str) -> String {
    if stato == "c" {
        format!("CAS {addr} {ts}")
    } else {
        format!("SP {addr} {ts} {stato}")
    }
}

fn carica_visti(dati: &Path) -> HashMap<String, u64> {
    std::fs::read_to_string(dati.join("visti"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let (a, t) = l.split_once(' ')?;
            Some((a.to_string(), t.trim().parse().ok()?))
        })
        .collect()
}

fn carica_contatti(dati: &Path) -> Vec<(String, String)> {
    match std::fs::read_to_string(dati.join("contatti")) {
        Ok(testo) => testo
            .lines()
            .filter_map(|l| {
                let mut it = l.splitn(2, ' ');
                let a = it.next()?.trim();
                if a.is_empty() {
                    return None;
                }
                Some((a.to_string(), it.next().unwrap_or("").trim().to_string()))
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Nome di file ricevuto reso innocuo: niente cartelle, niente nascosti,
/// niente caratteri di controllo, lunghezza limitata.
fn nome_sicuro(nome: &str) -> String {
    let base = nome.rsplit(['/', '\\']).next().unwrap_or("");
    let mut pulito: String = base
        .chars()
        .map(|c| if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') { '_' } else { c })
        .collect();
    pulito = pulito.trim().trim_start_matches('.').to_string();
    if pulito.chars().count() > 120 {
        let (stem, ext) = match pulito.rfind('.') {
            Some(i) if pulito.len() - i <= 10 => (pulito[..i].to_string(), pulito[i..].to_string()),
            _ => (pulito.clone(), String::new()),
        };
        pulito = stem.chars().take(110).collect::<String>() + &ext;
    }
    if pulito.is_empty() { "file".to_string() } else { pulito }
}

/// Tipo MIME accettato solo se è un token semplice (es. `image/jpeg`).
fn mime_sicuro(mime: &str) -> String {
    let m = mime.trim().to_ascii_lowercase();
    let ok = !m.is_empty()
        && m.len() <= 80
        && m.contains('/')
        && m.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '+' | '-' | '_'));
    if ok { m } else { "application/octet-stream".to_string() }
}

/// Confronto che non rivela, coi tempi, quanti caratteri iniziali coincidono.
fn uguali_a_tempo_costante(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn id_valido(id: &str) -> bool {
    (1..=32).contains(&id.len()) && id.chars().all(|c| c.is_ascii_hexdigit())
}

fn nuovo_id() -> String {
    let mut b = [0u8; 8];
    getrandom::fill(&mut b).expect("generatore casuale di sistema");
    in_esadecimale(&b)
}

/// Sottocartella degli allegati per contatto (prime 16 lettere dell'indirizzo).
fn cartella_contatto(peer: &str) -> String {
    peer.chars().filter(|c| c.is_ascii_alphanumeric()).take(16).collect()
}

/// `cartella/nome`, oppure `nome (2).ext`, `nome (3).ext`… se esiste già.
fn percorso_libero(cartella: &Path, nome: &str) -> PathBuf {
    let primo = cartella.join(nome);
    if !primo.exists() {
        return primo;
    }
    let (stem, ext) = match nome.rfind('.') {
        Some(i) if i > 0 => (&nome[..i], &nome[i..]),
        _ => (nome, ""),
    };
    (2..10_000)
        .map(|n| cartella.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .unwrap_or(primo)
}

fn da_hex(testo: &str) -> Risultato<Vec<u8>> {
    if testo.len() % 2 != 0 || !testo.is_ascii() {
        return Err("stringa esadecimale non valida".into());
    }
    (0..testo.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&testo[i..i + 2], 16).map_err(|_| "carattere non esadecimale".into()))
        .collect()
}

/// Client SOCKS5 minimo: CONNECT verso un dominio (il nostro proxy riconosce i
/// `.nyct` e li instrada col punto d'incontro).
async fn socks5_connetti(proxy: SocketAddr, host: &str, porta: u16) -> Risultato<TcpStream> {
    let mut s = TcpStream::connect(proxy).await?;
    s.write_all(&[0x05, 0x01, 0x00]).await?; // VER, 1 metodo, "senza autenticazione"
    let mut saluto = [0u8; 2];
    s.read_exact(&mut saluto).await?;
    if saluto != [0x05, 0x00] {
        return Err("SOCKS: metodo di autenticazione non accettato".into());
    }
    let hb = host.as_bytes();
    if hb.len() > 255 {
        return Err("SOCKS: nome host troppo lungo".into());
    }
    let mut richiesta = vec![0x05, 0x01, 0x00, 0x03, hb.len() as u8];
    richiesta.extend_from_slice(hb);
    richiesta.extend_from_slice(&porta.to_be_bytes());
    s.write_all(&richiesta).await?;

    let mut testa = [0u8; 4]; // VER, REP, RSV, ATYP
    s.read_exact(&mut testa).await?;
    if testa[1] != 0x00 {
        return Err("SOCKS: connessione rifiutata (servizio irraggiungibile?)".into());
    }
    let resto = match testa[3] {
        0x01 => 4,
        0x04 => 16,
        0x03 => {
            let mut l = [0u8; 1];
            s.read_exact(&mut l).await?;
            l[0] as usize
        }
        _ => return Err("SOCKS: tipo d'indirizzo sconosciuto".into()),
    };
    let mut scarto = vec![0u8; resto + 2]; // indirizzo + porta
    s.read_exact(&mut scarto).await?;
    Ok(s)
}

/// Frame con prefisso di lunghezza a 2 byte (big-endian).
async fn scrivi_frame<W: AsyncWriteExt + Unpin>(w: &mut W, dati: &[u8]) -> Risultato<()> {
    if dati.len() > u16::MAX as usize {
        return Err("frame troppo grande".into());
    }
    w.write_all(&(dati.len() as u16).to_be_bytes()).await?;
    w.write_all(dati).await?;
    w.flush().await?;
    Ok(())
}

async fn leggi_frame<R: AsyncReadExt + Unpin>(r: &mut R) -> Risultato<Option<Vec<u8>>> {
    let mut len = [0u8; 2];
    match r.read_exact(&mut len).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let n = u16::from_be_bytes(len) as usize;
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf).await?;
    Ok(Some(buf))
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn hex_andata_e_ritorno() {
        let dati = b"ciao \xf0\x9f\x91\x8b mondo"; // con emoji
        let h = in_esadecimale(dati);
        assert_eq!(da_hex(&h).unwrap(), dati);
    }

    #[test]
    fn suffisso_idempotente() {
        let a = "abc";
        assert_eq!(con_suffisso(a), "abc.nyct");
        assert_eq!(con_suffisso("abc.nyct"), "abc.nyct");
    }

    #[tokio::test]
    async fn frame_andata_e_ritorno() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        let atteso = b"un frame qualunque".to_vec();
        let atteso2 = atteso.clone();
        let h = tokio::spawn(async move { scrivi_frame(&mut a, &atteso2).await.unwrap() });
        let ricevuto = leggi_frame(&mut b).await.unwrap().unwrap();
        h.await.unwrap();
        assert_eq!(ricevuto, atteso);
    }

    #[test]
    fn nomi_di_file_resi_innocui() {
        assert_eq!(nome_sicuro("../../etc/passwd"), "passwd");
        assert_eq!(nome_sicuro("C:\\Windows\\evil.exe"), "evil.exe");
        assert_eq!(nome_sicuro(".bashrc"), "bashrc");
        assert_eq!(nome_sicuro("foto vacanze.jpg"), "foto vacanze.jpg");
        assert_eq!(nome_sicuro("a\nb\u{0}c.txt"), "a_b_c.txt");
        assert_eq!(nome_sicuro(""), "file");
        assert_eq!(nome_sicuro("..."), "file");
        let lungo = format!("{}.pdf", "x".repeat(300));
        let r = nome_sicuro(&lungo);
        assert!(r.ends_with(".pdf") && r.chars().count() <= 120);
    }

    #[test]
    fn mime_e_id_validati() {
        assert_eq!(mime_sicuro("image/JPEG"), "image/jpeg");
        assert_eq!(mime_sicuro("text/html; charset=x"), "application/octet-stream");
        assert_eq!(mime_sicuro(""), "application/octet-stream");
        assert!(id_valido("0123abcd") && !id_valido("../x") && !id_valido(""));
        assert_eq!(nuovo_id().len(), 16);
    }

    #[test]
    fn percorso_libero_non_sovrascrive() {
        let d = std::env::temp_dir().join(format!("nyct-prova-{}", nuovo_id()));
        std::fs::create_dir_all(&d).unwrap();
        assert_eq!(percorso_libero(&d, "a.txt"), d.join("a.txt"));
        std::fs::write(d.join("a.txt"), b"1").unwrap();
        assert_eq!(percorso_libero(&d, "a.txt"), d.join("a (2).txt"));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn confronto_segreto() {
        assert!(uguali_a_tempo_costante(b"abc123", b"abc123"));
        assert!(!uguali_a_tempo_costante(b"abc124", b"abc123"));
        assert!(!uguali_a_tempo_costante(b"abc", b"abc123"));
    }

    #[test]
    fn togli_riga_rimuove_solo_quel_messaggio() {
        let d = std::env::temp_dir().join(format!("nyct-cron-{}", nuovo_id()));
        let st = Stato {
            identita: Identita::genera().unwrap(),
            indirizzo: String::new(),
            socks: "127.0.0.1:1".parse().unwrap(),
            dati: d.clone(),
            contatti: Mutex::new(Vec::new()),
            sessioni: Mutex::new(HashMap::new()),
            composizione: tokio::sync::Mutex::new(()),
            prossima_sessione: AtomicU64::new(1),
            iscritti: Mutex::new(Vec::new()),
            in_arrivo: Mutex::new(HashMap::new()),
            max_file: 1,
            segreto: None,
            visti: Mutex::new(HashMap::new()),
            rubrica: None,
            casella: None,
            irraggiungibili: Mutex::new(HashSet::new()),
            posta_vista: Mutex::new(HashSet::new()),
            in_volo: Mutex::new(HashMap::new()),
            pendenti: Mutex::new(HashMap::new()),
            letti_inviati: Mutex::new(HashMap::new()),
            battito_noto: Mutex::new(HashSet::new()),
            inviti: Mutex::new(HashMap::new()),
        };
        st.registra_riga("x.nyct", "in 1 6161");
        st.registra_riga("x.nyct", "out 2 6262");
        st.registra_riga("x.nyct", "in 3 6363");
        assert!(st.togli_riga("x.nyct", "out 2 6262"));
        assert!(!st.togli_riga("x.nyct", "out 2 6262"));
        assert_eq!(std::fs::read_to_string(st.file_cronologia("x.nyct")).unwrap(), "in 1 6161\nin 3 6363\n");
        assert_eq!(st.ultima_riga("x.nyct").as_deref(), Some("in 3 6363"));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[tokio::test]
    async fn stretta_interna_autentica_il_mittente() {
        // Il servizio sfida l'indirizzo dichiarato; solo il vero proprietario
        // della chiave privata conclude la stretta e ottiene gli stessi segreti.
        let contatto = Identita::genera().unwrap();
        let (mittente, msg1) = Mittente::inizia(&contatto.pubblica()).unwrap();
        let (msg2, seg_contatto) = stretta::rispondi(&contatto, &msg1).unwrap();
        let seg_servizio = mittente.completa(&msg2).unwrap();
        assert_eq!(seg_servizio.verso_destinatario, seg_contatto.verso_destinatario);
        assert_eq!(seg_servizio.verso_mittente, seg_contatto.verso_mittente);

        // Un impostore che dichiara l'indirizzo altrui non ha la chiave privata:
        // non riesce nemmeno a leggere la sfida (cifrata verso la chiave vera) e,
        // se anche ci arrivasse, la sua risposta non concluderebbe la stretta.
        let impostore = Identita::genera().unwrap();
        let (mittente2, msg1b) = Mittente::inizia(&contatto.pubblica()).unwrap();
        match stretta::rispondi(&impostore, &msg1b) {
            Err(_) => {} // non può leggere la sfida: respinto subito
            Ok((msg2b, _)) => assert!(mittente2.completa(&msg2b).is_err()),
        }
    }
}
