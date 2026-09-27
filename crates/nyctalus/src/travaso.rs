// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.

//! Travaso bidirezionale di byte tra due canali (una connessione TCP e uno
//! stream QUIC), usato dal proxy e dal nodo di uscita.

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

/// Copia `da_a` → `verso_a` e `da_b` → `verso_b` in parallelo, finché uno dei
/// due lati non si chiude; poi chiude in scrittura anche l'altro. Gli errori
/// (connessione interrotta a metà) non sono fatali: si limita a terminare.
pub async fn collega<RA, WA, RB, WB>(mut da_a: RA, mut verso_a: WA, mut da_b: RB, mut verso_b: WB)
where
    RA: AsyncRead + Unpin,
    WA: AsyncWrite + Unpin,
    RB: AsyncRead + Unpin,
    WB: AsyncWrite + Unpin,
{
    let avanti = async {
        let _ = tokio::io::copy(&mut da_a, &mut verso_a).await;
        let _ = verso_a.shutdown().await;
    };
    let indietro = async {
        let _ = tokio::io::copy(&mut da_b, &mut verso_b).await;
        let _ = verso_b.shutdown().await;
    };
    tokio::join!(avanti, indietro);
}
