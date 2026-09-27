//! Derivazione delle chiavi di un flusso.
//!
//! Dal segreto condiviso tra mittente e destinatario (che in futuro arriverà
//! dalla stretta di mano Noise) si ricavano chiavi indipendenti per ogni uso
//! e per ogni flusso: cambiando il "contesto" BLAKE3 cambia la chiave, quindi
//! la chiave delle etichette non rivela nulla su quella di cifratura.

/// Contesto BLAKE3 per le etichette dei frammenti.
pub(crate) const CONTESTO_ETICHETTE: &str = "Nyctalus v0 2026-09-27 etichette dei frammenti";
/// Contesto BLAKE3 per la cifratura dei frammenti.
pub(crate) const CONTESTO_CIFRATURA: &str = "Nyctalus v0 2026-09-27 cifratura dei frammenti";

pub(crate) fn deriva(contesto: &str, segreto_condiviso: &[u8; 32], id_flusso: u64) -> [u8; 32] {
    let mut materiale = [0u8; 40];
    materiale[..32].copy_from_slice(segreto_condiviso);
    materiale[32..].copy_from_slice(&id_flusso.to_le_bytes());
    let chiave = blake3::derive_key(contesto, &materiale);
    azzera(&mut materiale);
    chiave
}

/// Azzera un buffer in modo che il compilatore non possa eliminare la
/// scrittura: le chiavi non devono restare in RAM dopo l'uso.
pub(crate) fn azzera(buffer: &mut [u8]) {
    for byte in buffer.iter_mut() {
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn contesti_diversi_chiavi_diverse() {
        let segreto = [1; 32];
        assert_ne!(deriva(CONTESTO_ETICHETTE, &segreto, 0), deriva(CONTESTO_CIFRATURA, &segreto, 0));
        assert_ne!(deriva(CONTESTO_CIFRATURA, &segreto, 0), deriva(CONTESTO_CIFRATURA, &segreto, 1));
    }
}
