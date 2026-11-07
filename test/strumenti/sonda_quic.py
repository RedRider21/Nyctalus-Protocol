#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Sonda come quella di un censore: manda a un indirizzo UDP datagrammi che un nodo QUIC
potrebbe prendere per l'inizio di un collegamento (intestazione lunga con una versione sconosciuta,
più byte a caso), e conta le risposte. Un nodo QUIC scoperto risponde con la «negoziazione di
versione»; un nodo camuffato e resistente alle sonde non risponde mai.
uso: sonda_quic.py PORTA [TENTATIVI]  → stampa «RISPOSTE n»"""
import os
import socket
import sys

porta = int(sys.argv[1])
n = int(sys.argv[2]) if len(sys.argv) > 2 else 40
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.settimeout(0.15)
risposte = 0
for i in range(n):
    if i % 2 == 0:   # come un Initial QUIC con versione sconosciuta: un nodo QUIC scoperto risponde
        d = bytes([0xc0]) + b"\x0a\x0a\x0a\x0a" + bytes([8]) + os.urandom(8) + bytes([8]) + os.urandom(8) + os.urandom(1300)
    else:            # byte del tutto a caso, di lunghezze diverse
        d = os.urandom(40 + (i * 97) % 1400)
    s.sendto(d, ("127.0.0.1", porta))
    try:
        while True:
            s.recvfrom(4096)
            risposte += 1
    except socket.timeout:
        pass
print("RISPOSTE", risposte)
