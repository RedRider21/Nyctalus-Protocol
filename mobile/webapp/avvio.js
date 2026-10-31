// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
// Schermata d'avvio senza GTK (Windows): come quella dell'app del telefono. Parla con lo stesso
// servizio della chat (/api/stato, /api/rete); chat e rubrica si aprono in finestre proprie.
// I dati altrui finiscono SEMPRE in textContent (mai innerHTML).
"use strict";
const $ = (id) => document.getElementById(id);
const el = (tag, cls, testo) => { const e = document.createElement(tag); if (cls) e.className = cls; if (testo !== undefined) e.textContent = testo; return e; };
const SITO = "https://redrider21.github.io/Nyctalus-Protocol/";
let info = {}, pronto = false;

async function stato() {
  try { info = await (await fetch("/api/stato")).json(); } catch (e) { info = { stato: "fermo" }; }
  pronto = info.stato === "pronto";
  const s = $("stato");
  s.textContent = pronto ? "● Nyctalus è pronto" : info.stato === "avvio" ? "⏳ avvio della rete… " + (info.dettaglio || "")
    : info.stato === "errore" ? "⚠ " + (info.dettaglio || "errore") : "Nyctalus è fermo";
  s.classList.toggle("pronto", pronto);
  ["t-chat", "t-rubrica"].forEach((id) => { $(id).disabled = !pronto; });
  setTimeout(stato, pronto ? 10000 : 1500);
}

async function rete() {
  let r = {};
  try { r = await (await fetch("/api/rete")).json(); } catch (e) { /* niente */ }
  $("rete").textContent = r.condivisa ? "🔗 Rete: " + r.nome + " · " + r.impronta : "🔗 Rete: prova locale (solo questo PC)";
  return r;
}

const finestra = (url, nome) => window.open(url, nome, "popup,width=1040,height=740");

function foglio(titolo, contenuto) {
  const f = $("foglio"); f.textContent = "";
  f.append(el("h2", "", titolo), ...contenuto);
  $("velo").classList.remove("nascosto"); f.classList.remove("nascosto");
}
function chiudi() { $("velo").classList.add("nascosto"); $("foglio").classList.add("nascosto"); }
$("velo").onclick = chiudi;

async function cambiaRete(dati) {
  $("stato").textContent = "cambio rete…";
  let r = {};
  try {
    r = await (await fetch("/api/rete", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(dati) })).json();
  } catch (e) { r = { errore: "servizio non raggiungibile" }; }
  chiudi();
  if (r.errore) alert("Rete: " + r.errore);
  rete();
}

$("rete").onclick = async () => {
  const r = await rete();
  const campo = el("textarea"); campo.placeholder = "Incolla qui il codice di rete (NYCT1:…)";
  const importa = el("button", "primario", "Importa"); importa.onclick = () => { const c = campo.value.trim(); if (c) cambiaRete({ codice: c }); };
  const annulla = el("button", "", "Annulla"); annulla.onclick = chiudi;
  const riga = el("div", "riga"); riga.append(annulla, importa);
  const voci = (r.reti || []).map((x, i) => {
    const b = el("button", "voce", (x.attiva ? "● " : "○ ") + x.nome + " · " + x.impronta);
    b.onclick = () => { if (!x.attiva) cambiaRete({ usa: i }); };
    return b;
  });
  const prova = el("button", "voce", "↩ Rete di prova (solo questo PC)"); prova.onclick = () => cambiaRete({ prova: true });
  foglio("🔗 Rete", [el("p", "", "La rete è una per tutto: chat, rubrica e navigazione. Il codice te lo dà chi gestisce la rete (a casa: «nyctalus-rete casa»)."),
    campo, riga, ...(voci.length ? [el("p", "", "Reti salvate:"), ...voci] : []), prova]);
  setTimeout(() => campo.focus(), 50);
};

$("t-chat").onclick = () => finestra("/", "nyctalus-chat");
$("t-rubrica").onclick = () => finestra("/#rubrica", "nyctalus-rubrica");
$("t-info").onclick = () => window.open(SITO, "_blank");
$("t-browser").onclick = () => {
  const porta = info.socks_browser || 18733;
  const lista = el("ol");
  ["Firefox → Impostazioni → Generale → Impostazioni di rete → «Impostazioni…».",
   "«Configurazione manuale dei proxy»: Host SOCKS 127.0.0.1, porta " + porta + ", SOCKS v5.",
   "Spunta «Usa il proxy DNS con SOCKS v5» (anche i nomi passano dalla rete).",
   "Ora i siti, anche i .nyct, passano da Nyctalus. Per tornare normale: «Nessun proxy»."].forEach((t) => lista.appendChild(el("li", "", t)));
  const ok = el("button", "primario", "Ho capito"); ok.onclick = chiudi;
  const riga = el("div", "riga"); riga.append(ok);
  foglio("🌐 Navigare con Nyctalus", [el("p", "", "Su Windows il browser Nyctalus non c'è ancora: si usa Firefox impostato su Nyctalus. Protegge meno del nostro browser (non controlla da solo di essere dentro la rete): usa un profilo di Firefox dedicato."), lista, riga]);
};
$("esci").onclick = async () => {
  if (!confirm("Spegnere Nyctalus su questo PC? Chat e rubrica smettono di ricevere.")) return;
  try { await fetch("/api/esci", { method: "POST" }); } catch (e) { /* già spento */ }
  $("stato").textContent = "Nyctalus spento: puoi chiudere questa finestra.";
  setTimeout(() => window.close(), 1500);
};

stato(); rete();
