#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Pilota della chat dentro Shadow: aggiunge il contatto, accende (o no) la modalità riservata e manda
messaggi agli orari dati (secondi dall'inizio della simulazione).
uso: pilota_chat.py PORTA SEGRETO RISERVATA(0|1) CONTATTO T1,T2,… AVVIO_DEL_PROCESSO"""
import socket
import sys
import threading
import time

zero = time.monotonic()      # istante d'avvio del processo (vedi AVVIO_DEL_PROCESSO)
porta, segreto, riservata, contatto, orari = int(sys.argv[1]), sys.argv[2], sys.argv[3] == "1", sys.argv[4], sys.argv[5]
for _ in range(120):
    try:
        s = socket.create_connection(("127.0.0.1", porta))
        break
    except OSError:
        time.sleep(1)
risposte = []
threading.Thread(target=lambda: [risposte.append(r) for r in s.makefile("rb")], daemon=True).start()
manda = lambda r: s.sendall((r + "\n").encode())
manda("AUTH " + segreto)
manda("ADD %s %s" % (contatto, "amico".encode().hex()))
if riservata:
    for _ in range(30):
        manda("RISERVATA 1")
        time.sleep(2)
        if any(r.startswith(b"RISERVATA 1 ") for r in risposte):
            break
    print("riservata:", any(r.startswith(b"RISERVATA 1 ") for r in risposte), flush=True)
# in Shadow l'orologio parte dall'inizio della simulazione (1 gennaio 2000): si usano i secondi
# trascorsi dal primo istante visto qui, più l'ora d'avvio del processo indicata nella configurazione
avvio = float(sys.argv[6]) if len(sys.argv) > 6 else 0.0
for i, t in enumerate(float(x) for x in orari.split(",")):
    time.sleep(max(0.0, t - avvio - (time.monotonic() - zero)))
    manda("SEND %s %s" % (contatto, ("messaggio %d" % i).encode().hex()))
    print("inviato %d a %.1f s" % (i, avvio + time.monotonic() - zero), flush=True)
time.sleep(120)
