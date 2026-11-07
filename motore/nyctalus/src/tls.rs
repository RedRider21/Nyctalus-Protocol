// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Configurazione TLS/QUIC del collegamento tra due estremi (livello L0).
//!
//! Il ricevitore usa un certificato autofirmato generato all'avvio. Il
//! mittente non si fida di nessuna autorità: accetta solo il certificato la
//! cui impronta BLAKE3 gli è stata comunicata ("pinning"), come faranno i
//! nodi della rete con le chiavi pubblicate nell'elenco dei nodi.

use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::WebPkiSupportedAlgorithms;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};

use crate::Risultato;


/// Nome da usare collegandosi a `indirizzo`: il suo IP. Così il primo pacchetto QUIC **non porta
/// nessun nome** (con un IP il client TLS non manda l'SNI). Prima c'era `nyctalus.invalid`, che
/// chiunque poteva leggere nel primo pacchetto (le sue chiavi sono pubbliche per standard) se il
/// camuffamento era spento: verificato in Shadow con `simulazione/verifica_paradigmi.py`.
pub fn nome_per(indirizzo: &std::net::SocketAddr) -> String {
    // forma HTTP/3: il nome scelto dal gestore, come un normale sito (se non c'è, nessun nome)
    crate::trasporto::nome_mimetico().unwrap_or_else(|| indirizzo.ip().to_string())
}

/// ALPN della forma HTTP/3.
const ALPN_H3: &[u8] = b"h3";

/// Certificato nuovo con dati normali: il predefinito della libreria si chiamava «rcgen self signed
/// cert» e valeva dal 1975 al 4096, riconoscibile da chiunque si colleghi (un censore che sonda).
fn certificato_nuovo() -> Risultato<rcgen::CertifiedKey<rcgen::KeyPair>> {
    let nome = crate::trasporto::nome_mimetico().unwrap_or_else(|| "localhost".to_string());
    let mut p = rcgen::CertificateParams::new(vec![nome.clone()])?;
    p.distinguished_name = rcgen::DistinguishedName::new();
    p.distinguished_name.push(rcgen::DnType::CommonName, nome);
    let oggi = std::time::SystemTime::now();
    p.not_before = (oggi - std::time::Duration::from_secs(86_400)).into();
    p.not_after = (oggi + std::time::Duration::from_secs(10 * 365 * 86_400)).into();
    let chiave = rcgen::KeyPair::generate()?;
    let cert = p.self_signed(&chiave)?;
    Ok(rcgen::CertifiedKey { cert, signing_key: chiave })
}

pub struct IdentitaServer {
    pub config: quinn::ServerConfig,
    pub impronta: [u8; 32],
}

pub fn identita_server() -> Risultato<IdentitaServer> {
    identita_server_in(None)
}

/// Come `identita_server`, ma se c'è una cartella di stato il certificato vi viene
/// salvato la prima volta e riusato dopo: così l'impronta (e quindi la riga del nodo
/// nell'elenco firmato) resta la stessa a ogni riavvio.
pub fn identita_server_in(cartella: Option<&std::path::Path>) -> Risultato<IdentitaServer> {
    let salvati = cartella.and_then(|c| Some((std::fs::read(c.join("tls-cert.der")).ok()?, std::fs::read(c.join("tls-chiave.der")).ok()?)));
    let (der, chiave_der) = match salvati {
        Some((cert, chiave)) => (CertificateDer::from(cert), chiave),
        None => {
            let certificato = certificato_nuovo()?;
            let der = certificato.cert.der().clone();
            let chiave = certificato.signing_key.serialize_der();
            if let Some(c) = cartella {
                std::fs::create_dir_all(c)?;
                std::fs::write(c.join("tls-cert.der"), der.as_ref())?;
                scrivi_privato(&c.join("tls-chiave.der"), &chiave)?;
            }
            (der, chiave)
        }
    };
    let impronta = *blake3::hash(&der).as_bytes();
    let chiave = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(chiave_der));
    if crate::trasporto::mimetico() && der.windows(22).any(|w| w == b"rcgen self signed cert") {
        eprintln!("WARNING: HTTP/3 form with an old, recognisable certificate: create a new node identity \
                   (new --data folder, or installa.sh --nuova-identita) and send the new line to the network manager");
    }
    let mut tls = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .with_no_client_auth()
        .with_single_cert(vec![der], chiave)?;
    tls.max_early_data_size = u32::MAX;
    if crate::trasporto::mimetico() {
        tls.alpn_protocols = vec![ALPN_H3.to_vec()];
    }
    let mut config = quinn::ServerConfig::with_crypto(Arc::new(quinn::crypto::rustls::QuicServerConfig::try_from(tls)?));
    config.transport_config(trasporto_client());   // stessi tempi: chi sparisce si scopre presto
    Ok(IdentitaServer { config, impronta })
}

/// Scrive un file leggibile solo dal proprietario (chiavi private).
pub fn scrivi_privato(percorso: &std::path::Path, dati: &[u8]) -> Risultato<()> {
    use std::io::Write;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    o.open(percorso)?.write_all(dati)?;
    Ok(())
}

pub fn config_client(impronta_attesa: [u8; 32]) -> Risultato<quinn::ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verificatore = Arc::new(VerificaImpronta {
        impronta_attesa,
        algoritmi: provider.signature_verification_algorithms,
    });
    let tls = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(verificatore)
        .with_no_client_auth();
    let mut tls = tls;
    if crate::trasporto::mimetico() {
        tls.alpn_protocols = vec![ALPN_H3.to_vec()];
    }
    let quic = quinn::crypto::rustls::QuicClientConfig::try_from(tls)?;
    let mut config = quinn::ClientConfig::new(Arc::new(quic));
    config.transport_config(trasporto_client());
    Ok(config)
}

/// Transport config dei collegamenti client: tiene vivi i circuiti inattivi
/// (es. un servizio `.nyct` registrato in attesa di un visitatore) con un ping
/// periodico, così non vengono chiusi dal timeout di inattività di QUIC.
fn trasporto_client() -> Arc<quinn::TransportConfig> {
    let mut t = quinn::TransportConfig::default();
    // Segnale di vita ogni 5 s; 15 s di silenzio = l'altro capo non c'è più. Un
    // servizio spento di colpo (senza salutare) si scopre così in pochi secondi, e la
    // caduta si propaga lungo il circuito fino al punto d'incontro.
    t.keep_alive_interval(Some(std::time::Duration::from_secs(5)));
    t.max_idle_timeout(Some(std::time::Duration::from_secs(15).try_into().expect("15s valido")));
    // Controllo di congestione: di serie **BBR** (dalla 0.3.8). Cubic, quello di serie di quinn,
    // scambia ogni pacchetto perso per congestione e dimezza la velocità: con l'1% di perdite
    // (Wi-Fi, rete mobile) eravamo a 1,3 Mbit/s contro i 2,3 di Tor; con BBR 2,2-2,4, e senza
    // perdite ~10 contro 6,4 (simulazione/velocita.py, Shadow). È una scelta locale di ogni
    // programma: nessun cambio di protocollo. `NYCTALUS_CONGESTIONE=cubic|newreno` per tornare indietro.
    match std::env::var("NYCTALUS_CONGESTIONE").as_deref() {
        Ok("cubic") => {
            t.congestion_controller_factory(Arc::new(quinn::congestion::CubicConfig::default()));
        }
        Ok("newreno") => {
            t.congestion_controller_factory(Arc::new(quinn::congestion::NewRenoConfig::default()));
        }
        _ => {
            t.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
        }
    }
    Arc::new(t)
}

/// Config client che accetta QUALSIASI certificato: usata SOLO per i
/// collegamenti fra nodi del prototipo, dove i nodi non conoscono in anticipo
/// l'impronta dei vicini. L'anonimato viene dagli strati a cipolla, non da
/// questo TLS; l'autenticazione fra nodi è una cosa da fare (vedi STATO.md).
pub fn config_client_insicuro() -> Risultato<quinn::ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verificatore = Arc::new(AccettaTutto { algoritmi: provider.signature_verification_algorithms });
    let tls = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])?
        .dangerous()
        .with_custom_certificate_verifier(verificatore)
        .with_no_client_auth();
    let mut tls = tls;
    if crate::trasporto::mimetico() {
        tls.alpn_protocols = vec![ALPN_H3.to_vec()];
    }
    let quic = quinn::crypto::rustls::QuicClientConfig::try_from(tls)?;
    let mut config = quinn::ClientConfig::new(Arc::new(quic));
    config.transport_config(trasporto_client());
    Ok(config)
}

#[derive(Debug)]
struct AccettaTutto {
    algoritmi: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for AccettaTutto {
    fn verify_server_cert(&self, _: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }
    fn verify_tls12_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(m, c, d, &self.algoritmi)
    }
    fn verify_tls13_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(m, c, d, &self.algoritmi)
    }
    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algoritmi.supported_schemes()
    }
}

#[derive(Debug)]
struct VerificaImpronta {
    impronta_attesa: [u8; 32],
    algoritmi: WebPkiSupportedAlgorithms,
}

impl ServerCertVerifier for VerificaImpronta {
    fn verify_server_cert(
        &self,
        certificato: &CertificateDer<'_>,
        _intermedi: &[CertificateDer<'_>],
        _nome: &ServerName<'_>,
        _ocsp: &[u8],
        _ora: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if blake3::hash(certificato).as_bytes() == &self.impronta_attesa {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("impronta del certificato diversa da quella attesa".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        messaggio: &[u8],
        certificato: &CertificateDer<'_>,
        firma: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(messaggio, certificato, firma, &self.algoritmi)
    }

    fn verify_tls13_signature(
        &self,
        messaggio: &[u8],
        certificato: &CertificateDer<'_>,
        firma: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(messaggio, certificato, firma, &self.algoritmi)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algoritmi.supported_schemes()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn impronta_stabile_con_la_cartella_di_stato() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let c = std::env::temp_dir().join(format!("nyct-tls-{}", std::process::id()));
        let a = identita_server_in(Some(&c)).unwrap().impronta;
        let b = identita_server_in(Some(&c)).unwrap().impronta;
        assert_eq!(a, b, "riavviato con la stessa cartella deve avere la stessa impronta");
        assert_ne!(a, identita_server_in(None).unwrap().impronta);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(c.join("tls-chiave.der")).unwrap().permissions().mode();
            assert_eq!(modo & 0o077, 0, "la chiave privata non deve essere leggibile da altri");
        }
        let _ = std::fs::remove_dir_all(&c);
    }
}
