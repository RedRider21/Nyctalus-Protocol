# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""nyctalus-cli fra due motori della chat (Alice e Bob) sulla rete di prova: identità, contatti,
messaggi in tempo reale, conversazione, file grandi (sotto e sopra soglia), impostazioni, errori.
Tutto SOLO attraverso il comando. Stampa «OK …» / «KO …».
uso: prova_cli.py CLI PORTA_A SEGRETO_A PORTA_B SEGRETO_B IND_A IND_B CARTELLA"""
import hashlib
import os
import queue
import subprocess
import sys
import threading
import time

cli, pa, sa, pb, sb, ind_a, ind_b, cartella = sys.argv[1:9]
esito = lambda ok, d: print(("OK " if ok else "KO ") + d, flush=True)
# HOME finto: il comando non deve MAI leggere il collegamento del motore vero dell'utente
casa = os.path.join(cartella, "casa-finta")
os.makedirs(casa, exist_ok=True)


def amb(porta, segreto):
    return dict(os.environ, HOME=casa, NYCTALUS_CHAT_PORT=porta, NYCTALUS_CHAT_TOKEN=segreto)


def a(*args, attesa=180):
    r = subprocess.run([sys.executable, cli, *args], env=amb(pa, sa), capture_output=True, text=True, timeout=attesa)
    return r.returncode, r.stdout + r.stderr


def b(*args, attesa=180):
    r = subprocess.run([sys.executable, cli, *args], env=amb(pb, sb), capture_output=True, text=True, timeout=attesa)
    return r.returncode, r.stdout + r.stderr


def impronta(p):
    return hashlib.sha256(open(p, "rb").read()).hexdigest()


def crea(nome, mb):
    p = os.path.join(cartella, nome)
    with open(p, "wb") as f:
        f.write(os.urandom(int(mb * 1024 * 1024)))
    return p


def trova(radice, nome):
    for d, _, ff in os.walk(radice):
        if nome in ff:
            return os.path.join(d, nome)
    return ""


def aspetta(funz, secondi, passo=1.0):
    fine = time.time() + secondi
    while time.time() < fine:
        r = funz()
        if r:
            return r
        time.sleep(passo)
    return None


# 1) identità e contatti
c, o = a("io")
esito(c == 0 and o.strip() == ind_a, "«io» dà il Contact ID di Alice")
c, o = a("aggiungi", ind_b, "Bob", "Rossi")
esito(c == 0 and "Bob Rossi" in o, "Alice aggiunge «Bob Rossi» dal terminale")
b("aggiungi", ind_a[:-5], "Alice")                          # anche senza «.nyct»
c, o = a("contatti")
esito(c == 0 and "Bob Rossi" in o and ind_b in o, "«contatti» elenca Bob con il suo indirizzo")
c, o = a("aggiungi", "nonvalido", "X")
esito(c != 0 and "non valido" in o, "indirizzo sbagliato: errore chiaro, codice d'uscita ≠ 0")

# 2) Bob ascolta in tempo reale, Alice scrive (nome abbreviato e senza maiuscole)
q = queue.Queue()
asc = subprocess.Popen([sys.executable, cli, "ascolta"], env=amb(pb, sb), stdout=subprocess.PIPE,
                       stderr=subprocess.STDOUT, text=True)
threading.Thread(target=lambda: [q.put(r) for r in asc.stdout], daemon=True).start()
time.sleep(1)
inviato = aspetta(lambda: a("scrivi", "bob", "ciao dal terminale, àèìòù 🦇")[0] == 0, 120, 3)
esito(bool(inviato), "Alice: «scrivi bob …» (nome abbreviato) → inviato attraverso la rete")


def dall_ascolto(testo, secondi):
    fine = time.time() + secondi
    while time.time() < fine:
        try:
            r = q.get(timeout=0.5)
        except queue.Empty:
            continue
        if testo in r:
            return r
    return None


r = dall_ascolto("ciao dal terminale, àèìòù 🦇", 60)
esito(bool(r) and "Alice:" in r, "Bob in «ascolta» lo vede arrivare subito, con il nome e gli accenti")
c, o = a("leggi", "Bob", "-n", "5")
esito(c == 0 and "tu: ciao dal terminale" in o, "Alice: «leggi Bob» mostra la conversazione")
c, o = b("leggi", "alice")
esito(c == 0 and "Alice: ciao dal terminale" in o, "Bob: «leggi alice» mostra il messaggio ricevuto")

# 3) file grande sotto soglia: parte da solo, arriva identico
f1 = crea("dal-terminale.bin", 3)
c, o = a("invia", "Bob", f1)
esito(c == 0 and "in partenza" in o, "Alice: «invia Bob file» → in partenza")
arrivato = aspetta(lambda: trova(os.path.join(cartella, "bob"), "dal-terminale.bin"), 240, 2)
esito(bool(arrivato) and impronta(arrivato) == impronta(f1), "il file arriva a Bob IDENTICO")
fatto = aspetta(lambda: "fatto" in a("file")[1], 60, 2)
esito(bool(fatto), "Alice: «file» lo mostra «fatto» al 100%")
c, o = b("file")
esito(c == 0 and "dal-terminale.bin" in o and "⇣ da" in o, "Bob: «file» lo mostra in arrivo da Alice")

# 4) impostazioni e file sopra soglia con «accetta»
c, o = b("impostazioni", "soglia", "1", "canali", "2")
esito(c == 0 and "1,0 MB" in o and "2" in o, "Bob: «impostazioni soglia 1 canali 2» (in MB)")
c, o = b("impostazioni", "canali", "999x")
esito(c != 0, "impostazione sbagliata: rifiutata")
f2 = crea("grande.bin", 2)
a("invia", "Bob", f2)
proposta = dall_ascolto("vuole mandarti «grande.bin»", 120)
esito(bool(proposta) and "nyctalus-cli accetta" in proposta, "sopra soglia: Bob in «ascolta» vede la richiesta con il comando da dare")
ident = proposta.rsplit(" ", 1)[1].strip() if proposta else "x"
c, o = b("accetta", ident)
esito(c == 0, "Bob: «accetta <inizio dell'id>»")
arr2 = aspetta(lambda: trova(os.path.join(cartella, "bob"), "grande.bin"), 240, 2)
esito(bool(arr2) and impronta(arr2) == impronta(f2), "accettato: arriva identico")

# 5) pulizia dell'elenco ed errori
aspetta(lambda: a("file")[1].count("fatto") >= 2, 60, 2)
c, o = a("pulisci")
esito(c == 0 and "tolti dall'elenco: 2" in o, "Alice: «pulisci» toglie i 2 trasferimenti conclusi")
c, o = a("file")
esito(c == 0 and "Nessun trasferimento" in o, "l'elenco di Alice è vuoto (i file restano)")
c, o = a("scrivi", "Nessuno", "x")
esito(c != 0 and "nessun contatto" in o, "contatto inesistente: errore chiaro")
r = subprocess.run([sys.executable, cli, "io"], env=dict(amb(pa, "segretosbagliato")), capture_output=True, text=True, timeout=30)
esito(r.returncode != 0 and "segreto" in r.stderr, "segreto sbagliato: il motore rifiuta, il comando lo dice")
r = subprocess.run([sys.executable, cli, "io"], env=dict(amb("1", "x")), capture_output=True, text=True, timeout=30)
esito(r.returncode != 0 and "non è acceso" in r.stderr, "motore spento: suggerisce come accenderlo")
asc.terminate()
