# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
# Pilota Chrome (senza finestra) col protocollo DevTools: naviga, aspetta, esegue JS, fotografa.
# uso: cdp.py PROFILO PASSI.json   — PASSI = [["vai", url], ["attendi", sec], ["finche", js, sec], ["js", codice], ["foto", file]]
import base64, json, os, socket, subprocess, sys, time, urllib.request

profilo, passi = sys.argv[1], json.load(open(sys.argv[2]))
chrome = subprocess.Popen(["google-chrome", "--headless=new", "--disable-gpu", "--no-first-run", "--no-default-browser-check",
                           "--remote-debugging-port=19333", "--user-data-dir=" + profilo, "--window-size=412,915",
                           "--force-device-scale-factor=2", "--hide-scrollbars",
                           # microfono/fotocamera FINTI (segnale di prova): mai quelli veri
                           "--use-fake-device-for-media-stream", "--use-fake-ui-for-media-stream",
                           "--autoplay-policy=no-user-gesture-required", "about:blank"],
                          stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    for _ in range(50):
        try:
            pagine = json.load(urllib.request.urlopen("http://127.0.0.1:19333/json/list")); break
        except Exception:
            time.sleep(0.2)
    ws_url = [p for p in pagine if p["type"] == "page"][0]["webSocketDebuggerUrl"]
    host, path = ws_url[5:].split("/", 1)
    s = socket.create_connection(tuple(host.split(":")[0:1]) + (int(host.split(":")[1]),))
    s.sendall(("GET /%s HTTP/1.1\r\nHost: %s\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
               "Sec-WebSocket-Key: %s\r\nSec-WebSocket-Version: 13\r\n\r\n" % (path, host, base64.b64encode(os.urandom(16)).decode())).encode())
    buf = b""
    while b"\r\n\r\n" not in buf: buf += s.recv(4096)
    buf = buf.split(b"\r\n\r\n", 1)[1]

    def manda(obj):
        d = json.dumps(obj).encode(); m = os.urandom(4); n = len(d)
        h = bytes([0x81]) + (bytes([0x80 | n]) if n < 126 else bytes([0x80 | 126]) + n.to_bytes(2, "big") if n < 65536 else bytes([0x80 | 127]) + n.to_bytes(8, "big"))
        s.sendall(h + m + bytes(b ^ m[i % 4] for i, b in enumerate(d)))

    def ricevi():
        global buf
        def leggi(k):
            global buf
            while len(buf) < k: buf += s.recv(65536)
            out, buf = buf[:k], buf[k:]; return out
        b0, b1 = leggi(2); n = b1 & 0x7F
        if n == 126: n = int.from_bytes(leggi(2), "big")
        elif n == 127: n = int.from_bytes(leggi(8), "big")
        return json.loads(leggi(n))

    nid = [0]
    def chiama(metodo, **p):
        nid[0] += 1; manda({"id": nid[0], "method": metodo, "params": p})
        while True:
            r = ricevi()
            if r.get("id") == nid[0]: return r.get("result", {})

    chiama("Page.enable")
    for passo in passi:
        if passo[0] == "vai": chiama("Page.navigate", url=passo[1])
        elif passo[0] == "attendi": time.sleep(passo[1])
        elif passo[0] == "finche":      # ["finche", espressione JS, secondi max]: attende che diventi vera
            fine = time.time() + passo[2]
            while time.time() < fine:
                r = chiama("Runtime.evaluate", expression=passo[1], returnByValue=True)
                if r.get("result", {}).get("value"): break
                time.sleep(0.5)
        elif passo[0] == "js":
            r = chiama("Runtime.evaluate", expression=passo[1], awaitPromise=True, returnByValue=True)
            v = r.get("result", {}).get("value")
            if v is not None: print("js →", v)
        elif passo[0] == "foto":
            open(passo[1], "wb").write(base64.b64decode(chiama("Page.captureScreenshot", format="png")["data"]))
            print("foto →", os.path.basename(passo[1]))
finally:
    chrome.terminate()
