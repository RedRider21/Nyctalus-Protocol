# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Piccolo sito di prova per Nyctalus Firefox: la pagina, con uno script, rimanda al server cosa
vede il browser (WebRTC, fuso orario, lingua, piattaforma). Ogni richiesta finisce nel registro.
uso: server_impronta.py PORTA REGISTRO"""
import http.server
import sys

porta, registro = int(sys.argv[1]), sys.argv[2]
PAGINA = b"""<!doctype html><meta charset=utf-8><title>prova</title><body>prova
<script>
var q = 'rtc=' + (typeof RTCPeerConnection) + '&tz=' + encodeURIComponent(Intl.DateTimeFormat().resolvedOptions().timeZone)
  + '&lang=' + encodeURIComponent(navigator.language) + '&webgl=' + (!!document.createElement('canvas').getContext('webgl'))
  + '&geo=' + ('geolocation' in navigator) + '&pl=' + encodeURIComponent(navigator.platform);
document.write('<img src="/impronta?' + q + '">');
</script></body>"""


class Gestore(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        with open(registro, "a") as f:
            f.write(self.path + "\n")
        corpo = PAGINA if not self.path.startswith("/impronta") else b"GIF89a"
        self.send_response(200)
        self.send_header("Content-Type", "text/html" if corpo is PAGINA else "image/gif")
        self.send_header("Content-Length", str(len(corpo)))
        self.end_headers()
        self.wfile.write(corpo)

    def log_message(self, *a):
        pass


http.server.ThreadingHTTPServer(("127.0.0.1", porta), Gestore).serve_forever()
