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

/// Nome del server nel certificato (non viene verificato: conta l'impronta).
pub const NOME_SERVER: &str = "nyctalus.invalid";

pub struct IdentitaServer {
    pub config: quinn::ServerConfig,
    pub impronta: [u8; 32],
}

pub fn identita_server() -> Risultato<IdentitaServer> {
    let certificato = rcgen::generate_simple_self_signed(vec![NOME_SERVER.to_string()])?;
    let der = certificato.cert.der().clone();
    let impronta = *blake3::hash(&der).as_bytes();
    let chiave = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(certificato.signing_key.serialize_der()));
    let config = quinn::ServerConfig::with_single_cert(vec![der], chiave)?;
    Ok(IdentitaServer { config, impronta })
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
    let quic = quinn::crypto::rustls::QuicClientConfig::try_from(tls)?;
    Ok(quinn::ClientConfig::new(Arc::new(quic)))
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
