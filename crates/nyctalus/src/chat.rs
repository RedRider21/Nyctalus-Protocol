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
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use nyctalus_core::stretta::{self, Identita, Mittente, SegretiSessione};
use nyctalus_core::indirizzo;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::incontro::Record;
use crate::{in_esadecimale, Risultato};

/// Avvia il motore della chat: ascolta i contatti in entrata (backend), serve
/// l'interfaccia grafica sulla porta di controllo e compone le connessioni in
/// uscita tramite il proxy SOCKS.
pub async fn chat(
    identita_path: PathBuf,
    backend: SocketAddr,
    controllo: SocketAddr,
    socks: SocketAddr,
    dati: PathBuf,
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
        iscritti: Mutex::new(Vec::new()),
    });

    println!("Nyctalus chat");
    println!("Il tuo Contact ID:  {indirizzo}");
    println!("Entrata (backend):  {backend}   ← punta qui il `site --backend`");
    println!("Controllo (UI):     {controllo}");
    println!("Uscita (SOCKS):     {socks}");
    println!("Dati:               {}", dati.display());

    let ascoltatore = TcpListener::bind(backend).await?;
    let st_ctrl = stato.clone();
    tokio::spawn(async move {
        if let Err(e) = st_ctrl.servi_controllo(controllo).await {
            eprintln!("controllo: {e}");
        }
    });

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
    sessioni: Mutex<HashMap<String, mpsc::UnboundedSender<Vec<u8>>>>, // indirizzo → scrittore
    iscritti: Mutex<Vec<mpsc::UnboundedSender<String>>>,     // UI in ascolto degli eventi
}

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

        let (ch_tx, mut ch_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        self.sessioni.lock().unwrap().insert(peer.clone(), ch_tx);
        self.notifica(format!("ST {peer} 1"));

        // scrittore: sigilla ogni messaggio in chiaro e lo spedisce
        let scrittore = tokio::spawn(async move {
            while let Some(chiaro) = ch_rx.recv().await {
                let frame = tx.sigilla(&chiaro);
                if scrivi_frame(&mut scrittura, &frame).await.is_err() {
                    break;
                }
            }
        });

        // lettore: apre i messaggi e li consegna
        let stato = self.clone();
        tokio::spawn(async move {
            loop {
                match leggi_frame(&mut lettura).await {
                    Ok(Some(frame)) => match rx.apri(&frame) {
                        Ok(chiaro) => stato.ricevi_messaggio(&peer, &chiaro),
                        Err(_) => break, // frame corrotto: chiudiamo
                    },
                    _ => break, // EOF o errore
                }
            }
            scrittore.abort();
            stato.sessioni.lock().unwrap().remove(&peer);
            stato.notifica(format!("ST {peer} 0"));
        });
    }

    /// Un messaggio applicativo è `M <ts> <testo>` (testo UTF-8, anche con spazi).
    fn ricevi_messaggio(&self, peer: &str, chiaro: &[u8]) {
        let Some(resto) = chiaro.strip_prefix(b"M ") else { return };
        let Some(sp) = resto.iter().position(|&b| b == b' ') else { return };
        let ts = std::str::from_utf8(&resto[..sp]).ok().and_then(|t| t.parse::<u64>().ok()).unwrap_or_else(ora);
        let testo = &resto[sp + 1..];
        self.registra(peer, "in", ts, testo);
        if !self.contatti.lock().unwrap().iter().any(|(a, _)| a == peer) {
            self.notifica(format!("NEW {peer}"));
        }
        self.notifica(format!("MSG {peer} {ts} {}", in_esadecimale(testo)));
    }

    /// Invia un messaggio a un contatto, aprendo la sessione se serve.
    async fn invia_a(self: &Arc<Self>, indirizzo_contatto: &str, testo: &[u8]) -> Risultato<()> {
        let addr = con_suffisso(indirizzo_contatto);
        let ts = ora();
        let scrittore = {
            let g = self.sessioni.lock().unwrap();
            g.get(&addr).cloned()
        };
        let scrittore = match scrittore {
            Some(s) => s,
            None => {
                self.componi(&addr).await?;
                self.sessioni.lock().unwrap().get(&addr).cloned().ok_or("sessione non creata")?
            }
        };
        let mut chiaro = format!("M {ts} ").into_bytes();
        chiaro.extend_from_slice(testo);
        scrittore.send(chiaro).map_err(|_| "sessione chiusa")?;
        self.registra(&addr, "out", ts, testo);
        self.notifica(format!("OUT {addr} {ts} {}", in_esadecimale(testo)));
        Ok(())
    }

    // --- persistenza ---------------------------------------------------------
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
                }
                let _ = tx.send("ENDC\n".into());
            }
            "SUB" => {
                self.iscritti.lock().unwrap().push(tx.clone());
                let online: Vec<String> = self.sessioni.lock().unwrap().keys().cloned().collect();
                for a in online {
                    let _ = tx.send(format!("ST {a} 1\n"));
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
                let _ = tx.send("ENDH\n".into());
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
