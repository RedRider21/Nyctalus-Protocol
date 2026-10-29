<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol. -->

# Nyctalus Browser

Client grafico di **Nyctalus Protocol**: un **clone di Firefox** costruito su
GTK3 + WebKit2, con una **modalità stealth** attivabile/disattivabile. È
derivato dal browser di NexusSec OS e adattato per Nyctalus (è un progetto a
sé: il browser di NexusSec OS **non** viene toccato).

> ⚠️ **Prototipo.** La modalità stealth instrada verso un proxy SOCKS locale
> (`127.0.0.1:1080` di default, cioè il demone `nyctalus start`); se non trova
> nessun proxy la navigazione resta in chiaro. Non proteggere ancora dati reali.

## Dipendenze

- Python 3
- PyGObject (GTK 3)
- WebKit2GTK — `WebKit2 4.1` se presente, altrimenti fallback automatico a `4.0`

Su Debian/Ubuntu/NexusSec:

```sh
sudo apt install python3-gi gir1.2-gtk-3.0 gir1.2-webkit2-4.1
# oppure, su sistemi più vecchi:
sudo apt install python3-gi gir1.2-gtk-3.0 gir1.2-webkit2-4.0
```

## Avvio

```sh
./nyctalus-browser                 # pagina iniziale
./nyctalus-browser example.com     # apre un sito
./nyctalus-browser pagina.html     # apre un file locale
```

Il launcher mette `browser/` nel `PYTHONPATH` (così trova `nyctalus_browser`
e `nxs_i18n`) e lancia `python3 -m nyctalus_browser`.

## Caratteristiche (clone di Firefox)

- Schede con trascinamento, riordino, audio per-scheda, riapri scheda chiusa.
- Barra di navigazione con pill dell'URL, lucchetto HTTPS, avanti/indietro/ricarica/home.
- **Sidebar dei preferiti** (apribile dal rail o dal menu), con aggiungi/modifica/elimina.
- Cronologia, gestore download, barra di ricerca nella pagina (Ctrl+F).
- **Menu ☰** (come Firefox): nuova scheda, chiudi/riapri scheda, aggiungi
  segnalibro, preferiti, trova, stampa, salva pagina, sorgente, zoom,
  **schermo intero**, tema, ispettore, cronologia, download, impostazioni, esci.
- **Impostazioni** (come le preferenze di Firefox):
  - *Generale*: pagina iniziale, motore di ricerca, **lingua dell'interfaccia**
    (it/en/fr/es/de), ripristino sessione.
  - *Download*: cartella o "chiedi ogni volta".
  - *Aspetto e privacy*: tema scuro, **stealth**, solo-HTTPS, anti-tracciamento,
    cronologia on/off, cancellazione dati.
  - *Traduzione*: lingua di destinazione, servizio (Google / LibreTranslate con
    URL e chiave).
- **Pulsante Stealth** sempre in vista: accende/spegne l'instradamento nella rete.
- **Traduci pagina** integrato.

### Scorciatoie (come Firefox)

`Ctrl+T` nuova scheda · `Ctrl+W` chiudi · `Ctrl+Shift+T` riapri · `Ctrl+L`
barra indirizzi · `Ctrl+R`/`F5` ricarica · `Ctrl+D` aggiungi segnalibro ·
`Ctrl+F` trova · `Ctrl+H` cronologia · `Ctrl+J` download · `Ctrl+P` stampa ·
`Ctrl+S` salva · `Ctrl+U` sorgente · `Ctrl +/-/0` zoom · `F11` schermo intero ·
`F12` ispettore.

## Modalità stealth e proxy

Di default lo stealth usa il proxy SOCKS su `127.0.0.1:1080`. Si cambia da
`~/.nyctalus-browser/config.json`:

```json
{ "socks_host": "127.0.0.1", "socks_port": 1080 }
```

Perché la navigazione stealth sia davvero anonima serve il demone attivo:

```sh
nyctalus start      # avvia il proxy SOCKS5 attraverso la ragnatela conflux
```

## Dati

Tutto in `~/.nyctalus-browser/` (config, preferiti, cronologia). La modalità
anonima non lascia tracce su disco: vive solo in RAM.

## Lingua

L'interfaccia è disponibile in italiano, inglese, francese, spagnolo e tedesco.
Si sceglie da **Impostazioni → Generale → Lingua dell'interfaccia**; il cambio
si applica alla riapertura della finestra. In alternativa, variabile d'ambiente
`NXS_LANG=en` o file `~/.config/nxs/lang`.

## Licenza

[AGPL-3.0-or-later](../LICENSE). © 2026 Daniele Deplano (RedRider21).
Derivato dal browser di NexusSec OS.
