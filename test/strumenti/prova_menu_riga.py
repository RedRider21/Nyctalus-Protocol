# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Chat del PC: il tasto destro su una FOTO deve indicare la riga di quella foto.
Elenco con tre «foto» (aree di disegno con finestra propria, come ImmagineArrotondata);
si simula il clic destro in alto nella TERZA foto. → righe «OK …» / «KO …»"""
import gi
gi.require_version("Gtk", "3.0")
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk, GLib, Gtk  # noqa: E402

from nyctalus_chat.app import riga_dell_evento  # noqa: E402

finestra = Gtk.OffscreenWindow()
elenco = Gtk.ListBox()
foto = []
for i in range(3):
    riga = Gtk.ListBoxRow()
    area = Gtk.DrawingArea()
    area.set_size_request(200, 120)
    area.add_events(Gdk.EventMask.BUTTON_PRESS_MASK)
    riga.add(area)
    elenco.add(riga)
    foto.append((riga, area))
finestra.add(elenco)
finestra.show_all()
while Gtk.events_pending():
    Gtk.main_iteration()

ev = Gdk.Event.new(Gdk.EventType.BUTTON_PRESS)
ev.button.button = 3
ev.button.window = foto[2][1].get_window()
ev.button.x, ev.button.y = 15.0, 10.0          # in alto nella terza foto (coordinate della foto)
trovata = riga_dell_evento(elenco, ev)
print(("OK " if trovata is foto[2][0] else "KO ") + "tasto destro sulla terza foto → riga %s"
      % (foto.index(next(f for f in foto if f[0] is trovata)) + 1 if trovata else None))
vecchia = elenco.get_row_at_y(10)
print(("OK " if vecchia is not foto[2][0] else "KO ")
      + "(il vecchio metodo con la coordinata avrebbe preso la riga %d: il difetto segnalato)"
      % (foto.index(next(f for f in foto if f[0] is vecchia)) + 1))
