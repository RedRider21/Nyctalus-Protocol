#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
"""Trasforma i manuali (documenti/manuali/*.md, più DEPLOY e SICUREZZA) in pagine HTML
per il sito (GitHub Pages), con lo stesso stile del sito e i collegamenti fra manuali
che restano validi.   Uso:  strumenti/manuali-html.py CARTELLA_DI_USCITA
Serve il modulo Python «markdown» (python3-markdown)."""
import html
import re
import sys
from pathlib import Path

import markdown

RADICE = Path(__file__).resolve().parents[1]
SORGENTI = sorted((RADICE / "documenti" / "manuali").glob("*.md")) + \
    [RADICE / "documenti" / "DEPLOY.md", RADICE / "documenti" / "SICUREZZA.md"]
ORDINE = ["LEGGIMI", "GUIDA-RETE-DI-CASA", "SCELTE-OPERATIVE", "MANUALE-INSTALLAZIONE", "MANUALE-PC", "MANUALE-TELEFONO",
          "MANUALE-CHAT", "MANUALE-SITI", "MANUALE-SERVER", "MANUALE-GESTORE-RETE", "MANUALE-PROVE", "DEPLOY", "SICUREZZA"]

PAGINA = """<!doctype html>
<!-- SPDX-License-Identifier: AGPL-3.0-or-later
     Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
     Pagina generata da strumenti/manuali-html.py: non modificarla a mano. -->
<html lang="it">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{titolo} · Nyctalus</title>
<link rel="icon" href="../logo.svg">
<link rel="stylesheet" href="../stile.css">
<link rel="stylesheet" href="manuali.css">
</head>
<body>
<div class="barra"><div class="wrap">
  <a class="marca" href="../index.html"><img src="../logo.svg" alt="Nyctalus"> Nyctalus</a>
  <div class="spazio"></div>
  <nav class="link"><a href="index.html">Manuali</a><a href="https://github.com/RedRider21/Nyctalus-Protocol">GitHub</a></nav>
  <button class="btn-icona" id="tema" title="Tema">🌓</button>
</div></div>
<main class="wrap manuale-doc">
<nav class="indice-manuali">{indice}</nav>
<article>
{corpo}
</article>
</main>
<button class="torna-su" id="torna-su" type="button" title="Torna su" aria-label="Torna su">↑</button>
<script>
(function(){{var b=document.getElementById('torna-su');
function aggiorna(){{b.classList.toggle('visibile',window.scrollY>250);}}
window.addEventListener('scroll',aggiorna,{{passive:true}});aggiorna();
b.addEventListener('click',function(){{window.scrollTo({{top:0,behavior:'smooth'}});}});}})();
</script>
<script>
(function(){{var r=document.documentElement;try{{var t=localStorage.getItem('nyct-tema');}}catch(e){{}}
if(!t)t=matchMedia('(prefers-color-scheme: light)').matches?'light':'dark';r.setAttribute('data-theme',t);
document.getElementById('tema').onclick=function(){{var n=r.getAttribute('data-theme')==='light'?'dark':'light';
r.setAttribute('data-theme',n);try{{localStorage.setItem('nyct-tema',n);}}catch(e){{}}}};}})();
</script>
</body>
</html>
"""

CSS = """/* SPDX-License-Identifier: AGPL-3.0-or-later
   Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol. */
.wrap.manuale-doc { max-width: 1180px; }
.manuale-doc { display: grid; grid-template-columns: 230px 1fr; gap: 32px; padding-top: 28px; padding-bottom: 60px; }
.indice-manuali { position: sticky; top: 76px; align-self: start; display: flex; flex-direction: column; gap: 2px; font-size: 14px; }
.indice-manuali a { padding: 7px 10px; border-radius: 8px; text-decoration: none; color: inherit; opacity: .75; }
.indice-manuali a:hover { opacity: 1; background: rgba(127,127,127,.12); }
.indice-manuali a.qui { opacity: 1; font-weight: 600; background: color-mix(in srgb, var(--acc2) 18%, transparent); }
.manuale-doc article { min-width: 0; line-height: 1.6; }
.manuale-doc article h1 { margin-top: 0; font-size: 2rem; line-height: 1.25; letter-spacing: 0; }
.manuale-doc article h2 { font-size: 1.4rem; }
.manuale-doc article h3 { font-size: 1.12rem; }
.manuale-doc article h2 { margin-top: 2em; padding-top: .4em; border-top: 1px solid rgba(127,127,127,.25); }
.manuale-doc table { border-collapse: collapse; width: 100%; margin: 1em 0; font-size: 14.5px; display: block; overflow-x: auto; }
.manuale-doc th, .manuale-doc td { border: 1px solid rgba(127,127,127,.3); padding: 7px 10px; text-align: left; vertical-align: top; }
.manuale-doc th { background: rgba(127,127,127,.1); }
.manuale-doc pre { overflow-x: auto; padding: 12px 14px; border-radius: 10px; background: rgba(127,127,127,.12); }
.manuale-doc code { font-size: .92em; }
.manuale-doc blockquote { margin: 1em 0; padding: 8px 14px; border-left: 4px solid #e0a63a; background: rgba(224,166,58,.08); border-radius: 0 8px 8px 0; }
@media (max-width: 760px) { .manuale-doc { grid-template-columns: 1fr; } .indice-manuali { position: static; flex-direction: row; flex-wrap: wrap; } }
"""


def titolo_di(testo, nome):
    m = re.search(r"^#\s+(.+)$", testo, re.M)
    return m.group(1).replace("`", "").strip() if m else nome


def main():
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else RADICE / "manuali-html"
    out.mkdir(parents=True, exist_ok=True)
    pagine = {p.stem: p for p in SORGENTI if p.exists()}
    nomi = [n for n in ORDINE if n in pagine] + sorted(n for n in pagine if n not in ORDINE)
    titoli = {n: titolo_di(pagine[n].read_text(encoding="utf-8"), n) for n in nomi}
    for n in nomi:
        testo = pagine[n].read_text(encoding="utf-8")
        testo = re.sub(r"<!--.*?-->", "", testo, flags=re.S)              # intestazioni di licenza
        # collegamenti fra manuali: X.md → X.html (anche ../DEPLOY.md, documenti/…)
        testo = re.sub(r"\]\((?:\.\./|documenti/(?:manuali/)?)?([A-Z][A-Z0-9-]+)\.md(#[^)]*)?\)",
                       lambda m: "](%s.html%s)" % (("index" if m.group(1) == "LEGGIMI" else m.group(1)), m.group(2) or ""), testo)
        corpo = markdown.markdown(testo, extensions=["tables", "fenced_code", "sane_lists", "toc"])
        # i nomi dei manuali citati nel testo come `MANUALE-X.md` diventano collegamenti
        corpo = re.sub(r"<code>([A-Z][A-Z0-9-]+)\.md</code>",
                       lambda m: ('<a href="%s.html"><code>%s.md</code></a>' % ("index" if m.group(1) == "LEGGIMI" else m.group(1), m.group(1))
                                  if m.group(1) in pagine else m.group(0)), corpo)
        indice = "".join('<a href="%s.html"%s>%s</a>' % ("index" if k == "LEGGIMI" else k,
                                                         ' class="qui"' if k == n else "", html.escape(titoli[k]))
                         for k in nomi)
        dest = out / ("index.html" if n == "LEGGIMI" else n + ".html")
        dest.write_text(PAGINA.format(titolo=html.escape(titoli[n]), indice=indice, corpo=corpo), encoding="utf-8")
    (out / "manuali.css").write_text(CSS, encoding="utf-8")
    print("manuali HTML in %s: %d pagine" % (out, len(nomi)))


if __name__ == "__main__":
    main()
