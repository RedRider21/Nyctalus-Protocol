<!--
SPDX-License-Identifier: AGPL-3.0-or-later
Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
-->

# Guida: provare con un amico via Internet

Tu a casa tua, il tuo amico in un'altra città: chat, file, chiamate e videochiamate
attraverso **la vostra** rete Nyctalus. Tempo: circa mezz'ora la prima volta.

**Come funziona, in una riga:** il **tuo PC** tiene accesa la rete (4 nodi + l'elenco dei
nodi) e la rende raggiungibile da Internet; il tuo amico **si collega** e basta, dal suo PC
(Linux o Windows) o dal telefono. Lui non deve aprire nulla sul suo router.

> ⚠️ **Cosa verifica questa prova, e cosa NO.** Verifica che Nyctalus **funzioni** fra due
> città (collegamento, router, velocità, chat, file, chiamate) e che tutto viaggi **cifrato**:
> nessuno, operatori compresi, legge messaggi, file o chiamate.
> **Non vi rende anonimi**: i nodi sono tutti sul PC di chi ospita, quindi lui vede l'IP
> dell'altro, e i vostri operatori vedono che i due indirizzi si parlano, quando e quanto.
> **Non vi rende invisibili**: si vede che c'è traffico cifrato (il camuffamento lo fa sembrare
> rumore, non lo fa sparire). Per un anonimato vero servono nodi su macchine di soggetti
> diversi e più utenti: vedi il [Piano verso una rete anonima](PIANO-ANONIMATO.md).

> Chi ospita la rete può essere anche il tuo amico: con un **IP statico** è perfino più
> semplice (al posto del nome No-IP si usa direttamente l'IP). I passi sono gli stessi a
> parti invertite.


## Cosa serve

| Chi | Cosa |
|---|---|
| **Tu** (ospiti la rete) | PC Linux con il pacchetto **PC-Linux** installato come *gestore* (`./installa.sh` → scelta 2); un **nome No-IP** che segue il tuo IP (es. `miacasa.ddns.net`); accesso al **router** |
| **Il tuo amico** | il pacchetto per il suo apparecchio, **della stessa data del tuo**: PC-Linux (scelta 1), Windows, oppure l'app Android |
| **Entrambi** | i pacchetti dalla stessa release su GitHub; un modo per sentirvi a voce o per messaggio (per confrontare l'impronta) |


## Passo 1 · Controlla di avere un IP pubblico (2 minuti)

Senza IP pubblico, gli altri non possono raggiungere il tuo PC e nessuna impostazione
del router può rimediare.

1. Nella pagina del router guarda l'**IP WAN** (o «IP pubblico», «Indirizzo Internet»).
2. Apri un sito come `https://ifconfig.me` e confronta.
3. **Uguali** → va bene, prosegui. **Diversi**, oppure l'IP WAN inizia con `100.64`–`100.127`,
   `10.`, `192.168.` → sei dietro **CGNAT**. Chiedi all'operatore un IP pubblico (spesso è
   gratuito), oppure fai ospitare la rete al tuo amico.

**TIM su fibra o ADSL** di solito dà un IP pubblico **dinamico**: va bene, è proprio il
caso del nome No-IP. Le offerte via radio (FWA/5G) di solito no.

## Passo 2 · Il nome No-IP sempre aggiornato (5 minuti)

Il nome deve puntare sempre al tuo IP attuale:

- **meglio nel router**: cerca la sezione **DNS dinamico** (DDNS), scegli No-IP e inserisci
  nome, utente e password;
- **altrimenti sul PC**: installa il programma di aggiornamento di No-IP (DUC).

Prova: `ping miacasa.ddns.net` deve mostrare lo stesso IP del passo 1.
Il nome gratuito di No-IP va **confermato ogni 30 giorni** (arriva una mail).

## Passo 3 · Dai al PC un indirizzo fisso in casa (2 minuti)

Nel router, sezione **DHCP** (o «Dispositivi»), **riserva** l'indirizzo attuale del PC
(es. `192.168.1.14`). Così l'inoltro del passo 4 non si rompe quando il PC si riaccende.
Il comando del passo 5 ti ricorda quale indirizzo ha il PC.

## Passo 4 · Inoltra le porte sul router (5 minuti)

Sezione **Port forwarding** / **Inoltro porte** / **Virtual server** / **Port mapping**
(sul TIM Hub di solito sta fra le impostazioni avanzate). Crea due regole verso
l'indirizzo del PC:

| Protocollo | Porte esterne | Verso il PC, porte |
|---|---|---|
| **UDP** | 4701-4704 | 4701-4704 (i nodi) |
| **TCP** | 18800 | 18800 (l'elenco dei nodi e la pagina del codice) |

Nient'altro va aperto: le porte della rubrica e della casella restano solo dentro il PC.

**Facoltativo (dalla 0.3.13):** inoltra **anche in TCP** le porte 4701-4704. Serve a chi si
collega da una rete che blocca UDP (ufficio, Wi-Fi di un albergo): il suo Nyctalus, se UDP non
risponde, passa da solo a TCP sulla stessa porta.

## Passo 5 · Accendi la rete raggiungibile da Internet

Se è accesa la rete di casa (`nyctalus-rete casa`), fermala con **Ctrl-C**: usa le stesse
porte. Poi:

```sh
nyctalus-rete casa --internet miacasa.ddns.net
```

Il comando:

- accende 4 nodi che si presentano col **nome** (non con l'IP: se l'IP cambia, non serve fare nulla);
- crea una rete a sé, «**Rete Internet di prova**», che non tocca la rete di casa;
- se il firewall del PC è attivo, chiede di aprire le porte (serve la password);
- ricorda quali porte inoltrare e verso quale indirizzo;
- **prova i nodi** passando dal nome pubblico: devono risultare `● online`;
- stampa il **codice di rete** e l'**impronta** (es. `93CF-D836`).

**Lascia la finestra aperta** finché fate le prove; Ctrl-C spegne tutto.

> Se la prova dice che i nodi **non rispondono**, spesso la causa è il router di casa che
> non fa «tornare dentro» i pacchetti diretti al proprio nome (*NAT loopback*). Da fuori
> funziona, ma il PC non riesce a raggiungersi da solo. Rimedio, **solo su questo PC**:
> aggiungi a `/etc/hosts` la riga `127.0.0.1 miacasa.ddns.net` e rilancia il comando.
> Il telefono, se è in casa sulla Wi-Fi, in quel caso usalo con i **dati mobili**.

## Passo 6 · Il tuo amico si collega

**Prima prova (dal suo browser, 10 secondi):** apre `http://miacasa.ddns.net:18800/`.
Se vede la pagina «Rete «Rete Internet di prova»» con l'impronta, l'inoltro funziona.
Se non si apre, vedi la tabella dei problemi.

Poi, a seconda dell'apparecchio:

| Apparecchio | Come si collega |
|---|---|
| **Telefono Android** | dalla pagina appena aperta tocca **📲 Collega l'app Nyctalus** e conferma |
| **PC Linux** | mini menu → ⚙ → **🔗 Collegati a una rete…** e incolla il codice che gli mandi (oppure, da terminale, `nyctalus-rete importa 'NYCT1:…'`); poi chiude e riapre chat e browser |
| **Windows** | nella finestra di Nyctalus tocca **🔗 Rete** e incolla il codice |

**Confrontate a voce l'impronta**: quella che vede lui dopo l'importazione deve essere
uguale a quella stampata dal tuo PC. Così siete sicuri che nessuno abbia sostituito il codice.

Anche **tu** usi questa rete: il PC si collega da solo; sul telefono apri la stessa pagina
e tocca 📲.

## Passo 7 · Trovatevi e provate tutto

1. Aprite la **chat** (PC: mini menu → Chat · telefono: 💬 Chat).
2. **👥 Rubrica**: iscrivetevi con un nome e vi vedrete a vicenda «in linea». Un clic sul
   nome e il contatto è aggiunto. In alternativa vi scambiate i Contact ID.
3. Provate nell'ordine: **messaggio** → **foto o file** → **chiamata** (menu ⋮ → Chiamata
   vocale) → **videochiamata** (⋮ → Videochiamata, oppure 📹 durante la chiamata).
4. Annotate, per le chiamate: ritardo percepito, qualità (📶) e se la voce si interrompe.
   Su Internet ogni pacchetto fa 3 salti fra i nodi, quindi un piccolo ritardo è normale.
5. **Collaudo**: su ciascun PC (e sul telefono, nel ⌨ Terminale) lanciate
   `nyctalus-collaudo --guidato --rapporto collaudo.txt`: controlla nodi, IP, DNS, velocità e
   chat e vi fa le domande sulle prove a mano. Dettagli: [Collaudo](MANUALE-COLLAUDO.md).


## Passo 8 (facoltativo) · Anche il tuo amico ospita dei nodi

Così la rete non sta più tutta a casa tua, e ogni percorso usa nodi di **gestori diversi**.
Il tuo amico con l'IP statico è il candidato ideale.

1. **Lui**, su un PC Linux o un Raspberry sempre acceso, scompatta `Nyctalus-Server-Linux.zip`
   (`-arm64` per il Raspberry) e lancia `./installa.sh --utente` (oppure `sudo ./installa.sh`).
   L'installatore chiede l'IP pubblico (il suo IP statico) e alla fine **stampa delle righe**:
   te le manda (non sono segrete).
2. **Lui** inoltra sul suo router le porte **UDP** dei nodi (quelle che dice l'installatore)
   verso quel PC, e se vuole anche le stesse in **TCP** (per chi ha UDP bloccato).
3. **Tu**, con la rete di casa accesa:
   ```sh
   nyctalus-rete ospite aggiungi --internet 'RIGA1' 'RIGA2' --famiglia mario
   nyctalus-rete ospite elenco --internet
   ```
   (`mario` = il suo nome: dice che quei nodi sono suoi). L'elenco si aggiorna subito; chi è
   già collegato li usa alla prossima apertura della chat. Restano anche ai riavvii.
4. Per togliere un suo nodo: `nyctalus-rete ospite togli --internet IP:PORTA`.

**Quante famiglie servono:** un percorso ha 3 nodi, tutti di gestori diversi. Con i tuoi
nodi e quelli del tuo amico le famiglie sono **2**: la rete funziona, ma il motore avvisa
«NON anonimo», perché un percorso deve per forza usare due nodi dello stesso gestore. Con un
**terzo** gestore (un altro amico, o un piccolo server affittato) i percorsi diventano tutti
di 3 gestori diversi. Il perché, in `PIANO-ANONIMATO.md` (Tappa 3).

## Se qualcosa non va

| Sintomo | Causa probabile · cosa fare |
|---|---|
| l'amico non apre `http://NOME:18800/` | inoltro TCP 18800 mancante o verso l'indirizzo sbagliato (passi 3-4); nome No-IP non aggiornato (`ping NOME` ≠ IP del router); firewall del PC (il comando del passo 5 lo dice); CGNAT (passo 1) |
| la pagina si apre ma la chat dice «rete non raggiungibile» | inoltro **UDP 4701-4704** mancante (la pagina usa TCP, i nodi UDP: servono tutte e due le regole) |
| sul tuo PC i nodi non rispondono, all'amico sì | NAT loopback del router: riga in `/etc/hosts` (passo 5) |
| funzionava, poi più nulla | il PC è stato riavviato o la finestra chiusa (rilancia il passo 5); l'IP è cambiato e No-IP non si è aggiornato; il nome gratuito non è stato confermato |
| impronte diverse | **non usate quella rete**: rifate l'importazione dal codice giusto |
| chiamata a scatti | linea di upload lenta su uno dei due lati o sul tuo (che ospita i nodi: tutto il traffico passa da te). Provate solo voce |

## Da sapere (onestamente)

- È un'**anteprima**: va benissimo per provare, ma il codice non è ancora stato verificato
  da esperti esterni. Non affidateci segreti veri.
- Con tutti i nodi sul tuo PC la rete **funziona**, ma verso di **te** non è anonima: tu
  ospiti ogni salto. Per l'anonimato vero servono nodi su macchine e linee diverse.
- **Sempre accesa, senza tenere acceso il PC:** su un Raspberry Pi basta `./rete-raspberry.sh
  --nome NOME` (`MANUALE-SERVER.md` §6-ter); su un PC sempre acceso
  `nyctalus-rete casa --internet NOME --servizio`.
- **La navigazione degli amici esce da casa tua.** Con i nodi sul tuo PC, chi naviga sui siti
  normali con il browser 🦇 lo fa con il **tuo** indirizzo. Se non vuoi, accendi la rete con
  `nyctalus-rete casa --internet NOME --senza-uscita` (dalla 0.3.22): chat, chiamate, file e
  siti `.nyct` funzionano lo stesso, la navigazione su Internet no (serve un'uscita su un server).
- **Passo successivo, facoltativo:** il tuo amico, con l'IP statico, ospita **anche lui** un
  paio di nodi (Passo 8). Così la rete diventa distribuita fra due città.
- Tutto il traffico in rete è cifrato e senza parole riconoscibili: lo dimostra la prova
  «Wireshark in mezzo» (`SICUREZZA.md` §1.1). Da fuori si capisce solo **quando** state
  usando molta banda, non cosa né con chi.
