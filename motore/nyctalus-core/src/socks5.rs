// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Protocollo SOCKS5 (RFC 1928), logica pura senza I/O.
//!
//! È l'interfaccia con cui i programmi dell'utente (browser, curl, git…)
//! entrano nella rete: il demone Nyctalus apre un proxy SOCKS5 locale, come
//! fa `tor`. Qui c'è solo l'analisi e la costruzione dei messaggi del
//! protocollo; l'apertura dei socket e l'instradamento vivono nel demone.
//!
//! Sequenza di una connessione SOCKS5:
//! 1. **Saluto:** il client elenca i metodi di autenticazione che conosce; il
//!    proxy ne sceglie uno. Nyctalus accetta solo "nessuna autenticazione"
//!    (`0x00`): il proxy è locale, sulla macchina dell'utente.
//! 2. **Richiesta:** il client chiede `CONNECT indirizzo:porta`. L'indirizzo
//!    può essere IPv4, IPv6 o un nome di dominio; se finisce in `.nyct` è un
//!    servizio interno alla rete (SPECIFICA §6), altrimenti esce su Internet.
//! 3. **Risposta:** il proxy comunica se la connessione è stata aperta.
//!
//! Le funzioni di analisi restituiscono [`ErroreSocks::Incompleto`] quando i
//! byte ricevuti non bastano ancora: il chiamante asincrono aspetta e riprova.

use std::fmt;

pub const VERSIONE: u8 = 0x05;
pub const METODO_SENZA_AUTENTICAZIONE: u8 = 0x00;
pub const METODO_NESSUNO_ACCETTABILE: u8 = 0xFF;
const CMD_CONNECT: u8 = 0x01;
const ATYP_IPV4: u8 = 0x01;
const ATYP_NOME: u8 = 0x03;
const ATYP_IPV6: u8 = 0x04;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroreSocks {
    /// Servono altri byte per completare l'analisi (non è un errore fatale).
    Incompleto,
    VersioneErrata,
    ComandoNonSupportato,
    TipoIndirizzoNonValido,
    NomeNonValido,
}

impl fmt::Display for ErroreSocks {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let testo = match self {
            Self::Incompleto => "messaggio SOCKS5 incompleto",
            Self::VersioneErrata => "versione SOCKS diversa da 5",
            Self::ComandoNonSupportato => "comando SOCKS non supportato (solo CONNECT)",
            Self::TipoIndirizzoNonValido => "tipo di indirizzo SOCKS non valido",
            Self::NomeNonValido => "nome di dominio non valido",
        };
        f.write_str(testo)
    }
}

impl std::error::Error for ErroreSocks {}

/// Destinazione richiesta dal client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destinazione {
    Ipv4([u8; 4]),
    Ipv6([u8; 16]),
    Nome(String),
}

impl Destinazione {
    /// Vero se è un servizio interno `.nyct`.
    pub fn e_interna(&self) -> bool {
        matches!(self, Self::Nome(n) if n.to_ascii_lowercase().ends_with(".nyct"))
    }
}

/// Esito del saluto: la lista dei metodi proposti dal client e quanti byte
/// sono stati consumati.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Saluto {
    pub metodi: Vec<u8>,
    pub byte_consumati: usize,
}

impl Saluto {
    /// Analizza il saluto iniziale: `[VER=5, NMETHODS, metodi...]`.
    pub fn analizza(byte: &[u8]) -> Result<Self, ErroreSocks> {
        let &[versione, n_metodi, ..] = byte else {
            return Err(ErroreSocks::Incompleto);
        };
        if versione != VERSIONE {
            return Err(ErroreSocks::VersioneErrata);
        }
        let totale = 2 + n_metodi as usize;
        if byte.len() < totale {
            return Err(ErroreSocks::Incompleto);
        }
        Ok(Self { metodi: byte[2..totale].to_vec(), byte_consumati: totale })
    }

    pub fn offre_senza_autenticazione(&self) -> bool {
        self.metodi.contains(&METODO_SENZA_AUTENTICAZIONE)
    }
}

/// Risposta al saluto: `[VER=5, metodo scelto]`.
pub fn risposta_saluto(metodo: u8) -> [u8; 2] {
    [VERSIONE, metodo]
}

/// Richiesta CONNECT del client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Richiesta {
    pub destinazione: Destinazione,
    pub porta: u16,
    pub byte_consumati: usize,
}

impl Richiesta {
    /// Analizza `[VER, CMD, RSV, ATYP, indirizzo, PORTA(2)]`.
    pub fn analizza(byte: &[u8]) -> Result<Self, ErroreSocks> {
        let &[versione, comando, _riservato, tipo, ..] = byte else {
            return Err(ErroreSocks::Incompleto);
        };
        if versione != VERSIONE {
            return Err(ErroreSocks::VersioneErrata);
        }
        if comando != CMD_CONNECT {
            return Err(ErroreSocks::ComandoNonSupportato);
        }
        let (destinazione, dopo_indirizzo) = match tipo {
            ATYP_IPV4 => estrai_ip(byte, 4, |b| Destinazione::Ipv4(b.try_into().unwrap()))?,
            ATYP_IPV6 => estrai_ip(byte, 16, |b| Destinazione::Ipv6(b.try_into().unwrap()))?,
            ATYP_NOME => {
                let &lunghezza = byte.get(4).ok_or(ErroreSocks::Incompleto)?;
                let inizio = 5;
                let fine = inizio + lunghezza as usize;
                let grezzo = byte.get(inizio..fine).ok_or(ErroreSocks::Incompleto)?;
                let nome = std::str::from_utf8(grezzo)
                    .map_err(|_| ErroreSocks::NomeNonValido)?
                    .to_string();
                (Destinazione::Nome(nome), fine)
            }
            _ => return Err(ErroreSocks::TipoIndirizzoNonValido),
        };
        let porta_byte = byte.get(dopo_indirizzo..dopo_indirizzo + 2).ok_or(ErroreSocks::Incompleto)?;
        let porta = u16::from_be_bytes([porta_byte[0], porta_byte[1]]);
        Ok(Self { destinazione, porta, byte_consumati: dopo_indirizzo + 2 })
    }
}

fn estrai_ip(
    byte: &[u8],
    lunghezza: usize,
    costruisci: impl Fn(&[u8]) -> Destinazione,
) -> Result<(Destinazione, usize), ErroreSocks> {
    let fine = 4 + lunghezza;
    let grezzo = byte.get(4..fine).ok_or(ErroreSocks::Incompleto)?;
    Ok((costruisci(grezzo), fine))
}

/// Codici di risposta alla richiesta (RFC 1928).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Esito {
    Riuscito = 0x00,
    ErroreGenerico = 0x01,
    NonRaggiungibile = 0x04,
    ConnessioneRifiutata = 0x05,
    ComandoNonSupportato = 0x07,
}

/// Risposta alla richiesta. L'indirizzo di binding non è significativo per un
/// proxy anonimo: si mette `0.0.0.0:0`, come è prassi.
pub fn risposta_richiesta(esito: Esito) -> Vec<u8> {
    vec![VERSIONE, esito as u8, 0x00, ATYP_IPV4, 0, 0, 0, 0, 0, 0]
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn saluto_normale() {
        let s = Saluto::analizza(&[5, 2, 0x00, 0x02]).unwrap();
        assert_eq!(s.metodi, vec![0x00, 0x02]);
        assert_eq!(s.byte_consumati, 4);
        assert!(s.offre_senza_autenticazione());
        assert_eq!(risposta_saluto(METODO_SENZA_AUTENTICAZIONE), [5, 0]);
    }

    #[test]
    fn saluto_incompleto_e_versione_errata() {
        assert_eq!(Saluto::analizza(&[5]), Err(ErroreSocks::Incompleto));
        assert_eq!(Saluto::analizza(&[5, 3, 0, 1]), Err(ErroreSocks::Incompleto));
        assert_eq!(Saluto::analizza(&[4, 1, 0]), Err(ErroreSocks::VersioneErrata));
    }

    #[test]
    fn connect_verso_dominio_nyct() {
        let nome = b"abc.nyct";
        let mut m = vec![5, CMD_CONNECT, 0, ATYP_NOME, nome.len() as u8];
        m.extend_from_slice(nome);
        m.extend_from_slice(&443u16.to_be_bytes());
        let r = Richiesta::analizza(&m).unwrap();
        assert_eq!(r.destinazione, Destinazione::Nome("abc.nyct".into()));
        assert!(r.destinazione.e_interna());
        assert_eq!(r.porta, 443);
        assert_eq!(r.byte_consumati, m.len());
    }

    #[test]
    fn connect_verso_ipv4_esterno() {
        let m = [5, CMD_CONNECT, 0, ATYP_IPV4, 93, 184, 216, 34, 0x00, 0x50];
        let r = Richiesta::analizza(&m).unwrap();
        assert_eq!(r.destinazione, Destinazione::Ipv4([93, 184, 216, 34]));
        assert!(!r.destinazione.e_interna());
        assert_eq!(r.porta, 80);
    }

    #[test]
    fn connect_verso_ipv6() {
        let mut m = vec![5, CMD_CONNECT, 0, ATYP_IPV6];
        m.extend_from_slice(&[0x20; 16]);
        m.extend_from_slice(&443u16.to_be_bytes());
        let r = Richiesta::analizza(&m).unwrap();
        assert_eq!(r.destinazione, Destinazione::Ipv6([0x20; 16]));
    }

    #[test]
    fn rifiuta_comando_diverso_da_connect() {
        let m = [5, 0x02, 0, ATYP_IPV4, 1, 2, 3, 4, 0, 80];
        assert_eq!(Richiesta::analizza(&m), Err(ErroreSocks::ComandoNonSupportato));
    }

    #[test]
    fn richiesta_incompleta() {
        let m = [5, CMD_CONNECT, 0, ATYP_IPV4, 1, 2]; // manca resto IP + porta
        assert_eq!(Richiesta::analizza(&m), Err(ErroreSocks::Incompleto));
    }

    #[test]
    fn risposta_ha_forma_attesa() {
        assert_eq!(risposta_richiesta(Esito::Riuscito), vec![5, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
        assert_eq!(risposta_richiesta(Esito::ConnessioneRifiutata)[1], 5);
    }
}
