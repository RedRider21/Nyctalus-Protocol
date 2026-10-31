<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Manuale dei siti `.nyct`

Un sito **`.nyct`** si raggiunge solo dalla rete Nyctalus. Chi lo visita non sa dove sta il
computer che lo ospita, e il sito non sa chi lo visita. Tutto si fa con un comando:
**`nyctalus-sito`**. Lo installano il pacchetto PC (`./installa.sh`) e il pacchetto server
(`./nyctalus-sito` nella cartella scompattata).

## 0. Due modi di ospitare il sito

| | **Cartella** (server integrato) | **Apache / nginx** (server tuo) |
|---|---|---|
| Cos'è | una cartella con `index.html`, CSS, immagini: è la `www` del sito | il tuo server web, in ascolto **solo** su `127.0.0.1` |
| Pagine | fisse (HTML, CSS, JS) | anche dinamiche: PHP, WordPress, un'applicazione |
| Serve installare | niente: c'è dentro Nyctalus | Apache/nginx/PHP |
| Comando | `nyctalus-sito crea NOME` (o `--cartella DIR`) | `nyctalus-sito crea NOME --backend 127.0.0.1:8080` |
| Dettagli | §2 | §3 |

In tutti e due i casi Nyctalus porta i visitatori al sito senza rivelare né chi visita né
dove sta il sito.

## 1. Il modo più semplice
```sh
nyctalus-sito
```
È una guida passo passo: chiede il nome, che tipo di sito è e su quale rete metterlo, poi
lo mette online subito o lo tiene **sempre acceso**.

## 2. Gli stessi passi, a comandi
```sh
nyctalus-sito crea mio-blog          # sito nuovo con una pagina d'esempio; stampa l'indirizzo .nyct
nyctalus-sito avvia mio-blog         # online finché non premi Ctrl-C
nyctalus-sito servizio mio-blog      # oppure: sempre acceso, anche dopo un riavvio
nyctalus-sito elenco                 # i tuoi siti, con indirizzo e stato (online / spento)
```
- **Il contenuto** sta in `~/.nyctalus-siti/mio-blog/www/`. Modifica `index.html` e
  `stile.css`, aggiungi pagine, cartelle e immagini: si vedono subito, senza riavviare.
- **Una cartella che hai già:** `nyctalus-sito crea mio-blog --cartella ~/il-mio-sito`.
  Se contiene già un `index.html`, non viene toccato.
- **Consiglio:** tieni tutto nella cartella e usa link relativi (`stile.css`, non
  `https://…`). Niente font, script o immagini presi da altri siti: chi naviga in modo
  anonimo non li caricherebbe, e lascerebbero tracce.

## 3. Siti dinamici (PHP, un'applicazione, un CMS)
Il server integrato mostra pagine fisse. Per un sito dinamico tieni acceso il tuo server
web **solo su questo computer** (`127.0.0.1`) e fai arrivare lì i visitatori:
```sh
php -S 127.0.0.1:8080 -t ~/mio-sito-php        # oppure nginx/Apache in ascolto su 127.0.0.1:8080
nyctalus-sito crea mio-sito --backend 127.0.0.1:8080
nyctalus-sito avvia mio-sito
```
Indirizzi diversi da `127.0.0.1` vengono rifiutati: il server web non deve essere
raggiungibile da Internet, solo da Nyctalus.

**Con Apache** (o nginx) è come la solita cartella `www`, con tre regole:
1. un `VirtualHost` **solo su `127.0.0.1:8080`** (`Listen 127.0.0.1:8080`): se lo stesso sito fosse
   raggiungibile anche da Internet, chiunque potrebbe collegare il `.nyct` al tuo IP;
2. `ServerTokens Prod` e `ServerSignature Off`: il server non dice nome e versione;
3. nel sito niente indirizzi assoluti verso altri domini o verso il tuo IP.

Apache vede tutte le visite come da `127.0.0.1`: non sa chi sta visitando.

## 4. Su quale rete
Di solito su quella di questo computer, cioè quella importata con `nyctalus-rete importa`
o accesa con `nyctalus-rete casa`. Su un server che non usa la chat si passa il codice della
rete:
```sh
./nyctalus-sito crea mio-sito --codice NYCT1:…     # il codice te lo dà chi gestisce la rete
nyctalus-sito rete mio-sito                        # su quale rete sta
nyctalus-sito rete mio-sito --codice NYCT1:…       # spostarlo su un'altra rete (poi riavvialo)
```
Il sito lo vede **solo chi è sulla stessa rete**. Con la rete di casa sono i dispositivi a cui
hai dato il codice. Per un sito pubblico serve una rete condivisa con nodi su server diversi
(scenario D in `SCELTE-OPERATIVE.md`). L'elenco dei nodi si riscarica ogni 6 ore: se cambia, il
sito riparte da solo.

## 5. Sempre acceso
```sh
nyctalus-sito servizio mio-blog            # servizio di sistema dell'utente: riparte da solo
nyctalus-sito servizio mio-blog --togli    # torna «solo quando lo avvio io»
```
È un servizio **systemd dell'utente**, quindi non serve root. Resta acceso finché sei collegato;
per tenerlo acceso anche dopo l'uscita (per esempio su un server) esegui una volta
`sudo loginctl enable-linger $USER`. Il comando te lo ricorda. Stato e registro:
`systemctl --user status nyctalus-sito-mio-blog` e `journalctl --user -u nyctalus-sito-mio-blog`.

## 6. La chiave è l'indirizzo
Il file `~/.nyctalus-siti/NOME/identita` è la **chiave del sito**, e l'indirizzo `.nyct` è
calcolato da lei:
- **fanne una copia di sicurezza:** `nyctalus-sito esporta mio-blog ~/chiave-mio-blog.nyctkey`;
- per **spostare il sito** su un altro computer con lo stesso indirizzo:
  `nyctalus-sito crea mio-blog --chiave ~/chiave-mio-blog.nyctkey` (poi copia anche i file);
- **non darla a nessuno:** chi la possiede può mettere online un sito falso al tuo indirizzo.

Per cancellare un sito: `nyctalus-sito togli mio-blog` (chiede conferma). La chiave resta in una
copia in `~/.nyctalus-siti/.chiavi-tolte/`, così puoi riprenderne l'indirizzo. I file di una
cartella tua (`--cartella`) non vengono toccati.

## 7. Visitare un sito
- **PC:** browser Nyctalus in **modalità 🦇**, scrivi l'indirizzo `….nyct`.
- **Telefono:** app Nyctalus → **🌐 Browser** → modalità 🦇.
- **Da terminale**, per una prova veloce:
  `nyctalus browse INDIRIZZO.nyct --directory directory.txt --authority CHIAVE`.

## 8. In breve, cosa resta sul tuo computer
| Dove | Cosa |
|---|---|
| `~/.nyctalus-siti/NOME/sito.json` | nome, indirizzo, tipo, rete (file 600) |
| `~/.nyctalus-siti/NOME/identita` | la chiave del sito (600) |
| `~/.nyctalus-siti/NOME/www/` | le pagine (se non hai scelto un'altra cartella) |
| `~/.nyctalus-siti/NOME/directory.txt` | l'ultimo elenco dei nodi scaricato |
| `~/.config/systemd/user/nyctalus-sito-NOME.service` | solo se è «sempre acceso» |

Il server integrato non tiene registri di chi visita il sito.
