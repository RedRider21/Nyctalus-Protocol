# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Browser del PC in modalità 🦇, dentro uno spazio di rete isolato:
- un «sito» su 10.99.0.1:8080 conta chi lo raggiunge DIRETTAMENTE;
- un finto ingresso della rete Nyctalus (SOCKS5) risponde da sé e registra cosa gli chiedono.
Il SOCKS parte DOPO: nei primi secondi la scheda 🦇 non deve contattare il sito.
Stampa «OK …» / «KO …». uso: prova_browser_pc.py PORTA_SOCKS"""
import http.server
import socket
import struct
import threading
import time

import gi
gi.require_version("Gtk", "3.0")
gi.require_version("WebKit2", "4.0")
from gi.repository import GLib, Gtk  # noqa: E402
from nyctalus_browser import app as bapp  # noqa: E402

PS = int(__import__("sys").argv[1])
esito = lambda ok, d: print(("OK " if ok else "KO ") + d, flush=True)
diretti, richieste = [], []


class Sito(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        diretti.append(self.path)
        corpo = b"<title>DIRETTO</title>diretto"
        self.send_response(200); self.send_header("Content-Length", str(len(corpo))); self.end_headers()
        self.wfile.write(corpo)

    def log_message(self, *a):
        pass


class SenzaDNS(http.server.ThreadingHTTPServer):
    def server_bind(self):          # niente getfqdn(): farebbe una richiesta DNS sua, non del browser
        __import__("socketserver").TCPServer.server_bind(self)
        self.server_name, self.server_port = "sito", self.server_address[1]


def sito():
    SenzaDNS(("10.99.0.1", 8080), Sito).serve_forever()


def socks():
    """SOCKS5 minimo: annota la destinazione (nome o IP) e risponde con una pagina sua."""
    srv = socket.socket(); srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", PS)); srv.listen(16)
    while True:
        c, _ = srv.accept()
        threading.Thread(target=cliente, args=(c,), daemon=True).start()


def cliente(c):
    try:
        n = c.recv(2)[1]; c.recv(n); c.sendall(b"\x05\x00")
        _v, cmd, _r, tipo = c.recv(4)
        if tipo == 3:
            l = c.recv(1)[0]; dest, forma = c.recv(l).decode(), "nome"
        elif tipo == 1:
            dest, forma = socket.inet_ntoa(c.recv(4)), "ip"
        else:
            dest, forma = "?", "ipv6"
            c.recv(16)
        porta = struct.unpack("!H", c.recv(2))[0]
        richieste.append((dest, porta, forma))
        c.sendall(b"\x05\x00\x00\x01" + socket.inet_aton("0.0.0.0") + b"\x00\x00")
        c.recv(65536)
        corpo = ("<title>VIA-NYCTALUS %s:%d</title>ok" % (dest, porta)).encode()
        c.sendall(b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\nContent-Length: %d\r\n\r\n" % len(corpo) + corpo)
    except Exception:            # noqa: BLE001
        pass
    finally:
        c.close()


threading.Thread(target=sito, daemon=True).start()
bapp.Browser._trova_binario = lambda self: None         # niente rete di prova automatica: il SOCKS lo diamo noi
b = bapp.Browser(None)
titolo = lambda v: v.get_title() or ""


def passo1():
    b._v1 = b.new_tab("http://10.99.0.1:8080/pagina", stealth=True)
    GLib.timeout_add(4000, passo2)
    return False


def passo2():
    esito(not diretti, "rete in avvio: la scheda 🦇 NON contatta il sito (visite dirette: %d)" % len(diretti))
    esito("Nyctalus" in titolo(b._v1), "al suo posto la pagina d'attesa («%s»)" % titolo(b._v1))
    threading.Thread(target=socks, daemon=True).start()   # ora la rete «risponde»
    GLib.timeout_add(6000, passo3)
    return False


def passo3():
    schede = [v for v in b.stack.get_children() if b._view_mode.get(v)]
    t = [titolo(v) for v in schede]
    esito(any(x.startswith("VIA-NYCTALUS 10.99.0.1:8080") for x in t) and not diretti,
          "rete pronta: la scheda 🦇 riparte da sola e passa dal proxy Nyctalus (%s), nessuna visita diretta" % t)
    b._v2 = b.new_tab("http://sito-di-prova.example/", stealth=True)
    GLib.timeout_add(4000, passo4)
    return False


def passo4():
    # (http:// diventa https:// per la protezione «solo HTTPS»: porta 80 o 443)
    nomi = [r for r in richieste if r[0] == "sito-di-prova.example"]
    esito(nomi and all(f == "nome" for _d, _p, f in nomi),
          "il nome del sito arriva alla rete così com'è: lo risolve la rete, non il PC (%s)" % nomi)
    esito(all(f != "ipv6" for _d, _p, f in richieste), "nessuna richiesta anomala al proxy")
    b._v3 = b.new_tab("http://10.99.0.1:8080/normale", stealth=False)
    GLib.timeout_add(4000, passo5)
    return False


def passo5():
    esito(diretti and diretti[0] == "/normale" and titolo(b._v3) == "DIRETTO",
          "controllo: una scheda NORMALE raggiunge il sito direttamente (%s), quindi il contatore funziona" % diretti)
    Gtk.main_quit()
    return False


GLib.timeout_add(2500, passo1)
GLib.timeout_add(60000, lambda: (esito(False, "tempo scaduto"), Gtk.main_quit()))
Gtk.main()
