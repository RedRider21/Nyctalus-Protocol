# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Il programma File del PC (finestra GTK) sopra il motore di Bob; Alice (motore guidato da qui)
gli manda due file: uno sotto soglia (parte da solo), uno sopra (si tocca «Accetta» nella
finestra). Stampa «OK …» / «KO …». uso: prova_file_pc.py PORTA_A SEGRETO_A IND_B IND_A CARTELLA FOTO
(il collegamento a Bob arriva da NYCTALUS_CHAT_PORT / NYCTALUS_CHAT_TOKEN)."""
import hashlib
import os
import socket
import sys
import threading
import time

import gi
gi.require_version("Gtk", "3.0")
from gi.repository import GLib, Gtk  # noqa: E402
from nyctalus_file import app as fileapp  # noqa: E402

pa, sa, ind_b, ind_a, cartella, foto = sys.argv[1:7]
esito = lambda ok, d: print(("OK " if ok else "KO ") + d, flush=True)
alice = socket.create_connection(("127.0.0.1", int(pa)))
risposte = []


def leggi():
    buf = b""
    while True:
        d = alice.recv(65536)
        if not d:
            return
        buf += d
        while b"\n" in buf:
            r, buf = buf.split(b"\n", 1)
            risposte.append(r.decode(errors="replace"))


threading.Thread(target=leggi, daemon=True).start()
alice.sendall(("AUTH %s\nSUB\nADD %s %s\n" % (sa, ind_b, b"Bob".hex())).encode())


def crea(nome, mb):
    p = os.path.join(cartella, nome)
    with open(p, "wb") as f:
        f.write(os.urandom(int(mb * 1024 * 1024)))
    return p


def sha(p):
    return hashlib.sha256(open(p, "rb").read()).hexdigest()


app = fileapp.FileApp()
app.resize(760, 520)
f1, f2 = crea("relazione.pdf", 1.5), crea("filmato.mp4", 3)
stato = {}


def righe(verso):
    return [r for r in app.liste[verso].get_children() if isinstance(r, fileapp.Riga)]


def passo1():
    # Bob (la finestra) aggiunge Alice ai contatti e mette la soglia a 2 MB
    app.cmd("ADD %s %s" % (ind_a, b"Alice".hex()))
    app.cmd("XCONF soglia %d" % (2 * 1024 * 1024))
    app.cmd("LIST")
    GLib.timeout_add(1500, passo2)
    return False


def passo2():
    for f, m in ((f1, "application/pdf"), (f2, "video/mp4")):
        alice.sendall(("XSEND %s %s %s\n" % (ind_b, f.encode().hex(), m)).encode())
    stato["t0"] = time.time()
    GLib.timeout_add(500, passo3)
    return False


def passo3():
    rr = {r.d["nome"]: r for r in righe("in")}
    pdf, video = rr.get("relazione.pdf"), rr.get("filmato.mp4")
    if not (pdf and pdf.d["stato"] == "fatto" and video and video.d["stato"] == "chiede"):
        if time.time() - stato["t0"] < 120:
            return True
        esito(False, "righe nella finestra: %s" % {k: v.d["stato"] for k, v in rr.items()})
        return fine()
    esito(True, "la finestra mostra i due file in arrivo: il PDF (sotto soglia) già «Arrivato», il video chiede «Accetta»")
    esito("Alice" in pdf.sotto.get_text() and "1,5 MB" in pdf.sotto.get_text(), "con chi lo manda e la misura: «%s»" % pdf.sotto.get_text())
    esito(sha(pdf.d.get("percorso", "")) == sha(f1) if pdf.d.get("percorso") else False, "il PDF arrivato è identico")
    bott = [b.get_label() for b in video.azioni.get_children()]
    esito(bott == ["Rifiuta", "Accetta"], "per il video: pulsanti «Rifiuta» e «Accetta»")
    app.get_window().process_updates(True) if app.get_window() else None
    GLib.timeout_add(300, lambda: (foto_finestra("-chiede"), False)[1])
    GLib.timeout_add(800, passo4)
    return False


def passo4():
    video = {r.d["nome"]: r for r in righe("in")}["filmato.mp4"]
    [b for b in video.azioni.get_children() if b.get_label() == "Accetta"][0].clicked()
    stato["t1"] = time.time()
    GLib.timeout_add(500, passo5)
    return False


def passo5():
    video = {r.d["nome"]: r for r in righe("in")}["filmato.mp4"]
    if video.d["stato"] != "fatto":
        if time.time() - stato["t1"] < 120:
            return True
        esito(False, "video dopo Accetta: %s" % video.d["stato"])
        return fine()
    esito(sha(video.d.get("percorso", "")) == sha(f2) if video.d.get("percorso") else False,
          "toccato «Accetta»: il video arriva identico e la riga diventa «Arrivato» con «Apri» e «Mostra nella cartella»")
    bott = video.azioni.get_children()
    esito([b.get_label() for b in bott if b.get_label()] == ["Mostra nella cartella", "Apri"]
          and any(b.get_tooltip_text() and "Togli" in b.get_tooltip_text() for b in bott),
          "azioni a file arrivato: «Mostra nella cartella», «Apri» e ✕ «Togli dall'elenco»")
    GLib.timeout_add(300, lambda: (foto_finestra(""), False)[1])
    GLib.timeout_add(900, passo6)
    return False


def passo6():
    # il video arrivato viene cancellato dalla cartella: tornando sulla finestra la riga lo dice
    rr = {r.d["nome"]: r for r in righe("in")}
    stato["pdf"] = rr["relazione.pdf"].d["percorso"]          # la COPIA ricevuta da Bob
    os.remove(rr["filmato.mp4"].d["percorso"])
    app.cmd("XLIST")
    GLib.timeout_add(1200, passo7)
    return False


def passo7():
    rr = {r.d["nome"]: r for r in righe("in")}
    v = rr["filmato.mp4"]
    esito(v.d["stato"] == "eliminato" and v.stato.get_text() == "Eliminato dalla cartella"
          and not any(b.get_label() == "Apri" for b in v.azioni.get_children()),
          "file cancellato dalla cartella: la riga dice «Eliminato dalla cartella», niente più «Apri»")
    x = [b for b in v.azioni.get_children() if b.get_tooltip_text() and "Togli" in b.get_tooltip_text()][0]
    x.clicked()
    GLib.timeout_add(1200, passo8)
    return False


def passo8():
    esito("filmato.mp4" not in {r.d["nome"] for r in righe("in")}, "✕ «Togli dall'elenco»: la riga sparisce")
    app.cmd("XPULISCI")
    GLib.timeout_add(1200, passo9)
    return False


def passo9():
    esito(not righe("in"), "«Pulisci elenco»: spariscono tutti i trasferimenti conclusi")
    esito(os.path.exists(stato["pdf"]) and sha(stato["pdf"]) == sha(f1), "togliere dall'elenco non cancella i file ricevuti (il PDF arrivato a Bob c'è ancora)")
    fine()
    return False


def foto_finestra(suff):
    w = app.get_window()
    pb = __import__("gi.repository.Gdk", fromlist=["Gdk"]).pixbuf_get_from_window(w, 0, 0, w.get_width(), w.get_height())
    if pb:
        pb.savev(foto.replace(".png", suff + ".png"), "png", [], [])


def fine():
    sys.stdout.flush()
    os._exit(0)


GLib.timeout_add(1500, passo1)
GLib.timeout_add(300000, lambda: (esito(False, "tempo scaduto"), fine()))
Gtk.main()
