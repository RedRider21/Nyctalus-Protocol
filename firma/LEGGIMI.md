# Firma delle release di Nyctalus

Dalla 0.3.20 ogni release contiene **`SHA256SUMS`** (l'impronta di ogni file) e **`SHA256SUMS.sig`**
(la firma di quell'elenco con la chiave della release, strumento standard `ssh-keygen -Y`).

Chiave pubblica della release (`nyctalus-release.pub`):

    ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIIH2tSuN3+IVKV6CPiXCbkdT5FCyjciRePBoxVokIiiY release@nyctalus

## Verificare a mano (Linux, macOS, Termux con `pkg install openssh`, Windows con OpenSSH)

Nella cartella con i file scaricati, più `SHA256SUMS`, `SHA256SUMS.sig` e `allowed_signers` (questa cartella):

    ssh-keygen -Y verify -f allowed_signers -I release@nyctalus -n nyctalus-release -s SHA256SUMS.sig < SHA256SUMS
    sha256sum -c --ignore-missing SHA256SUMS

Oppure, dai sorgenti: `strumenti/verifica-release.sh CARTELLA`.

- Lo **script unico di Termux** fa questa verifica da solo e non installa nulla se la firma o l'impronta non tornano.
- L'**APK** è firmato anche dalla chiave Android: il telefono accetta un aggiornamento solo se è firmato con la stessa chiave.
- La chiave privata non è mai su GitHub.
