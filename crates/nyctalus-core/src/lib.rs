//! Nucleo di Nyctalus Protocol.
//!
//! Contiene la logica "pura" della rete, senza I/O né socket, così da poterla
//! verificare con i test prima di collegarla al trasporto QUIC:
//!
//! - [`etichette`]: etichette segrete rotanti per riconoscere i frammenti
//!   (VISIONE.md §5.4) senza mai mettere indirizzi IP nei pacchetti;
//! - [`cifratura`]: cifratura end-to-end dei frammenti (ChaCha20-Poly1305),
//!   con pacchetti tutti della stessa lunghezza;
//! - [`ricomposizione`]: rimette in ordine i frammenti arrivati in disordine
//!   dalla ragnatela, con limiti di memoria contro gli attacchi DoS;
//! - [`flusso`]: unisce tutto, lato mittente (spezza e cifra) e lato
//!   destinatario (riconosce, verifica, decifra e ricompone).

mod chiavi;
pub mod cifratura;
pub mod etichette;
pub mod flusso;
pub mod ricomposizione;
