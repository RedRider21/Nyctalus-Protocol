//! Un flusso di dati spezzato in frammenti e ricomposto a destinazione.
//!
//! Il mittente taglia i dati in frammenti di dimensione fissa, ciascuno con
//! la sua etichetta segreta; il destinatario riconosce le etichette e
//! ricompone. Il percorso dei frammenti nella rete (ingresso fisso, poi
//! ragnatela) e la cifratura a strati arriveranno nei livelli superiori:
//! qui c'è solo la logica di un flusso tra due estremi.
//!
//! NOTA: il campo `ultimo` dovrà viaggiare **dentro** la parte cifrata del
//! frammento, altrimenti un nodo intermedio potrebbe vedere dove finisce un
//! flusso (e quanto è lungo).

use crate::etichette::{Etichetta, GeneratoreEtichette, RiconoscitoreEtichette};
use crate::ricomposizione::{ErroreRicomposizione, Ricompositore};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frammento {
    pub etichetta: Etichetta,
    pub dati: Vec<u8>,
    pub ultimo: bool,
}

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
        // 1200 byte stanno comodi in un pacchetto UDP senza frammentazione IP.
        Self { dimensione_frammento: 1200, finestra: 512 }
    }
}

pub struct MittenteFlusso {
    generatore: GeneratoreEtichette,
    parametri: ParametriFlusso,
    prossimo: u64,
}

impl MittenteFlusso {
    pub fn nuovo(segreto: &[u8; 32], id_flusso: u64, parametri: ParametriFlusso) -> Self {
        Self { generatore: GeneratoreEtichette::nuovo(segreto, id_flusso), parametri, prossimo: 0 }
    }

    /// Spezza `dati` in frammenti. Se `fine` è vero l'ultimo frammento
    /// chiude il flusso (anche con `dati` vuoti).
    pub fn spezza(&mut self, dati: &[u8], fine: bool) -> Vec<Frammento> {
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
                Frammento {
                    etichetta: self.generatore.etichetta(indice),
                    dati: pezzo.to_vec(),
                    ultimo: fine && n + 1 == quanti,
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroreRicezione {
    /// Etichetta sconosciuta: frammento di un altro flusso, replay o rumore.
    EtichettaSconosciuta,
    Ricomposizione(ErroreRicomposizione),
}

pub struct RicevitoreFlusso {
    riconoscitore: RiconoscitoreEtichette,
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
            ricompositore: Ricompositore::nuovo(parametri.finestra, max_byte),
        }
    }

    /// Riceve un frammento e restituisce i dati diventati consegnabili.
    pub fn ricevi(&mut self, frammento: Frammento) -> Result<Vec<u8>, ErroreRicezione> {
        let indice = self
            .riconoscitore
            .riconosci(&frammento.etichetta)
            .ok_or(ErroreRicezione::EtichettaSconosciuta)?;
        self.ricompositore
            .inserisci(indice, frammento.dati, frammento.ultimo)
            .map_err(ErroreRicezione::Ricomposizione)?;
        let pronti = self.ricompositore.estrai_pronti();
        self.riconoscitore.avanza(self.ricompositore.prossimo_atteso());
        Ok(pronti)
    }

    pub fn completo(&self) -> bool {
        self.ricompositore.completo()
    }
}

#[cfg(test)]
mod test {
    use super::*;

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

    /// Mescola i frammenti a blocchi: simula la ragnatela, dove i frammenti
    /// arrivano in disordine ma entro la finestra consentita.
    fn mescola_a_blocchi(frammenti: &mut [Frammento], blocco: usize, rng: &mut Xorshift) {
        for parte in frammenti.chunks_mut(blocco) {
            for i in (1..parte.len()).rev() {
                let j = (rng.prossimo() % (i as u64 + 1)) as usize;
                parte.swap(i, j);
            }
        }
    }

    #[test]
    fn diecimila_frammenti_in_disordine() {
        let parametri = ParametriFlusso { dimensione_frammento: 100, finestra: 512 };
        let originale: Vec<u8> = (0..1_000_000u32).map(|n| (n * 31 % 251) as u8).collect();

        let mut mittente = MittenteFlusso::nuovo(&SEGRETO, 9, parametri);
        let mut frammenti = mittente.spezza(&originale, true);
        assert_eq!(frammenti.len(), 10_000);
        mescola_a_blocchi(&mut frammenti, 256, &mut Xorshift(0x9E37_79B9_7F4A_7C15));

        let mut ricevitore = RicevitoreFlusso::nuovo(&SEGRETO, 9, parametri);
        let mut ricomposto = Vec::with_capacity(originale.len());
        for frammento in frammenti {
            ricomposto.extend(ricevitore.ricevi(frammento).unwrap());
        }
        assert!(ricevitore.completo());
        assert_eq!(ricomposto, originale);
    }

    #[test]
    fn le_etichette_non_si_ripetono() {
        let mut mittente = MittenteFlusso::nuovo(&SEGRETO, 1, ParametriFlusso::default());
        let frammenti = mittente.spezza(&vec![0; 1200 * 1000], true);
        let uniche: std::collections::HashSet<_> =
            frammenti.iter().map(|f| f.etichetta).collect();
        assert_eq!(uniche.len(), frammenti.len());
    }

    #[test]
    fn rifiuta_replay_e_frammenti_di_altri_flussi() {
        let parametri = ParametriFlusso::default();
        let mut mittente = MittenteFlusso::nuovo(&SEGRETO, 1, parametri);
        let mut estraneo = MittenteFlusso::nuovo(&SEGRETO, 2, parametri);
        let mut ricevitore = RicevitoreFlusso::nuovo(&SEGRETO, 1, parametri);

        let frammento = mittente.spezza(b"ciao", true).remove(0);
        assert_eq!(ricevitore.ricevi(frammento.clone()).unwrap(), b"ciao");
        assert_eq!(ricevitore.ricevi(frammento), Err(ErroreRicezione::EtichettaSconosciuta));

        let intruso = estraneo.spezza(b"intruso", true).remove(0);
        assert_eq!(ricevitore.ricevi(intruso), Err(ErroreRicezione::EtichettaSconosciuta));
    }
}
