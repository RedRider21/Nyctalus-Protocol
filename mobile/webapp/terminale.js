// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
// Nyctalus · ⌨ Terminale: nyctalus-cli dentro l'app (telefono e Windows).
// Come la shell nel pannello di Termux-NexusSEC-OS: si scrive un comando, l'orchestratore lo
// esegue in Termux con le pipe e l'output scorre qui in diretta; ⏹ lo ferma. Per sicurezza
// esegue SOLO nyctalus-cli (si scrive «contatti», non «nyctalus-cli contatti»), mai una shell.
//   →  {t:"cmd", riga} · {t:"in", d} · {t:"stop"}        ←  {t:"out", d} · {t:"err", d} · {t:"fine", codice}
"use strict";
const Terminale = (() => {
  let ws = null, inCorso = false, uscita = null, campo = null, ferma = null;
  let storia = [], posto = -1, coda = [];
  const MAX = 200000;                      // caratteri tenuti a schermo
  const RAPIDI = [["contatti", "contatti"], ["io", "il mio ID"], ["file", "file"], ["file -s", "file in diretta"],
                  ["ascolta", "ascolta"], ["aiuto", "aiuto"]];
  try { storia = JSON.parse(localStorage.getItem("nyct-term-storia") || "[]").slice(-50); } catch (e) { storia = []; }

  function collega() {
    if (ws && (ws.readyState === 0 || ws.readyState === 1)) return;
    ws = new WebSocket((location.protocol === "https:" ? "wss://" : "ws://") + location.host + "/ws/terminale");
    ws.onopen = () => { while (coda.length) ws.send(coda.shift()); };
    ws.onmessage = (e) => {
      let m; try { m = JSON.parse(e.data); } catch (x) { return; }
      if (m.t === "out") scrivi(m.d);
      else if (m.t === "err") scrivi("✗ " + m.d + "\n", "term-errore");
      else if (m.t === "fine") { stato(false); if (m.codice && m.codice > 0 && m.codice !== 2) scrivi("(uscito con codice " + m.codice + ")\n", "term-tenue"); }
    };
    ws.onclose = () => { if (inCorso) { scrivi("(collegamento interrotto)\n", "term-tenue"); stato(false); } ws = null; };
  }
  function manda(obj) {
    const t = JSON.stringify(obj);
    collega();
    if (ws.readyState === 1) ws.send(t); else coda.push(t);
  }
  // l'output dei comandi: «pulisci schermo» (file -s) svuota, gli altri codici di terminale si tolgono
  function scrivi(testo, classe) {
    if (!uscita) return;
    const pezzi = String(testo).split("\x1b[2J\x1b[H");
    if (pezzi.length > 1) { uscita.textContent = ""; testo = pezzi[pezzi.length - 1]; }
    testo = String(testo).replace(/\x1b\[[0-9;?]*[A-Za-z]/g, "");
    if (!testo) return;
    const fondo = uscita.scrollHeight - uscita.scrollTop - uscita.clientHeight < 40;
    if (classe) uscita.append(el("span", classe, testo)); else uscita.append(document.createTextNode(testo));
    while (uscita.textContent.length > MAX && uscita.firstChild) uscita.firstChild.remove();
    if (fondo) uscita.scrollTop = uscita.scrollHeight;
  }
  function stato(acceso) {
    inCorso = acceso;
    if (ferma) ferma.disabled = !acceso;
    if (campo) campo.placeholder = acceso ? "comando in corso — ⏹ per fermarlo" : "scrivi un comando, es. contatti";
  }
  function esegui(riga) {
    riga = riga.trim();
    if (!riga) return;
    if (inCorso) { manda({ t: "in", d: riga }); scrivi(riga + "\n", "term-tenue"); return; }
    if (riga === "pulisci schermo" || riga === "clear" || riga === "cls") { uscita.textContent = ""; return; }
    if (storia[storia.length - 1] !== riga) storia.push(riga);
    storia = storia.slice(-50); posto = -1;
    try { localStorage.setItem("nyct-term-storia", JSON.stringify(storia)); } catch (e) { /* niente memoria: pazienza */ }
    scrivi("› " + riga + "\n", "term-comando");
    stato(true);
    // quanti caratteri ci stanno in una riga del pannello: nyctalus-cli impagina di conseguenza
    const prova = el("span", "", "x".repeat(50)); prova.style.visibility = "hidden"; uscita.append(prova);
    const colonne = Math.floor((uscita.clientWidth - 24) / (prova.getBoundingClientRect().width / 50)) || 40; prova.remove();
    manda({ t: "cmd", riga: riga === "aiuto" ? "" : riga, colonne });
    uscita.scrollTop = uscita.scrollHeight;
  }
  function richiama(passo) {
    if (!storia.length) return;
    posto = posto < 0 ? storia.length - (passo < 0 ? 1 : 0) : posto + passo;
    if (posto >= storia.length) { posto = -1; campo.value = ""; return; }
    posto = Math.max(0, posto);
    campo.value = storia[posto];
  }

  function apri(solo) {
    if (solo && !S.soloRubrica) { S.soloRubrica = true; document.body.classList.add("solo-rubrica"); if (HOST && HOST.rubrica) HOST.rubrica(true); }
    const pagina = el("div", "term-pagina");
    uscita = el("pre", "term-uscita");
    uscita.setAttribute("aria-live", "polite");
    const rapidi = el("div", "term-rapidi");
    for (const [cmd, testo] of RAPIDI) {
      const b = el("button", "term-rapido", testo); b.onclick = () => { if (!inCorso) esegui(cmd); }; rapidi.append(b);
    }
    const riga = el("form", "term-riga");
    campo = el("input", "term-campo");
    Object.assign(campo, { type: "text", autocomplete: "off", spellcheck: false, enterKeyHint: "send" });
    campo.setAttribute("autocapitalize", "off"); campo.setAttribute("autocorrect", "off");
    campo.setAttribute("aria-label", "Comando");
    campo.onkeydown = (e) => {
      if (e.key === "ArrowUp") { e.preventDefault(); richiama(-1); }
      else if (e.key === "ArrowDown") { e.preventDefault(); richiama(1); }
    };
    const su = el("button", "term-tasto", "↑"); su.type = "button"; su.title = "Comando precedente"; su.onclick = () => { richiama(-1); campo.focus(); };
    const vai = el("button", "term-tasto term-vai", "⏎"); vai.type = "submit"; vai.title = "Esegui";
    ferma = el("button", "term-tasto term-ferma", "⏹"); ferma.type = "button"; ferma.title = "Ferma il comando (Ctrl-C)";
    ferma.onclick = () => manda({ t: "stop" });
    riga.onsubmit = (e) => { e.preventDefault(); esegui(campo.value); campo.value = ""; };
    riga.append(el("span", "term-prompt", "›"), campo, su, vai, ferma);
    pagina.append(uscita, rapidi, riga);
    apriFoglio("⌨ Terminale", [pagina]);
    $("foglio").classList.add("terminale");
    $("foglio")._chiudi = () => { $("foglio").classList.remove("terminale"); if (ws) ws.close(); ws = null; uscita = null; stato(false); };
    if (solo) {
      const ind = el("button", "indietro-rubrica", "←"); ind.onclick = chiudiFoglio;
      document.querySelector("#foglio .titolo").prepend(ind);
    }
    scrivi("Nyctalus dal terminale: chat e file con i tuoi contatti, attraverso la rete.\n" +
           "Scrivi un comando (senza «nyctalus-cli» davanti) o tocca un pulsante. «aiuto» li elenca tutti.\n" +
           "Esempi:  scrivi mario ciao  ·  leggi mario  ·  invia mario ~/storage/shared/Download/film.mkv\n\n", "term-tenue");
    stato(false);
    collega();
  }
  return { apri, esegui };
})();
window.nyctApriTerminale = () => Terminale.apri(true);
if (location.hash === "#terminale") Terminale.apri(true);
