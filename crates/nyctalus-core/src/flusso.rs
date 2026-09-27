//! Un flusso di dati spezzato in frammenti cifrati e ricomposto a destinazione.
//!
//! Il mittente taglia i dati in frammenti, dà a ciascuno la sua etichetta
//! segreta e lo cifra: sulla rete escono solo pacchetti tutti della stessa
//! lunghezza, indistinguibili da byte casuali. Il destinatario riconosce
//! l'etichetta, verifica e decifra, poi ricompone.
//!
//! Il percorso dei pacchetti nella rete (ingresso fisso, poi ragnatela) e la
//! cifratura a strati dei nodi arriveranno nei livelli superiori: qui c'è la
//! parte end-to-end tra i due estremi di un flusso (che va in una sola
//! direzione: una conversazione usa due flussi con id diversi).

use std::fmt;

use crate::cifratura::{CifrarioFlusso, ErroreCifratura};
use crate::etichette::{Etichetta, GeneratoreEtichette, LUNGHEZZA_ETICHETTA, RiconoscitoreEtichette};
use crate::ricomposizione::{ErroreRicomposizione, Ricompositore};

/// Parametri condivisi da mittente e destinatario.
#[derive(Debug, Clone, Copy)]
pub struct ParametriFlusso {
    /// Byte utili per frammento.
    pub dimensione_frammento: usize,
    /// Quanti frammenti possono essere "in volo" oltre quello atteso.
    pub finestra: u64,
}

impl Default for ParametriFlusso {
    fn default() -> Self {
        // 1173 byte utili = pacchetto da 1200 byte, la dimensione che QUIC
        // garantisce su qualunque rete senza frammentazione IP.
        Self { dimensione_frammento: 1173, finestra: 512 }
    }
}

pub struct MittenteFlusso {
    etichette: GeneratoreEtichette,
    cifrario: CifrarioFlusso,
    parametri: ParametriFlusso,
    prossimo: u64,
}

impl MittenteFlusso {
    pub fn nuovo(segreto: &[u8; 32], id_flusso: u64, parametri: ParametriFlusso) -> Self {
        Self {
            etichette: GeneratoreEtichette::nuovo(segreto, id_flusso),
            cifrario: CifrarioFlusso::nuovo(segreto, id_flusso, parametri.dimensione_frammento),
            parametri,
            prossimo: 0,
        }
    }

    /// Spezza e cifra `dati`, restituendo i pacchetti da spedire. Se `fine` è
    /// vero l'ultimo pacchetto chiude il flusso (anche con `dati` vuoti).
    pub fn spezza(&mut self, dati: &[u8], fine: bool) -> Vec<Vec<u8>> {
        let mut pezzi: Vec<&[u8]> = dati.chunks(self.parametri.dimensione_frammento).collect();
        if pezzi.is_empty() && fine {
            pezzi.push(&[]);
        }
        let quanti = pezzi.len();
        pezzi
            .into_iter()
            .enumerate()
            .map(|(n, pezzo)| {
                let indice = self.prossimo;
                self.prossimo += 1;
                let etichetta = self.etichette.etichetta(indice);
                self.cifrario.sigilla(indice, &etichetta, pezzo, fine && n + 1 == quanti)
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroreRicezione {
    /// Etichetta sconosciuta: pacchetto di un altro flusso, replay o rumore.
    EtichettaSconosciuta,
    Cifratura(ErroreCifratura),
    Ricomposizione(ErroreRicomposizione),
}

impl fmt::Display for ErroreRicezione {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EtichettaSconosciuta => f.write_str("etichetta sconosciuta"),
            Self::Cifratura(e) => e.fmt(f),
            Self::Ricomposizione(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ErroreRicezione {}

pub struct RicevitoreFlusso {
    riconoscitore: RiconoscitoreEtichette,
    cifrario: CifrarioFlusso,
    ricompositore: Ricompositore,
}

impl RicevitoreFlusso {
    pub fn nuovo(segreto: &[u8; 32], id_flusso: u64, parametri: ParametriFlusso) -> Self {
        let max_byte = parametri.dimensione_frammento * parametri.finestra as usize;
        Self {
            riconoscitore: RiconoscitoreEtichette::nuovo(
                GeneratoreEtichette::nuovo(segreto, id_flusso),
                parametri.finestra,
            ),
            cifrario: CifrarioFlusso::nuovo(segreto, id_flusso, parametri.dimensione_frammento),
            ricompositore: Ricompositore::nuovo(parametri.finestra, max_byte),
        }
    }

    /// Riceve un pacchetto dalla rete e restituisce i dati diventati
    /// consegnabili (possono essere vuoti se mancano frammenti precedenti).
    pub fn ricevi(&mut self, pacchetto: &[u8]) -> Result<Vec<u8>, ErroreRicezione> {
        let etichetta: Etichetta = pacchetto
            .get(..LUNGHEZZA_ETICHETTA)
            .and_then(|e| e.try_into().ok())
            .ok_or(ErroreRicezione::Cifratura(ErroreCifratura::LunghezzaErrata))?;
        let indice = self
            .riconoscitore
            .cerca(&etichetta)
            .ok_or(ErroreRicezione::EtichettaSconosciuta)?;
        let (dati, ultimo) =
            self.cifrario.apri(indice, pacchetto).map_err(ErroreRicezione::Cifratura)?;
        // Solo ora il pacchetto è sicuramente autentico.
        self.riconoscitore.consuma(&etichetta);
        self.ricompositore
            .inserisci(indice, dati, ultimo)
            .map_err(ErroreRicezione::Ricomposizione)?;
        let pronti = self.ricompositore.estrai_pronti();
        self.riconoscitore.avanza(self.ricompositore.prossimo_atteso());
        Ok(pronti)
    }

    pub fn completo(&self) -> bool {
        self.ricompositore.completo()
    }

    /// Indice del prossimo frammento atteso: tutti i precedenti sono stati
    /// consegnati. È il valore da confermare al mittente per il controllo
    /// del flusso (il mittente non va oltre `prossimo_atteso + finestra`).
    pub fn prossimo_atteso(&self) -> u64 {
        self.ricompositore.prossimo_atteso()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::cifratura::lunghezza_pacchetto;

    const SEGRETO: [u8; 32] = [42; 32];

    /// Generatore pseudo-casuale deterministico (xorshift) per i test,
    /// così non serve una dipendenza esterna.
    struct Xorshift(u64);
    impl Xorshift {
        fn prossimo(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
    }

    /// Mescola i pacchetti a blocchi: simula la ragnatela, dove i frammenti
    /// arrivano in disordine ma entro la finestra consentita.
    fn mescola_a_blocchi(pacchetti: &mut [Vec<u8>], blocco: usize, rng: &mut Xorshift) {
        for parte in pacchetti.chunks_mut(blocco) {
            for i in (1..parte.len()).rev() {
                let j = (rng.prossimo() % (i as u64 + 1)) as usize;
                parte.swap(i, j);
            }
        }
    }

    #[test]
    fn diecimila_frammenti_cifrati_in_disordine() {
        let parametri = ParametriFlusso { dimensione_frammento: 100, finestra: 512 };
        let originale: Vec<u8> = (0..1_000_000u32).map(|n| (n * 31 % 251) as u8).collect();

        let mut mittente = MittenteFlusso::nuovo(&SEGRETO, 9, parametri);
        let mut pacchetti = mittente.spezza(&originale, true);
        assert_eq!(pacchetti.len(), 10_000);
        assert!(pacchetti.iter().all(|p| p.len() == lunghezza_pacchetto(100)));
        mescola_a_blocchi(&mut pacchetti, 256, &mut Xorshift(0x9E37_79B9_7F4A_7C15));

        let mut ricevitore = RicevitoreFlusso::nuovo(&SEGRETO, 9, parametri);
        let mut ricomposto = Vec::with_capacity(originale.len());
        for pacchetto in &pacchetti {
            ricomposto.extend(ricevitore.ricevi(pacchetto).unwrap());
        }
        assert!(ricevitore.completo());
        assert_eq!(ricomposto, originale);
    }

    #[test]
    fn le_etichette_non_si_ripetono() {
        let mut mittente = MittenteFlusso::nuovo(&SEGRETO, 1, ParametriFlusso::default());
        let pacchetti = mittente.spezza(&vec![0; 1173 * 1000], true);
        let uniche: std::collections::HashSet<_> =
            pacchetti.iter().map(|p| p[..LUNGHEZZA_ETICHETTA].to_vec()).collect();
        assert_eq!(uniche.len(), pacchetti.len());
    }

    #[test]
    fn pacchetto_predefinito_da_1200_byte() {
        let mut mittente = MittenteFlusso::nuovo(&SEGRETO, 1, ParametriFlusso::default());
        assert_eq!(mittente.spezza(b"x", true)[0].len(), 1200);
    }

    #[test]
    fn rifiuta_replay_e_pacchetti_di_altri_flussi() {
        let parametri = ParametriFlusso::default();
        let mut mittente = MittenteFlusso::nuovo(&SEGRETO, 1, parametri);
        let mut estraneo = MittenteFlusso::nuovo(&SEGRETO, 2, parametri);
        let mut ricevitore = RicevitoreFlusso::nuovo(&SEGRETO, 1, parametri);

        let pacchetto = mittente.spezza(b"ciao", true).remove(0);
        assert_eq!(ricevitore.ricevi(&pacchetto).unwrap(), b"ciao");
        assert_eq!(ricevitore.ricevi(&pacchetto), Err(ErroreRicezione::EtichettaSconosciuta));

        let intruso = estraneo.spezza(b"intruso", true).remove(0);
        assert_eq!(ricevitore.ricevi(&intruso), Err(ErroreRicezione::EtichettaSconosciuta));
    }

    /// Un nodo vede passare un'etichetta e prova a "bruciarla" mandando per
    /// primo un pacchetto falso con la stessa etichetta: il falso viene
    /// rifiutato e il vero, arrivato dopo, viene accettato lo stesso.
    #[test]
    fn un_pacchetto_falso_non_brucia_l_etichetta() {
        let parametri = ParametriFlusso::default();
        let mut mittente = MittenteFlusso::nuovo(&SEGRETO, 1, parametri);
        let mut ricevitore = RicevitoreFlusso::nuovo(&SEGRETO, 1, parametri);

        let vero = mittente.spezza(b"messaggio vero", true).remove(0);
        let mut falso = vero.clone();
        for byte in &mut falso[LUNGHEZZA_ETICHETTA..] {
            *byte = 0xAB;
        }
        assert_eq!(
            ricevitore.ricevi(&falso),
            Err(ErroreRicezione::Cifratura(ErroreCifratura::AutenticazioneFallita))
        );
        assert_eq!(ricevitore.ricevi(&vero).unwrap(), b"messaggio vero");
        assert!(ricevitore.completo());
    }
}
