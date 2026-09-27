// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Nucleo di Nyctalus Protocol.
//!
//! Contiene la logica "pura" della rete, senza I/O né socket, così da poterla
//! verificare con i test prima di collegarla al trasporto QUIC:
//!
//! - [`etichette`]: etichette segrete rotanti per riconoscere i frammenti
//!   (VISIONE.md §5.4) senza mai mettere indirizzi IP nei pacchetti;
//! - [`cipolla`]: strati simmetrici del percorso (livello a cipolla L1);
//! - [`circuito`]: apertura del circuito con Sphinx (chiavi di salto per-nodo);
//! - [`cifratura`]: cifratura end-to-end dei frammenti (ChaCha20-Poly1305),
//!   con pacchetti tutti della stessa lunghezza;
//! - [`ricomposizione`]: rimette in ordine i frammenti arrivati in disordine
//!   dalla ragnatela, con limiti di memoria contro gli attacchi DoS;
//! - [`flusso`]: unisce tutto, lato mittente (spezza e cifra) e lato
//!   destinatario (riconosce, verifica, decifra e ricompone);
//! - [`stretta`]: stretta di mano Noise NK che produce i segreti dei flussi;
//! - [`socks5`]: protocollo SOCKS5, con cui i programmi entrano nella rete;
//! - [`indirizzo`]: indirizzi `.nyct` dei servizi interni (la chiave pubblica
//!   in base32 con controllo anti-errori di battitura).

mod chiavi;
pub mod cifratura;
pub mod cipolla;
pub mod circuito;
pub mod etichette;
pub mod flusso;
pub mod indirizzo;
pub mod ricomposizione;
pub mod socks5;
pub mod stretta;
