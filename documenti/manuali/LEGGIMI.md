<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Inizia qui

**Nyctalus** è una rete anonima con chat, chiamate, videochiamate, browser e siti `.nyct`.
Tutto passa cifrato attraverso **3 nodi**: nessuno vede chi parla con chi, nemmeno i nodi.

> 📘 **Tutto in una pagina:** [Guida completa](GUIDA-COMPLETA.md): cosa scaricare, come si
> configura un client, un nodo o il gestore, e tutte le opzioni spiegate (a cosa servono e come
> si accendono).

## Cosa vuoi fare?

| Voglio… | Segui | Tempo |
|---|---|---|
| **provare da solo**, su un apparecchio | [Installazione](MANUALE-INSTALLAZIONE.md), poi apri la chat: c'è già una rete di prova | 10 min |
| **parlare fra il mio PC e il mio telefono**, a casa | [Guida: rete di casa](GUIDA-RETE-DI-CASA.md), 5 passi | 15 min |
| **provare con un amico via Internet** | [Guida: con un amico via Internet](GUIDA-INTERNET.md), 7 passi | 30 min |
| **una rete vera** con server (anche di amici, anche Raspberry) | [Server](MANUALE-SERVER.md) per i nodi + [Gestore della rete](MANUALE-GESTORE-RETE.md) per l'elenco e il codice | 1 ora |
| **pubblicare un sito `.nyct`** | [Siti .nyct](MANUALE-SITI.md) | 10 min |
| **capire e accendere le opzioni** (modalità riservata, protezione, famiglie…) | [Guida completa](GUIDA-COMPLETA.md), Parte 3 | 5 min |

## Come si usa

| Argomento | Manuale |
|---|---|
| chat, contatti, rubrica, allegati, **chiamate e videochiamate**, temi | [Chat](MANUALE-CHAT.md) — vale per PC e telefono |
| l'app sul telefono: avvio, impostazioni, browser 🦇, notifiche | [Telefono](MANUALE-TELEFONO.md) |
| **mandare file grandi** ai contatti (pausa, ripresa, più veloce) | [File](MANUALE-FILE.md) |
| chat e file **scrivendo comandi** (terminale, SSH, server, script) | [Terminale](MANUALE-TERMINALE.md) |
| il PC: mini menu, browser, dove stanno i dati | [PC Linux](MANUALE-PC.md) |
| Windows (anteprima) | [Windows](MANUALE-WINDOWS.md) |

## Le parole da sapere

| Parola | Cosa vuol dire |
|---|---|
| **Client** | chi **usa** la rete: il tuo PC, il tuo telefono |
| **Nodo** | un programma che **inoltra** il traffico cifrato. Sta su un computer sempre acceso e raggiungibile (server, Raspberry, o il PC di casa). Una rete ne ha almeno 3, meglio 4 |
| **Gestore** | chi **firma l'elenco dei nodi** e dà il codice di rete (basta un PC) |
| **Codice di rete** `NYCT1:…` | l'unica cosa che serve per collegarsi a una rete. Non è segreto |
| **Impronta** (es. `92E6-9EB0`) | il «controllo» del codice: chi si collega deve vedere la stessa impronta del gestore |
| **Contact ID** `….nyct` | il tuo indirizzo nella chat, da dare ai contatti |

I **telefoni sono solo client**: cambiano indirizzo e Android li mette a dormire, quindi non
possono fare da nodo.

## Quale pacchetto scaricare

Dalla [release su GitHub](https://github.com/RedRider21/Nyctalus-Protocol/releases):

| Apparecchio | Pacchetto | Poi |
|---|---|---|
| PC Linux | `Nyctalus-PC-Linux.zip` | `./installa.sh` → **1** client · **2** gestore (anche client) · **3** nodo |
| Telefono Android | `Nyctalus-Android.zip` | installa l'APK, poi nell'app tocca **Installa / aggiorna Nyctalus in Termux** |
| Windows (anteprima) | `Nyctalus-Windows.zip` | scompatta, doppio clic su `Nyctalus.cmd` |
| Server, VPS | `Nyctalus-Server-Linux.zip` | `sudo ./installa.sh` |
| Raspberry Pi, ARM | `Nyctalus-Server-Linux-arm64.zip` | `sudo ./installa.sh` |

Usate tutti pacchetti **della stessa data**.

## Domande rapide

- **Mi serve un server per provare?** No: da solo, rete di casa, o con un amico usando il PC
  di casa (servono solo un IP pubblico e due regole sul router).
- **Che server prendere per una rete vera?** Un VPS Linux qualsiasi, anche il più piccolo
  (1 CPU, 512 MB-1 GB di RAM), con IP pubblico e porte **UDP** apribili.
- **Tutti i nodi su un solo server o un solo PC vanno bene?** Per provare sì. Per l'anonimato
  no: chi li ospita vedrebbe entrata e uscita. Servono nodi su macchine e fornitori diversi.
- **Ho aggiunto o cambiato nodi: devo ridare il codice?** No. Si ridà solo se cambia
  l'indirizzo dell'elenco o se la rete viene ricreata.
- **Chi ha il codice può alterare la rete?** No: serve la chiave segreta del gestore, che non
  esce mai dal suo PC (`~/.nyctalus-rete`). Fanne una copia di sicurezza.
- **Sono anonimo?** Il contenuto è sempre cifrato; l'**anonimato** (nessuno sa chi parla con chi) dipende da dove stanno i nodi e da quanti usano la rete. Con i nodi tutti su un PC, come nella prova con un amico, **non** lo siete. Le tappe per arrivarci: [Piano verso una rete anonima](PIANO-ANONIMATO.md).
- **Com'è rispetto a Tor?** Misurato nella stessa Internet simulata: stessa velocità e stessa
  resistenza sugli scaricamenti; sulla chat, con la modalità riservata, chi vede tutte le linee
  non capisce chi scrive a chi. Tor resta molto più maturo e collaudato. Tabella completa:
  [Nyctalus e Tor, a confronto](PIANO-ANONIMATO.md#nyctalus-e-tor-a-confronto).
- **È sicuro?** Cosa si verifica e come: [Sicurezza](SICUREZZA.md), compresa la prova
  «Wireshark in mezzo». Rifare tutte le prove: [Prove](MANUALE-PROVE.md). Tutte le opzioni
  tecniche: [Deploy](DEPLOY.md).
