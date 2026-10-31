<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Manuale Windows (anteprima)

Su Windows Nyctalus funziona **come l'app del telefono**: una finestra con Chat, Rubrica,
Browser e Info. Dentro c'è lo stesso motore (`nyctalus.exe`), e **Python è incluso**: non serve
installare nulla. Serve Windows 10/11 a 64 bit; la finestra la apre **Edge**, già presente.

## 1. Avviare
1. Scarica `Nyctalus-Windows.zip` (dalla release) e **scompattalo** in una cartella tua (es.
   `Documenti\Nyctalus`): da dentro lo zip non parte.
2. Doppio clic su **`Nyctalus.cmd`**. La prima volta Windows può chiedere conferma
   («Ulteriori informazioni → Esegui comunque») e il firewall chiede l'accesso alla rete per
   `nyctalus.exe`: **Consenti**.
3. Si apre la finestra di Nyctalus. Quando dice **«● Nyctalus è pronto»** i riquadri si accendono.

## 2. Collegarsi a una rete
Tocca **🔗 Rete** e incolla il **codice di rete** (`NYCT1:…`) che ti dà chi la gestisce (a casa:
`nyctalus-rete casa` su un PC Linux, `GUIDA-RETE-DI-CASA.md`). Controlla che l'**impronta**
sia uguale a quella del gestore. Senza codice usi una rete di prova solo su questo PC.

## 3. Usare
| Riquadro | Cosa fa |
|---|---|
| 💬 **Chat** | la chat in una finestra sua: contatti, allegati, spunte, casella (`MANUALE-CHAT.md`) |
| 👥 **Rubrica** | chi usa la rete, chi è in linea, iscriversi con un nome |
| 🌐 **Browser** | come impostare **Firefox** su Nyctalus (§4) |
| ℹ️ **Info** | il sito con i manuali |

**⏻ Esci da Nyctalus** (in fondo) spegne tutto. Finché è acceso, Nyctalus riceve i messaggi
anche con le finestre chiuse; riaprile con un nuovo doppio clic su `Nyctalus.cmd`.

## 4. Navigare con Nyctalus (Firefox)
Il browser Nyctalus non c'è ancora per Windows: si usa Firefox, meglio con un **profilo dedicato**.
1. Firefox → Impostazioni → Generale → **Impostazioni di rete** → «Impostazioni…».
2. **Configurazione manuale dei proxy**: Host SOCKS `127.0.0.1`, porta **18733**, **SOCKS v5**.
3. Spunta **«Usa il proxy DNS con SOCKS v5»**.

Ora i siti, anche i `.nyct`, passano da Nyctalus. Protegge meno del nostro browser: non verifica
da solo di essere dentro la rete, e WebRTC va spento a mano (`about:config` →
`media.peerconnection.enabled` = false).

## 5. Dove stanno i dati e come togliere tutto
- Dati: `%USERPROFILE%\.nyctalus-mobile` (servizio) e `%USERPROFILE%\.nyctalus-chat` (identità,
  contatti, cronologia).
- Togliere: **⏻ Esci**, poi cancella la cartella scompattata; cancella anche le due cartelle dei
  dati se vuoi eliminare identità e messaggi.

## 6. Limiti dell'anteprima
- Non ancora provata su molti PC Windows: segnala cosa non va.
- Niente notifiche di Windows per ora: i messaggi si vedono nella finestra della chat.
- Nodi e gestore della rete: su Linux (`MANUALE-SERVER.md`, `MANUALE-GESTORE-RETE.md`).
