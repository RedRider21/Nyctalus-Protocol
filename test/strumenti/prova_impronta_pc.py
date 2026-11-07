# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Impronta del browser del PC in una scheda 🦇: un finto ingresso della rete (SOCKS5) risponde
con una pagina che misura quello che misura un sito che traccia (identità, lingua, ora, schermo,
WebGL, audio, canvas, WebRTC, anche in una cornice) e lo scrive nel titolo. Stampa «OK …»/«KO …»
e, per ultima, la riga «IMPRONTA {json}» da confrontare fra due avvii con fuso e lingua diversi.
uso: prova_impronta_pc.py PORTA_SOCKS"""
import json
import socket
import struct
import sys
import threading

import gi
gi.require_version("Gtk", "3.0")
gi.require_version("WebKit2", "4.0")
from gi.repository import GLib, Gtk  # noqa: E402
from nyctalus_browser import app as bapp  # noqa: E402

PS = int(sys.argv[1])
esito = lambda ok, d: print(("OK " if ok else "KO ") + d, flush=True)
intestazioni = []
PAGINA = r"""<!doctype html><meta charset=utf-8><title>attesa</title><body>
<iframe id=f srcdoc="<p>cornice</p>"></iframe>
<script>
function misura() {
  const c = document.createElement('canvas'); c.width = 200; c.height = 40;
  const g = c.getContext('2d'); g.font = '18px serif'; g.fillStyle = '#f60'; g.fillText('Nyctalus 🦇 impronta', 4, 26);
  const vuota = document.createElement('canvas'); vuota.width = 200; vuota.height = 40;
  const fw = document.getElementById('f').contentWindow;
  const r = {
    ua: navigator.userAgent, platform: navigator.platform, lingua: navigator.language, lingue: navigator.languages.join(','),
    cpu: navigator.hardwareConcurrency, tz: Intl.DateTimeFormat().resolvedOptions().timeZone,
    offset: new Date(2024, 6, 1).getTimezoneOffset(), data0: new Date(0).toString(),
    ore: new Date(2020, 0, 1, 12).getHours() + '/' + new Date(Date.UTC(2020, 0, 1, 12)).getHours(),
    locale: new Date(0).toLocaleString(), schermo: screen.width + 'x' + screen.height,
    finestra: (screen.width % 200) + ',' + (screen.height % 100), dpr: devicePixelRatio,
    webgl: !!c.getContext('webgl') || !!document.createElement('canvas').getContext('webgl'),
    audio: typeof AudioContext + '/' + typeof webkitAudioContext, rtc: typeof RTCPeerConnection,
    canvas: c.toDataURL() === vuota.toDataURL(), pixel: Array.from(g.getImageData(0, 0, 5, 5).data).some(x => x),
    nativo: Object.getOwnPropertyDescriptor(Navigator.prototype, 'platform').get.toString().includes('[native code]'),
    cornice: fw.navigator.platform + '/' + fw.Intl.DateTimeFormat().resolvedOptions().timeZone,
  };
  document.title = 'IMPRONTA ' + JSON.stringify(r);
}
setTimeout(misura, 300);
</script></body>"""


def socks():
    srv = socket.socket(); srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", PS)); srv.listen(16)
    while True:
        c, _ = srv.accept()
        threading.Thread(target=cliente, args=(c,), daemon=True).start()


def cliente(c):
    try:
        n = c.recv(2)[1]; c.recv(n); c.sendall(b"\x05\x00")
        _v, _cmd, _r, tipo = c.recv(4)
        if tipo == 3:
            c.recv(c.recv(1)[0])
        elif tipo == 1:
            c.recv(4)
        else:
            c.recv(16)
        c.recv(2)
        c.sendall(b"\x05\x00\x00\x01" + socket.inet_aton("0.0.0.0") + b"\x00\x00")
        richiesta = c.recv(65536).decode("latin-1")
        intestazioni.append(richiesta)
        corpo = PAGINA.encode()
        c.sendall(b"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\nContent-Length: %d\r\n\r\n" % len(corpo) + corpo)
    except Exception:            # noqa: BLE001
        pass
    finally:
        c.close()


threading.Thread(target=socks, daemon=True).start()
bapp.Browser._trova_binario = lambda self: None
b = bapp.Browser(None)


def apri():
    b._v = b.new_tab("http://" + "i" * 56 + ".nyct/", stealth=True)   # .nyct: già cifrato, niente «solo HTTPS»
    GLib.timeout_add(500, aspetta, 0)
    return False


def aspetta(n):
    t = b._v.get_title() or ""
    if t.startswith("IMPRONTA ") or n > 40:
        controlla(t)
        Gtk.main_quit()
        return False
    GLib.timeout_add(500, aspetta, n + 1)
    return False


def controlla(t):
    if not t.startswith("IMPRONTA "):
        esito(False, "la pagina non ha misurato l'impronta (titolo: %r)" % t)
        return
    r = json.loads(t[9:])
    lingua = [h for h in intestazioni if "accept-language" in h.lower()]
    esito("Safari" in r["ua"] and "Macintosh" in r["ua"] and r["platform"] == "MacIntel",
          "identità coerente con il motore: Safari su macOS (%s)" % r["platform"])
    esito(r["lingua"] == "en-US" and r["lingue"] == "en-US,en" and lingua and "en-US" in lingua[0] and "it" not in lingua[0].split("ccept-Language:")[-1][:20].lower(),
          "lingua uniforme en-US, anche nell'intestazione Accept-Language")
    esito(r["tz"] == "UTC" and r["offset"] == 0 and "GMT+0000" in r["data0"] and r["ore"] == "12/12",
          "ora sempre UTC, qualunque sia il fuso del PC (%s, %s)" % (r["tz"], r["data0"][:24]))
    esito(r["finestra"] == "0,0" and r["dpr"] == 1, "misure dello schermo arrotondate (%s, dpr %s)" % (r["schermo"], r["dpr"]))
    esito(r["cpu"] == 2, "processori dichiarati: 2, per tutti")
    esito(not r["webgl"], "WebGL spento (niente impronta della scheda video)")
    esito(r["audio"] == "undefined/undefined", "Web Audio spento (niente impronta audio): %s" % r["audio"])
    esito(r["rtc"] == "undefined", "WebRTC spento (non può rivelare l'IP)")
    esito(r["canvas"] and not r["pixel"], "le immagini disegnate non si leggono (canvas uguale per tutti)")
    esito(r["nativo"], "le funzioni coperte sembrano quelle originali")
    esito(r["cornice"] == "MacIntel/UTC", "anche dentro le cornici (iframe): %s" % r["cornice"])
    print("IMPRONTA " + json.dumps({k: v for k, v in r.items()}, sort_keys=True), flush=True)


GLib.timeout_add(2500, apri)
GLib.timeout_add(60000, lambda: (esito(False, "tempo scaduto"), Gtk.main_quit()))
Gtk.main()
