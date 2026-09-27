//! Nucleo del protocollo NyxShift.
//!
//! Contiene la logica "pura" della rete, senza I/O né socket, così da poterla
//! verificare con i test prima di collegarla al trasporto QUIC:
//!
//! - [`etichette`]: etichette segrete rotanti per riconoscere i frammenti
//!   (VISIONE.md §5.4) senza mai mettere indirizzi IP nei pacchetti;
//! - [`ricomposizione`]: rimette in ordine i frammenti arrivati in disordine
//!   dalla ragnatela, con limiti di memoria contro gli attacchi DoS;
//! - [`flusso`]: unisce le due cose, lato mittente (spezza) e lato
//!   destinatario (riconosce e ricompone).

pub mod etichette;
pub mod flusso;
pub mod ricomposizione;
