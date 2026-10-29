// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
//
// Web-app della chat .nyct per il telefono. Parla con il motore attraverso
// l'orchestratore (WebSocket /ws) con lo STESSO protocollo a righe della
// finestra GTK del PC:
//   →  WHOAMI · LIST · SUB · ADD · HIST · SEND · SENDFILE · CLEAR · DEL · DELMSG
//   ←  ME · C · L · ST · MSG · OUT · P · ENDH · FILE · PROG · FERR · NEW · OK · ERR
// I dati altrui finiscono SEMPRE in textContent (mai innerHTML).
"use strict";

// ---------- utilità ---------------------------------------------------------------
const $ = (id) => document.getElementById(id);
const enc = new TextEncoder(), dec = new TextDecoder();
const inHex = (s) => Array.from(enc.encode(s), (b) => b.toString(16).padStart(2, "0")).join("");
const daHex = (h) => {
  try { return dec.decode(new Uint8Array((h.match(/../g) || []).map((x) => parseInt(x, 16)))); }
  catch (e) { return ""; }
};
const el = (tag, cls, testo) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (testo !== undefined) e.textContent = testo;
  return e;
};
const COLORI_AVATAR = ["#5aa9ff", "#7f6cff", "#38c6a4", "#e0794b", "#d45b8e", "#c9a227", "#4bb3e0", "#8a86ff"];
const coloreAvatar = (addr) => COLORI_AVATAR[enc.encode(addr).reduce((a, b) => a + b, 0) % COLORI_AVATAR.length];
const ora = (ts) => new Date(ts * 1000).toLocaleTimeString("it-IT", { hour: "2-digit", minute: "2-digit" });
const giorno = (ts) => {
  const d = new Date(ts * 1000), oggi = new Date();
  if (d.toDateString() === oggi.toDateString()) return "Oggi";
  const ieri = new Date(oggi); ieri.setDate(oggi.getDate() - 1);
  if (d.toDateString() === ieri.toDateString()) return "Ieri";
  return d.toLocaleDateString("it-IT", { day: "numeric", month: "long", year: d.getFullYear() === oggi.getFullYear() ? undefined : "numeric" });
};
const oraOGiorno = (ts) => {
  const d = new Date(ts * 1000), oggi = new Date();
  if (d.toDateString() === oggi.toDateString()) return ora(ts);
  if ((oggi - d) < 6 * 86400000) return d.toLocaleDateString("it-IT", { weekday: "short" });
  return d.toLocaleDateString("it-IT", { day: "2-digit", month: "2-digit", year: "2-digit" });
};
const dimensione = (n) => {
  n = Number(n); const u = ["B", "KB", "MB", "GB"]; let i = 0;
  while (n >= 1024 && i < 3) { n /= 1024; i++; }
  return i ? n.toFixed(1) + " " + u[i] : n + " B";
};
const durata = (s) => { s = Math.round(s || 0); const m = Math.floor(s / 60); return m + ":" + String(s % 60).padStart(2, "0"); };
const ANTEPRIMA_FOTO = ["image/jpeg", "image/png", "image/gif", "image/webp", "image/bmp"];
const famiglia = (mime, nome) => {
  const ext = (nome.split(".").pop() || "").toLowerCase();
  if (mime === "application/pdf" || ext === "pdf") return ["pdf", "#e05a4f"];
  if (mime.startsWith("audio/")) return ["audio", "#9a6ef0"];
  if (mime.startsWith("video/")) return ["video", "#e05a8a"];
  if (mime.startsWith("image/")) return ["immagine", "#3fa7c9"];
  if (["zip", "7z", "rar", "tar", "gz", "xz"].includes(ext)) return ["archivio", "#e09a3e"];
  if (["xls", "xlsx", "ods", "csv"].includes(ext)) return ["foglio", "#43a565"];
  if (["doc", "docx", "odt", "rtf", "txt", "md"].includes(ext) || mime.startsWith("text/")) return ["documento", "#4f86e0"];
  return ["altro", "#6b7797"];
};
const estensione = (nome) => { const p = nome.split("."); return p.length > 1 ? p.pop().toUpperCase().slice(0, 4) : "FILE"; };
// si aprono con le app del telefono solo i tipi innocui (mai eseguibili/script/APK)
const apribile = (mime) => (/^(image|video|audio)\//.test(mime) && mime !== "image/svg+xml") || mime === "application/pdf" || mime === "text/plain";
const urlFile = (p) => "/api/file?p=" + inHex(p);
function toast(testo) {
  const t = el("div", "toast", testo); document.body.appendChild(t); setTimeout(() => t.remove(), 2400);
}

// ---------- cache sul telefono (IndexedDB) -------------------------------------------
// Elenco e ultime conversazioni compaiono SUBITO all'apertura, anche mentre la rete
// sta ancora salendo; poi si allineano con il motore (che resta la fonte di verità).
const MAX_RIGHE_CACHE = 300;
const DB = (() => {
  let db = null;
  const apri = () => db || (db = new Promise((ok) => {
    try {
      const r = indexedDB.open("nyctalus", 1);
      r.onupgradeneeded = () => { r.result.createObjectStore("contatti", { keyPath: "addr" }); r.result.createObjectStore("messaggi", { keyPath: "addr" }); };
      r.onsuccess = () => ok(r.result); r.onerror = () => ok(null);
    } catch (e) { ok(null); }
  }));
  const tx = async (store, modo, fn) => {
    const d = await apri(); if (!d) return null;
    return new Promise((ok) => {
      const t = d.transaction(store, modo), st = t.objectStore(store), r = fn(st);
      t.oncomplete = () => ok(r && r.result); t.onerror = () => ok(null);
    });
  };
  return {
    tutti: (store) => tx(store, "readonly", (st) => st.getAll()),
    leggi: (store, k) => tx(store, "readonly", (st) => st.get(k)),
    scrivi: (store, v) => tx(store, "readwrite", (st) => st.put(v)),
    togli: (store, k) => tx(store, "readwrite", (st) => st.delete(k)),
  };
})();
async function cacheAggiungiRiga(addr, grezza) {
  const v = (await DB.leggi("messaggi", addr)) || { addr, righe: [] };
  v.righe.push(grezza); v.righe = v.righe.slice(-MAX_RIGHE_CACHE);
  DB.scrivi("messaggi", v);
}
async function cacheTogliRiga(addr, grezza) {
  const v = await DB.leggi("messaggi", addr); if (!v) return;
  const i = v.righe.lastIndexOf(grezza); if (i >= 0) { v.righe.splice(i, 1); DB.scrivi("messaggi", v); }
}
const cacheContatto = (addr) => DB.scrivi("contatti", { addr, nome: S.contatti.get(addr) || "", ultimo: S.ultimo.get(addr) || null });

// ---------- stato ------------------------------------------------------------------
const salvato = (k, def) => { try { return JSON.parse(localStorage.getItem("nyct." + k)) ?? def; } catch (e) { return def; } };
const salva = (k, v) => { try { localStorage.setItem("nyct." + k, JSON.stringify(v)); } catch (e) { /* niente */ } };
const S = {
  me: "", contatti: new Map(), online: new Set(), ultimo: new Map(), attivo: null,
  nonLetti: salvato("nonLetti", {}), fissati: salvato("fissati", []),
  tema: salvato("tema", "notte"), sfondo: salvato("sfondo", "pipistrelli"), exif: salvato("exif", true),
  trasferimenti: new Map(), ultimoGiorno: null, ws: null, filtro: "",
  storia: null, cacheChat: null, inFondo: true,
};

// ---------- connessione al motore ------------------------------------------------------
function connetti() {
  const ws = new WebSocket("ws://" + location.host + "/ws");
  S.ws = ws;
  ws.onopen = () => { manda("WHOAMI"); manda("LIST"); manda("SUB"); };
  ws.onmessage = (ev) => ev.data.split("\n").forEach((r) => r && evento(r));
  ws.onclose = () => { S.ws = null; setTimeout(controllaServizi, 1500); };
}
const manda = (riga) => { if (S.ws && S.ws.readyState === 1) S.ws.send(riga); };

async function controllaServizi() {
  let info;
  try { info = await (await fetch("/api/stato")).json(); } catch (e) { return setTimeout(controllaServizi, 2000); }
  const barra = $("stato-rete"), testo = $("stato-testo"), btn = $("btn-avvia");
  btn.classList.add("nascosto");
  if (info.stato === "pronto") {
    barra.classList.toggle("nascosto", info.modalita !== "locale");
    testo.textContent = "Rete di prova LOCALE: per parlare con altre persone configura la rete condivisa";
  } else if (info.stato === "avvio") {
    barra.classList.remove("nascosto"); testo.textContent = "⏳ Avvio della rete… " + (info.dettaglio || "");
  } else {
    barra.classList.remove("nascosto"); btn.classList.remove("nascosto");
    testo.textContent = info.stato === "errore" ? "⚠ " + info.dettaglio : "Nyctalus è fermo";
  }
  if (info.indirizzo && !S.ws) connetti();
  if (info.stato !== "pronto") setTimeout(controllaServizi, 1500);
}
const post = (url) => fetch(url, { method: "POST" }).then((r) => r.json()).catch(() => ({}));

// ---------- eventi dal motore ----------------------------------------------------------
function evento(riga) {
  const p = riga.split(" "), t = p[0];
  if (t === "ME") { S.me = p[1]; $("mio-id-valore").textContent = S.me; }
  else if (t === "C") { aggiornaContatto(p[1], p[2] ? daHex(p[2]) : p[1].slice(0, 10)); cacheContatto(p[1]); }
  else if (t === "L") { anteprima(p[1], p.slice(2).join(" ")); cacheContatto(p[1]); }
  else if (t === "ST") { p[2] === "1" ? S.online.add(p[1]) : S.online.delete(p[1]); disegnaElenco(); if (p[1] === S.attivo) testata(); }
  else if (t === "MSG" || t === "OUT") {
    const dir = t === "MSG" ? "in" : "out", grezza = dir + " " + p[2] + " " + p[3];
    anteprima(p[1], grezza, dir === "in"); cacheAggiungiRiga(p[1], grezza); cacheContatto(p[1]);
    if (p[1] === S.attivo) bollaTesto(dir, daHex(p[3]), +p[2], grezza);
  }
  else if (t === "P") { if (S.storia) S.storia.push(riga.slice(2)); else mostraRiga(riga.slice(2)); }
  else if (t === "ENDH") fineStoria();
  else if (t === "FILE" && p.length >= 9) {
    const grezza = [p[2] === "out" ? "fout" : "fin", ...p.slice(3, 9)].join(" ");
    anteprima(p[1], grezza, p[2] === "in"); cacheAggiungiRiga(p[1], grezza); cacheContatto(p[1]);
    if (p[1] === S.attivo) bollaFile(p[2], +p[3], p[4], +p[5], p[6], daHex(p[7]), daHex(p[8]), grezza);
  }
  else if (t === "PROG" && p[1] === S.attivo) avanzamento(p[2], p[3], +p[4], +p[5], p[6] || "", p[7] ? daHex(p[7]) : "");
  else if (t === "FERR" && p[1] === S.attivo) { togliTrasferimento(p[2]); avviso("⚠ " + daHex(p[3])); }
  else if (t === "NEW" && !S.contatti.has(p[1])) dialogoAggiungi(p[1], "Qualcuno ti ha scritto da un indirizzo che non è in rubrica. Aggiungilo per rispondere.");
  else if (t === "ERR") toast(riga.slice(4));
  else if (t === "ENDC") {   // collegamento diretto: #chat=<indirizzo> apre quella conversazione
    const m = location.hash.match(/chat=([a-z2-7]{56}\.nyct)/);
    if (m && S.contatti.has(m[1]) && !S.attivo) apriChat(m[1]);
  }
}

// ---------- elenco chat ------------------------------------------------------------------
function aggiornaContatto(addr, nome) { S.contatti.set(addr, nome); disegnaElenco(); if (addr === S.attivo) testata(); }

function testoAnteprima(grezza) {
  const p = grezza.split(" ");
  if ((p[0] === "in" || p[0] === "out") && p.length >= 3) return [+p[1], (p[0] === "out" ? "Tu: " : "") + daHex(p[2]).replace(/\n/g, " ")];
  if ((p[0] === "fin" || p[0] === "fout") && p.length >= 6) {
    const mime = p[4], nome = daHex(p[5]);
    const cosa = ANTEPRIMA_FOTO.includes(mime) ? "🖼 Foto" : mime.startsWith("video/") ? "🎬 Video" : mime.startsWith("audio/") ? "🎵 " + nome : "📎 " + nome;
    return [+p[1], (p[0] === "fout" ? "Tu: " : "") + cosa];
  }
  return [null, ""];
}

function anteprima(addr, grezza, nuovo) {
  const [ts, testo] = testoAnteprima(grezza);
  if (ts === null) return;
  S.ultimo.set(addr, [ts, testo]);
  if (nuovo && addr !== S.attivo) { S.nonLetti[addr] = (S.nonLetti[addr] || 0) + 1; salva("nonLetti", S.nonLetti); }
  disegnaElenco();
}

function avatar(addr, nome, piccolo) {
  const a = el("div", "avatar" + (piccolo ? " piccolo" : "") + (S.online.has(addr) ? " online" : ""), (nome || "?").trim().charAt(0).toUpperCase());
  a.style.background = coloreAvatar(addr);
  return a;
}

function disegnaElenco() {
  const box = $("elenco"); box.textContent = "";
  const voci = [...S.contatti.entries()].filter(([a, n]) => !S.filtro || n.toLowerCase().includes(S.filtro) || a.includes(S.filtro));
  voci.sort(([a], [b]) => {
    const fa = S.fissati.includes(a), fb = S.fissati.includes(b);
    if (fa !== fb) return fa ? -1 : 1;
    return ((S.ultimo.get(b) || [0])[0]) - ((S.ultimo.get(a) || [0])[0]);
  });
  if (!voci.length) {
    const v = el("div", "vuoto");
    v.append(el("span", "pipistrello", "🦇"), el("div", "", S.contatti.size ? "Nessun risultato" : "Nessun contatto. Tocca ✏️ e incolla il Contact ID di un amico."));
    return box.appendChild(v);
  }
  for (const [addr, nome] of voci) {
    const r = el("div", "riga" + (addr === S.attivo ? " attiva" : ""));
    const centro = el("div", "centro"), alto = el("div", "alto"), basso = el("div", "basso");
    const [ts, testo] = S.ultimo.get(addr) || [null, S.online.has(addr) ? "in linea" : "non in linea"];
    alto.append(el("span", "nome", nome), ...(S.fissati.includes(addr) ? [el("span", "", "📌")] : []), el("span", "ora", ts ? oraOGiorno(ts) : ""));
    basso.append(el("span", "anteprima", testo));
    if (S.nonLetti[addr]) basso.append(el("span", "badge", S.nonLetti[addr] > 99 ? "99+" : String(S.nonLetti[addr])));
    centro.append(alto, basso);
    r.append(avatar(addr, nome), centro);
    r.onclick = () => apriChat(addr);
    r.oncontextmenu = (e) => { e.preventDefault(); menuContatto(addr); };
    box.appendChild(r);
  }
}

// ---------- conversazione ------------------------------------------------------------
function apriChat(addr, daStoria) {
  S.attivo = addr; S.ultimoGiorno = null; S.trasferimenti.clear();
  if (S.nonLetti[addr]) { delete S.nonLetti[addr]; salva("nonLetti", S.nonLetti); }
  ["testa-chat", "messaggi", "compositore"].forEach((id) => $(id).classList.remove("nascosto"));
  $("benvenuto").classList.add("nascosto");
  $("messaggi").textContent = ""; S.inFondo = true;
  testata(); disegnaElenco();
  $("app").classList.add("in-chat");
  if (!daStoria) history.pushState({ chat: addr }, "");
  S.storia = []; S.cacheChat = null;
  DB.leggi("messaggi", addr).then((v) => {          // subito dalla cache…
    if (S.attivo !== addr || !v || !S.storia) return;
    S.cacheChat = v.righe; v.righe.forEach(mostraRiga);
  });
  manda("HIST " + addr);                              // …poi la cronologia vera dal motore
}

function mostraRiga(grezza) {
  const p = grezza.split(" ");
  if ((p[0] === "fin" || p[0] === "fout") && p.length >= 7) bollaFile(p[0] === "fout" ? "out" : "in", +p[1], p[2], +p[3], p[4], daHex(p[5]), daHex(p[6]), grezza);
  else if ((p[0] === "in" || p[0] === "out") && p.length >= 3) bollaTesto(p[0], daHex(p[2]), +p[1], grezza);
}

function fineStoria() {
  const righe = S.storia || []; S.storia = null;
  if (!S.attivo) return;
  // si ridisegna solo se la cache era diversa (niente sfarfallio)
  if (!S.cacheChat || JSON.stringify(S.cacheChat) !== JSON.stringify(righe.slice(-MAX_RIGHE_CACHE))) {
    $("messaggi").textContent = ""; S.ultimoGiorno = null; S.inFondo = true;
    righe.forEach(mostraRiga);
  }
  DB.scrivi("messaggi", { addr: S.attivo, righe: righe.slice(-MAX_RIGHE_CACHE) });
  restaInFondo(true);
}

// la chat resta "attaccata" in fondo come in Telegram, anche quando foto e video finiscono di caricarsi
function restaInFondo(forza) {
  const m = $("messaggi");
  if (forza || S.inFondo) requestAnimationFrame(() => { m.scrollTop = m.scrollHeight; });
}
$("messaggi").addEventListener("scroll", () => {
  const m = $("messaggi"); S.inFondo = m.scrollHeight - m.scrollTop - m.clientHeight < 120;
});
function chiudiChat() {
  S.attivo = null; $("app").classList.remove("in-chat");
  ["testa-chat", "messaggi", "compositore"].forEach((id) => $(id).classList.add("nascosto"));
  $("benvenuto").classList.remove("nascosto"); disegnaElenco();
}
window.onpopstate = () => { if (S.attivo) chiudiChat(); };

function testata() {
  if (!S.attivo) return;
  const nome = S.contatti.get(S.attivo) || S.attivo.slice(0, 10), on = S.online.has(S.attivo);
  $("testa-nome").textContent = nome;
  $("testa-stato").textContent = on ? "in linea" : "non in linea";
  $("testa-stato").classList.toggle("on", on);
  const slot = $("testa-avatar"); slot.textContent = ""; slot.appendChild(avatar(S.attivo, nome, true));
}

function aggiungiMessaggio(dir, ts, bolla, grezza, dati) {
  const box = $("messaggi");
  if (ts) {
    const g = giorno(ts);
    if (g !== S.ultimoGiorno) { S.ultimoGiorno = g; const d = el("div", "giorno"); d.appendChild(el("span", "", g)); box.appendChild(d); }
  }
  const riga = el("div", "msg " + dir);
  riga.appendChild(bolla);
  if (grezza) {
    riga.oncontextmenu = (e) => { e.preventDefault(); menuMessaggio(riga, grezza, dati); };
  }
  box.appendChild(riga);
  restaInFondo();
  return riga;
}

function bollaTesto(dir, testo, ts, grezza) {
  const b = el("div", "bolla");
  b.append(el("span", "testo", testo), el("span", "ora", ora(ts)));
  aggiungiMessaggio(dir, ts, b, grezza, { testo });
}

function bollaFile(dir, ts, id, dim, mime, nome, percorso, grezza) {
  togliTrasferimento(id);
  const b = el("div", "bolla");
  if (ANTEPRIMA_FOTO.includes(mime)) {
    b.classList.add("media");
    const img = el("img"); img.src = urlFile(percorso); img.alt = nome; img.loading = "lazy";
    img.onclick = () => apriFile(percorso, mime);
    img.onload = () => restaInFondo();
    img.onerror = () => { b.classList.remove("media"); b.textContent = ""; b.append(schedaFile(nome, mime, dim, percorso, true)); };
    b.append(img, el("span", "ora", ora(ts)));
  } else if (mime.startsWith("video/")) {
    b.classList.add("media");
    const v = el("video"); v.src = urlFile(percorso); v.preload = "metadata"; v.playsInline = true;
    v.poster = "/api/miniatura?p=" + inHex(percorso);
    const d = el("span", "durata", dimensione(dim)), o = el("span", "ora", ora(ts));
    const play = el("div", "play"); play.appendChild(el("span", "", "▶"));
    v.onloadedmetadata = () => { if (isFinite(v.duration)) d.textContent = durata(v.duration); restaInFondo(); };
    b.onclick = () => { if (v.controls) return; v.controls = true; play.remove(); d.remove(); o.remove(); v.play(); };
    b.append(v, play, d, o);
  } else {
    b.append(schedaFile(nome, mime, dim, percorso), el("span", "ora", ora(ts)));
  }
  aggiungiMessaggio(dir, ts, b, grezza, { file: percorso, mime, nome });
}

function schedaFile(nome, mime, dim, percorso, mancante) {
  const [fam, colore] = famiglia(mime, nome);
  const s = el("div", "file");
  const c = el("button", "cerchio", fam === "audio" ? "▶" : estensione(nome)); c.style.background = colore;
  const info = el("div"); const dett = el("div", "dett", mancante ? "file non più disponibile" : dimensione(dim) + (fam === "audio" ? "" : " · " + estensione(nome)));
  info.append(el("div", "nomef", nome), dett);
  s.append(c, info);
  if (fam === "audio" && !mancante) {
    const a = new Audio(); a.preload = "metadata"; a.src = urlFile(percorso);
    a.onloadedmetadata = () => { dett.textContent = durata(a.duration) + " · " + dimensione(dim); };
    a.ontimeupdate = () => { dett.textContent = durata(a.currentTime) + " / " + durata(a.duration); };
    a.onended = a.onpause = () => { c.textContent = "▶"; };
    a.onplay = () => { c.textContent = "⏸"; document.querySelectorAll("audio").forEach((x) => x !== a && x.pause()); };
    document.body.appendChild(a); a.classList.add("nascosto");
    c.onclick = () => (a.paused ? a.play() : a.pause());
  } else if (!mancante) {
    c.onclick = () => apriFile(percorso, mime);
  }
  return s;
}

function cerchioAvanzamento(colore, testo) {
  const c = el("div", "cerchio", testo); c.style.background = colore;
  c.insertAdjacentHTML("beforeend", '<svg viewBox="0 0 48 48"><circle cx="24" cy="24" r="18" fill="none" stroke="rgba(255,255,255,.3)" stroke-width="3"/><circle class="arco" cx="24" cy="24" r="18" fill="none" stroke="#fff" stroke-width="3" stroke-dasharray="113.1" stroke-dashoffset="113.1" stroke-linecap="round"/></svg>');
  return c;
}
function avanzamento(dir, id, fatti, totale, mime, nome) {
  let t = S.trasferimenti.get(id);
  if (!t) {
    const [, colore] = famiglia(mime, nome || "file");
    const b = el("div", "bolla"), s = el("div", "file"), c = cerchioAvanzamento(colore, ""), info = el("div");
    const dett = el("div", "dett");
    info.append(el("div", "nomef", nome || "…"), dett); s.append(c, info); b.append(s);
    t = { riga: aggiungiMessaggio(dir, null, b), arco: c.querySelector(".arco"), dett };
    S.trasferimenti.set(id, t);
  }
  const f = totale ? Math.min(1, fatti / totale) : 1;
  t.arco.setAttribute("stroke-dashoffset", String(113.1 * (1 - Math.max(0.02, f))));
  t.dett.textContent = (dir === "out" ? "Invio " : "Ricezione ") + dimensione(fatti) + " / " + dimensione(totale);
}
function togliTrasferimento(id) { const t = S.trasferimenti.get(id); if (t) { t.riga.remove(); S.trasferimenti.delete(id); } }
function avviso(testo) { const a = el("div", "avviso", testo); $("messaggi").appendChild(a); }

async function apriFile(percorso, mime) {
  if (!apribile(mime)) return toast("Per sicurezza questo tipo di file non si apre da qui");
  const r = await post("/api/apri?p=" + inHex(percorso));
  if (!r.ok) window.open(urlFile(percorso), "_blank");
}

// ---------- invio ----------------------------------------------------------------------
const campo = $("testo");
campo.addEventListener("input", () => {
  campo.style.height = "auto"; campo.style.height = Math.min(140, campo.scrollHeight) + "px";
  $("invia").disabled = !campo.value.trim();
});
campo.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !e.shiftKey && !("ontouchstart" in window)) { e.preventDefault(); inviaTesto(); }
});
$("invia").onclick = inviaTesto;
function inviaTesto() {
  const t = campo.value.trim(); if (!t || !S.attivo) return;
  manda("SEND " + S.attivo + " " + inHex(t)); campo.value = ""; campo.dispatchEvent(new Event("input"));
}

// graffetta: selettore come Telegram per Android — griglia delle foto/video recenti
// (prima casella = fotocamera), selezione numerata, schede in basso, «Invia (N)».
$("btn-allega").onclick = apriGalleria;

async function apriGalleria() {
  if (!S.attivo) return;
  const sel = [];                                   // voci selezionate, nell'ordine di tocco
  const f = $("foglio"); f.textContent = ""; f.classList.add("galleria");
  const testa = el("div", "testa-gal"), titolo = el("span", "", "Galleria"), conta = el("span", "conta", "");
  testa.append(titolo, conta);
  const griglia = el("div", "griglia-gal");
  const cam = el("button", "tile tile-cam"); cam.append(el("span", "ic", "📷"), el("span", "", "Fotocamera"));
  cam.onclick = () => { chiudiFoglio(); $("in-foto").click(); };
  griglia.appendChild(cam);

  const did = el("input"); did.type = "text"; did.placeholder = "Aggiungi una didascalia…";
  const invia = el("button", "invia-gal", "Invia");
  const barraInvio = el("div", "invio-gal nascosto"); barraInvio.append(did, invia);

  const schede = el("div", "schede-gal");
  [["🖼", "Galleria", "#3fa7c9", null], ["📄", "File", "#4f86e0", "in-file"], ["🎞", "Altre foto", "#7f6cff", "in-galleria"],
   ["🎥", "Video", "#e05a8a", "in-video"]].forEach(([ic, nome, col, inp]) => {
    const b = el("button", inp ? "" : "attiva"); const t = el("span", "tondo", ic); t.style.background = col;
    b.append(t, el("span", "", nome));
    b.onclick = () => { if (!inp) return; chiudiFoglio(); $(inp).click(); };
    schede.appendChild(b);
  });

  const aggiorna = () => {
    griglia.querySelectorAll(".tile[data-i]").forEach((t) => {
      const k = sel.indexOf(t._voce);
      t.classList.toggle("scelta", k >= 0);
      t.querySelector(".num").textContent = k >= 0 ? String(k + 1) : "";
    });
    conta.textContent = sel.length ? sel.length + " selezionat" + (sel.length === 1 ? "o" : "i") : "";
    barraInvio.classList.toggle("nascosto", !sel.length);
    schede.classList.toggle("nascosto", !!sel.length);
    invia.textContent = sel.length > 1 ? "Invia (" + sel.length + ")" : "Invia";
  };

  f.append(el("div", "maniglia"), testa, griglia, schede, barraInvio);
  $("velo").classList.remove("nascosto"); f.classList.remove("nascosto");

  let dati = { disponibile: false, voci: [] };
  try { dati = await (await fetch("/api/galleria")).json(); } catch (e) { /* niente */ }
  if (!dati.voci.length) {
    const v = el("div", "vuoto-gal", dati.disponibile ? "Nessuna foto o video recente."
      : "Per vedere qui le foto del telefono, in Termux scrivi:  termux-setup-storage  e consenti l'accesso ai file.");
    griglia.appendChild(v);
  }
  dati.voci.forEach((v, i) => {
    const t = el("button", "tile"); t.dataset.i = i; t._voce = v;
    const img = el("img"); img.loading = "lazy"; img.decoding = "async"; img.alt = v.nome;
    img.src = "/api/miniatura?p=" + v.p;
    img.onerror = () => { img.remove(); t.appendChild(el("span", "ic", v.mime.startsWith("video/") ? "🎬" : "🖼")); };
    t.appendChild(img);
    if (v.mime.startsWith("video/")) t.appendChild(el("span", "video-gal", "▶"));
    t.appendChild(el("span", "num"));
    t.onclick = () => { const k = sel.indexOf(v); k >= 0 ? sel.splice(k, 1) : sel.push(v); aggiorna(); };
    griglia.appendChild(t);
  });

  invia.onclick = async () => {
    const dest = S.attivo, testo = did.value.trim(), scelte = sel.slice();
    chiudiFoglio();
    for (const v of scelte) {
      const percorso = daHex(v.p);
      if (S.exif && (v.mime === "image/jpeg" || v.mime === "image/png")) {
        // foto con metadati: si ripulisce qui (ridisegno su canvas) e si carica la copia pulita
        try {
          const blob = await (await fetch("/api/file?p=" + v.p)).blob();
          await carica(dest, new File([blob], v.nome, { type: v.mime }), true);
          continue;
        } catch (e) { /* ripiego: invio diretto */ }
      }
      manda("SENDFILE " + dest + " " + v.p + " " + v.mime);   // il motore legge il file direttamente
    }
    if (testo) manda("SEND " + dest + " " + inHex(testo));
  };
}
["in-foto", "in-video", "in-galleria", "in-file"].forEach((id) => {
  $(id).onchange = (e) => { const f = [...e.target.files]; e.target.value = ""; if (f.length) anteprimaInvio(f); };
});

function anteprimaInvio(files) {
  const scatola = [];
  const visuali = files.filter((f) => /^(image|video)\//.test(f.type));
  if (visuali.length) {
    const m = el("div", "miniature" + (visuali.length === 1 ? " una" : ""));
    visuali.forEach((f) => {
      const u = URL.createObjectURL(f);
      const x = f.type.startsWith("video/") ? Object.assign(el("video"), { src: u, muted: true, playsInline: true }) : Object.assign(el("img"), { src: u });
      m.appendChild(x);
    });
    scatola.push(m);
  }
  files.filter((f) => !/^(image|video)\//.test(f.type)).forEach((f) => {
    const [, col] = famiglia(f.type || "", f.name); const s = el("div", "file"); const c = el("div", "cerchio", estensione(f.name)); c.style.background = col;
    const info = el("div"); info.append(el("div", "nomef", f.name), el("div", "dett", dimensione(f.size))); s.append(c, info); scatola.push(s);
  });
  const did = el("input"); did.type = "text"; did.placeholder = "Aggiungi una didascalia…";
  scatola.push(did);
  let spunta = null;
  if (files.some((f) => f.type === "image/jpeg" || f.type === "image/png")) {
    const l = el("label", "spunta"); spunta = el("input"); spunta.type = "checkbox"; spunta.checked = S.exif;
    l.append(spunta, el("span", "", "Togli i metadati dalle foto (posizione GPS, fotocamera)")); scatola.push(l);
  }
  dialogo(files.length > 1 ? "Invia " + files.length + " file" : "Invia", scatola, [
    ["Annulla", null], ["Invia", async () => {
      const dest = S.attivo, togli = spunta ? spunta.checked : false, testo = did.value.trim();
      for (const f of files) await carica(dest, f, togli);
      if (testo) manda("SEND " + dest + " " + inHex(testo));
    }, "primario"]]);
  setTimeout(() => did.focus(), 50);
}

// foto ripulite NEL TELEFONO: ridisegnate su canvas (orientamento corretto, niente EXIF)
async function senzaMetadati(f) {
  if (!(f.type === "image/jpeg" || f.type === "image/png") || !window.createImageBitmap) return f;
  try {
    const bmp = await createImageBitmap(f, { imageOrientation: "from-image" });
    const c = document.createElement("canvas"); c.width = bmp.width; c.height = bmp.height;
    c.getContext("2d").drawImage(bmp, 0, 0);
    const blob = await new Promise((ok) => c.toBlob(ok, f.type, 0.92));
    return blob ? new File([blob], f.name, { type: f.type }) : f;
  } catch (e) { return f; }
}

async function carica(dest, file, togliExif) {
  const f = togliExif ? await senzaMetadati(file) : file;
  const id = "c" + Math.random().toString(36).slice(2);
  const mime = f.type || "application/octet-stream";
  avanzamento("out", id, 0, f.size, mime, f.name);
  const percorso = await new Promise((ok) => {
    const x = new XMLHttpRequest();
    x.open("POST", "/api/carica?nome=" + encodeURIComponent(f.name));
    x.upload.onprogress = (e) => { if (S.attivo === dest) avanzamento("out", id, e.loaded, e.total || f.size, mime, f.name); };
    x.onload = () => { try { ok(JSON.parse(x.responseText).percorso || null); } catch (e) { ok(null); } };
    x.onerror = () => ok(null);
    x.send(f);
  });
  togliTrasferimento(id);
  if (!percorso) return toast("Caricamento non riuscito: " + f.name);
  manda("SENDFILE " + dest + " " + inHex(percorso) + " " + mime);
}

// ---------- menu della pressione lunga (come il tasto destro sul PC) --------------------
function apriFoglio(titolo, contenuto) {
  const f = $("foglio"); f.textContent = ""; f.classList.remove("galleria");
  f.append(el("div", "maniglia"), ...(titolo ? [el("div", "titolo", titolo)] : []), ...contenuto);
  $("velo").classList.remove("nascosto"); f.classList.remove("nascosto");
}
function chiudiFoglio() { $("velo").classList.add("nascosto"); $("foglio").classList.add("nascosto"); $("foglio").classList.remove("galleria"); }
$("velo").onclick = chiudiFoglio;
const voce = (ic, testo, az, pericolosa) => {
  const b = el("button", "voce" + (pericolosa ? " pericolosa" : "")); b.append(el("span", "ic", ic), el("span", "", testo));
  b.onclick = () => { chiudiFoglio(); az(); }; return b;
};

function menuContatto(addr) {
  const nome = S.contatti.get(addr) || addr.slice(0, 10), nl = S.nonLetti[addr], fiss = S.fissati.includes(addr);
  apriFoglio(nome, [
    voce("💬", "Apri la chat", () => apriChat(addr)),
    voce(nl ? "✉️" : "📩", nl ? "Segna come letto" : "Segna come non letto", () => {
      nl ? delete S.nonLetti[addr] : (S.nonLetti[addr] = 1); salva("nonLetti", S.nonLetti); disegnaElenco(); }),
    voce("📌", fiss ? "Togli dai fissati" : "Fissa in alto", () => {
      S.fissati = fiss ? S.fissati.filter((a) => a !== addr) : [...S.fissati, addr]; salva("fissati", S.fissati); disegnaElenco(); }),
    voce("✏️", "Rinomina…", () => dialogoRinomina(addr)),
    voce("📋", "Copia il Contact ID", () => copia(addr)),
    voce("🧹", "Cancella cronologia", () => conferma("Cancella cronologia?", "Tutti i messaggi con " + nome + " verranno cancellati da questo telefono.", "Cancella", () => {
      manda("CLEAR " + addr); S.ultimo.delete(addr); DB.togli("messaggi", addr); cacheContatto(addr); if (addr === S.attivo) $("messaggi").textContent = ""; disegnaElenco(); }), true),
    voce("🗑", "Elimina la chat", () => conferma("Elimina la chat?", nome + " verrà tolto dai contatti e la cronologia cancellata da questo telefono.", "Elimina", () => {
      manda("DEL " + addr); S.contatti.delete(addr); S.ultimo.delete(addr); DB.togli("messaggi", addr); DB.togli("contatti", addr); if (addr === S.attivo) { chiudiChat(); } disegnaElenco(); }), true),
  ]);
}

function menuMessaggio(riga, grezza, dati) {
  const voci = [];
  if (dati.file) {
    if (apribile(dati.mime)) voci.push(voce("📂", "Apri", () => apriFile(dati.file, dati.mime)));
    voci.push(voce("↪️", "Inoltra", () => inoltra(null, dati)));
  } else {
    voci.push(voce("📋", "Copia testo", () => copia(dati.testo)), voce("↪️", "Inoltra", () => inoltra(dati.testo)));
  }
  voci.push(voce("🗑", "Elimina per me", () => conferma("Elimina il messaggio?", "Verrà cancellato solo da questo telefono, non da quello dell'altra persona.", "Elimina", () => {
    manda("DELMSG " + S.attivo + " " + inHex(grezza)); cacheTogliRiga(S.attivo, grezza); riga.remove(); }), true));
  apriFoglio(null, voci);
}

function inoltra(testo, file) {
  const voci = [...S.contatti.entries()].sort((a, b) => a[1].localeCompare(b[1])).map(([addr, nome]) => voce("➡️", nome, () => {
    if (testo !== null) manda("SEND " + addr + " " + inHex(testo));
    else manda("SENDFILE " + addr + " " + inHex(file.file) + " " + file.mime);
    toast("Inoltrato a " + nome);
  }));
  apriFoglio("Inoltra a…", voci);
}

// dentro l'app Android (APK) la pagina ha il ponte window.NyctalusHost: appunti e
// condivisione passano da lì, perché nella WebView le API del browser non sempre ci sono
const HOST = window.NyctalusHost || null;

async function copia(testo) {
  if (HOST) { HOST.copia(String(testo)); toast("Copiato"); return; }
  try { await navigator.clipboard.writeText(testo); toast("Copiato"); }
  catch (e) { toast("Copia non riuscita"); }
}

// ---------- finestre di dialogo ------------------------------------------------------------
function dialogo(titolo, contenuto, pulsanti) {
  const s = $("dialogo-scatola"); s.textContent = "";
  s.append(el("h3", "", titolo), ...contenuto);
  const p = el("div", "pulsanti");
  pulsanti.forEach(([testo, az, cls]) => {
    const b = el("button", cls || "", testo);
    b.onclick = () => { $("dialogo").classList.add("nascosto"); if (az) az(); };
    p.appendChild(b);
  });
  s.appendChild(p); $("dialogo").classList.remove("nascosto");
}
$("dialogo").onclick = (e) => { if (e.target.id === "dialogo") $("dialogo").classList.add("nascosto"); };
const conferma = (titolo, testo, ok, az) => dialogo(titolo, [el("p", "", testo)], [["Annulla", null], [ok, az, "pericoloso"]]);

function dialogoAggiungi(pre, info) {
  const a = el("input"); a.type = "text"; a.placeholder = "Contact ID (indirizzo .nyct)"; a.value = pre || ""; a.autocapitalize = "off"; a.spellcheck = false;
  const n = el("input"); n.type = "text"; n.placeholder = "Nome (solo per te)";
  dialogo("Aggiungi contatto", [...(info ? [el("p", "", info)] : []), a, n], [["Annulla", null], ["Aggiungi", () => {
    let addr = a.value.trim().toLowerCase(); if (!addr) return;
    if (!addr.endsWith(".nyct")) addr += ".nyct";
    if (!/^[a-z2-7]{56}\.nyct$/.test(addr)) return toast("Contact ID non valido");
    const nome = n.value.trim() || addr.slice(0, 10);
    manda("ADD " + addr + " " + inHex(nome)); aggiornaContatto(addr, nome);
  }, "primario"]]);
  setTimeout(() => (pre ? n : a).focus(), 50);
}
function dialogoRinomina(addr) {
  const n = el("input"); n.type = "text"; n.value = S.contatti.get(addr) || "";
  dialogo("Rinomina", [n], [["Annulla", null], ["Salva", () => {
    const nome = n.value.trim(); if (!nome) return; manda("ADD " + addr + " " + inHex(nome)); aggiornaContatto(addr, nome); }, "primario"]]);
  setTimeout(() => n.focus(), 50);
}

function impostazioni() {
  const id = el("div", "codice-id", S.me || "…");
  const bott = el("div", "pulsanti"); bott.style.justifyContent = "flex-start"; bott.style.marginTop = "0";
  const bc = el("button", "", "📋 Copia"); bc.onclick = () => copia(S.me);
  bott.appendChild(bc);
  if (HOST) { const bs = el("button", "", "📤 Condividi"); bs.onclick = () => HOST.condividi(S.me); bott.appendChild(bs); }
  else if (navigator.share) { const bs = el("button", "", "📤 Condividi"); bs.onclick = () => navigator.share({ text: S.me }).catch(() => {}); bott.appendChild(bs); }
  const temi = el("div", "scelta-tema");
  [["notte", "🌙 Notte"], ["ardesia", "🪨 Ardesia"], ["viola", "🔮 Viola"], ["chiaro", "☀️ Chiaro"]].forEach(([k, nome]) => {
    const b = el("button", k === S.tema ? "sel" : "", nome);
    b.onclick = () => { S.tema = k; salva("tema", k); applicaAspetto(); temi.querySelectorAll("button").forEach((x) => x.classList.toggle("sel", x === b)); };
    temi.appendChild(b);
  });
  const sfondi = el("div", "scelta-tema");
  [["pipistrelli", "🦇 Pipistrelli"], ["velo", "✨ Velo"], ["tinta", "▪️ Tinta unita"]].forEach(([k, nome]) => {
    const b = el("button", k === S.sfondo ? "sel" : "", nome);
    b.onclick = () => { S.sfondo = k; salva("sfondo", k); applicaAspetto(); sfondi.querySelectorAll("button").forEach((x) => x.classList.toggle("sel", x === b)); };
    sfondi.appendChild(b);
  });
  const l = el("label", "spunta"); const c = el("input"); c.type = "checkbox"; c.checked = S.exif;
  c.onchange = () => { S.exif = c.checked; salva("exif", c.checked); };
  l.append(c, el("span", "", "Foto senza metadati (consigliato)"));
  const servizi = el("div", "pulsanti"); servizi.style.justifyContent = "flex-start";
  const ba = el("button", "", "▶ Avvia rete"); ba.onclick = async () => { await post("/api/servizi/avvia"); controllaServizi(); toast("Avvio…"); };
  const bf = el("button", "pericoloso", "■ Ferma rete"); bf.onclick = async () => { await post("/api/servizi/ferma"); controllaServizi(); toast("Nyctalus fermato"); };
  servizi.append(ba, bf);
  dialogo("Impostazioni", [el("p", "", "Il tuo Contact ID — dallo agli amici perché ti scrivano (non è segreto):"), id, bott,
    el("p", "", "Tema"), temi, el("p", "", "Sfondo della chat"), sfondi, l, el("p", "", "Rete Nyctalus"), servizi], [["Chiudi", null, "primario"]]);
}

function applicaAspetto() {
  document.documentElement.dataset.tema = S.tema;
  document.documentElement.dataset.sfondo = S.sfondo;
  const pannello = getComputedStyle(document.documentElement).getPropertyValue("--panel").trim();
  document.querySelector('meta[name="theme-color"]').content = pannello || "#0e1424";
}

// ---------- pulsanti fissi e avvio ----------------------------------------------------------
$("btn-nuovo").onclick = () => dialogoAggiungi();
$("btn-menu").onclick = impostazioni;
$("btn-cerca").onclick = () => { $("ricerca").classList.toggle("nascosto"); $("campo-cerca").focus(); };
$("campo-cerca").oninput = (e) => { S.filtro = e.target.value.trim().toLowerCase(); disegnaElenco(); };
$("indietro").onclick = () => history.back();
$("btn-altro").onclick = () => S.attivo && menuContatto(S.attivo);
$("btn-avvia").onclick = async () => { await post("/api/servizi/avvia"); controllaServizi(); };
$("mio-id").onclick = () => S.me && copia(S.me);

applicaAspetto();
DB.tutti("contatti").then((voci) => {
  (voci || []).forEach((v) => { if (!S.contatti.has(v.addr)) { S.contatti.set(v.addr, v.nome); if (v.ultimo) S.ultimo.set(v.addr, v.ultimo); } });
  disegnaElenco();
});
controllaServizi();
if ("serviceWorker" in navigator) navigator.serviceWorker.register("sw.js").catch(() => {});
