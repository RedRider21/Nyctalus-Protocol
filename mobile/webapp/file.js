// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
// Nyctalus · File sul telefono (e su Windows): file grandi ai contatti, attraverso la rete.
// Stesso motore della chat (comandi dei trasferimenti):
//   →  XSEND <addr> <percorso hex> [mime] · XOK|XNO|XPAUSA|XRIPRENDI|XANNULLA <id> · XLIST · XCONF
//   ←  XNEW · XSTATO · XPROG · XFINE · ENDX
// NIENTE COPIE: si sceglie nelle cartelle del telefono (galleria o «sfoglia») e il motore legge
// il file dov'è. I file ricevuti restano nell'ARCHIVIO PRIVATO di Nyctalus (cartella di Termux,
// invisibile alle altre app): da qui si aprono, si condividono (scelta esplicita) o si eliminano.
"use strict";
const FileGrandi = (() => {
  const T = new Map();          // id → {id, verso, peer, dim, mime, nome, stato, fatti, bps, percorso, dettaglio, ts}
  let pagina = null;            // la schermata aperta (per aggiornarla al volo)
  let scheda = "in";
  let motoreOk = null;          // null = non so ancora; false = il motore non conosce i trasferimenti
  let attesaElenco = 0;

  const misura = (n) => {
    n = +n || 0;
    for (const u of ["B", "KB", "MB", "GB", "TB"]) {
      if (n < 1024 || u === "TB") return u === "B" ? n + " B" : n.toFixed(1).replace(".", ",") + " " + u;
      n /= 1024;
    }
    return "";
  };
  const durata = (s) => (s < 60 ? Math.max(1, Math.round(s)) + " s" : s < 3600 ? Math.ceil(s / 60) + " min"
    : Math.floor(s / 3600) + " h " + Math.floor((s % 3600) / 60) + " min");
  const nomeDi = (addr) => S.contatti.get(addr) || addr.slice(0, 10);
  // voce di menu che NON chiude il foglio (la chiusura, a tutto schermo, riporterebbe all'avvio):
  // l'azione apre il passo successivo al suo posto
  const foglioFile = (titolo, contenuto) => apriFoglio(titolo, [vf("↩", "Torna a File", () => apri(S.soloRubrica)), ...contenuto]);
  const vf = (ic, testo, az) => { const b = el("button", "voce"); b.append(el("span", "ic", ic), el("span", "", testo)); b.onclick = az; return b; };
  const icona = (mime) => mime.startsWith("image/") ? "🖼" : mime.startsWith("video/") ? "🎬" : mime.startsWith("audio/") ? "🎵"
    : mime === "application/pdf" ? "📄" : /zip|tar|7z|rar|compress/.test(mime) ? "🗜" : "📦";

  function testoStato(d) {
    const chi = nomeDi(d.peer), out = d.verso === "out";
    let t = {
      prepara: "Preparo il file…",
      attesa: out ? "Aspetto che " + chi + " sia in linea" : "Riprende quando " + chi + " è in linea",
      chiede: out ? "Aspetto che " + chi + " accetti" : "Vuole mandarti questo file",
      pausa: "In pausa", fatto: out ? "Consegnato" : "Arrivato",
      errore: "Problema: " + (d.dettaglio || "?"), rifiutato: "Rifiutato", annullato: "Annullato",
    }[d.stato] || d.stato;
    if (d.stato === "corre") t = d.bps > 0 && d.dim ? misura(d.bps) + "/s · mancano " + durata((d.dim - d.fatti) / d.bps) : "In corso…";
    if (["corre", "pausa", "attesa", "errore"].includes(d.stato) && d.fatti > 0 && d.fatti < d.dim) t = misura(d.fatti) + " · " + t;
    return t;
  }

  function riga(d) {
    const r = el("div", "xf-riga"); r.dataset.id = d.id;
    const ic = el("div", "xf-ic", icona(d.mime));
    const corpo = el("div", "xf-corpo");
    const testa = el("div", "xf-testa");
    testa.append(el("span", "xf-nome", d.nome), el("span", "xf-sotto", (d.verso === "out" ? "a " : "da ") + nomeDi(d.peer) + " · " + misura(d.dim)));
    const barra = el("div", "xf-barra" + (d.stato === "fatto" ? " fatto" : d.stato !== "corre" ? " ferma" : ""));
    const pieno = el("div", "xf-pieno");
    pieno.style.width = Math.min(100, d.stato === "fatto" ? 100 : d.dim ? (100 * d.fatti) / d.dim : 0) + "%";
    barra.append(pieno);
    const st = el("div", "xf-stato " + (d.stato === "chiede" && d.verso === "in" ? "chiede" : d.stato), testoStato(d));
    const az = el("div", "xf-azioni");
    const b = (testo, f, cls) => { const x = el("button", cls || "", testo); x.onclick = f; az.append(x); };
    if (d.stato === "chiede" && d.verso === "in") { b("Rifiuta", () => comando("XNO", d.id), "pericolo"); b("Accetta", () => comando("XOK", d.id), "primario"); }
    else if (["corre", "attesa"].includes(d.stato)) { b("Pausa", () => comando("XPAUSA", d.id)); b("Annulla", () => annulla(d), "pericolo"); }
    else if (["pausa", "errore"].includes(d.stato)) { b("Riprendi", () => comando("XRIPRENDI", d.id), "primario"); b("Annulla", () => annulla(d), "pericolo"); }
    else if (d.stato === "fatto" && d.percorso && d.verso === "in") b("Apri", () => apriFile(d.percorso, d.mime), "primario");
    corpo.append(testa, barra, st, az);
    r.append(ic, corpo);
    return r;
  }

  function disegna() {
    if (!pagina || !pagina.isConnected) { pagina = null; return; }
    const lista = pagina.querySelector(".xf-lista");
    lista.textContent = "";
    pagina.querySelectorAll(".xf-schede button").forEach((x) => x.classList.toggle("sel", x.dataset.v === scheda));
    const conta = { in: 0, out: 0 };
    T.forEach((d) => { conta[d.verso]++; });
    pagina.querySelector('.xf-schede [data-v="in"]').textContent = "In arrivo" + (conta.in ? " · " + conta.in : "");
    pagina.querySelector('.xf-schede [data-v="out"]').textContent = "In uscita" + (conta.out ? " · " + conta.out : "");
    if (scheda === "arch") return disegnaArchivio(lista);
    if (motoreOk === false) {
      lista.append(el("p", "xf-vuoto", "Il motore di Nyctalus su questo dispositivo è di una versione che non conosce ancora il programma File. Aggiornalo (nell'app: «Installa / aggiorna Nyctalus in Termux») quando sarà disponibile."));
      return;
    }
    const voci = [...T.values()].filter((d) => d.verso === scheda).sort((a, b) => b.ts - a.ts);
    if (!voci.length) {
      lista.append(el("p", "xf-vuoto", scheda === "in" ? "Nessun file in arrivo.\nQuando un contatto ti manda un file, lo trovi qui."
        : "Nessun file inviato.\nTocca «＋ Invia» per mandare un file grande a un contatto."));
      return;
    }
    voci.forEach((d) => lista.append(riga(d)));
  }
  function aggiornaRiga(d) {
    if (!pagina || !pagina.isConnected) return;
    const vecchia = pagina.querySelector('.xf-riga[data-id="' + d.id + '"]');
    if (vecchia && d.verso === scheda) vecchia.replaceWith(riga(d)); else disegna();
  }

  function comando(c, id) { manda(c + " " + id); }
  function annulla(d) {
    conferma("Annullare «" + d.nome + "»?", "Il trasferimento si ferma per tutti e due; quello che è già arrivato viene buttato.", "Annulla il file", () => comando("XANNULLA", d.id));
  }

  // --- invio: a chi, poi da dove (galleria senza copie, o altri file) -----------------------
  function scegliContatto(poi) {
    const contatti = [...S.contatti.entries()].filter(([a]) => a !== S.rubrica).sort((a, b) => a[1].localeCompare(b[1]));
    if (!contatti.length) return toast("Non hai ancora contatti: aggiungili dalla chat");
    foglioFile("A chi mandi il file?", contatti.map(([addr, nome]) => vf("👤", nome, () => poi(addr))));
  }
  function scegliOrigine(addr) {
    foglioFile("Cosa mandi a " + nomeDi(addr) + "?", [
      vf("🖼", "Foto e video della galleria", () => dallaGalleria(addr)),
      vf("📁", "Sfoglia le cartelle del telefono", () => sfogliaCartelle(addr, "")),
    ]);
  }
  async function dallaGalleria(addr) {
    let r;
    try { r = await (await fetch("/api/galleria?n=60")).json(); } catch (e) { return toast("Galleria non disponibile"); }
    if (!r.voci || !r.voci.length) return toast(r.disponibile ? "La galleria è vuota" : "Galleria non disponibile: in Termux serve «termux-setup-storage»");
    const scelte = new Set();
    const griglia = el("div", "xf-griglia");
    r.voci.forEach((v) => {
      const c = el("button", "xf-tile");
      const img = el("img"); img.loading = "lazy"; img.src = "/api/miniatura?p=" + v.p;
      c.append(img, el("span", "xf-dim", misura(v.dim)));
      c.onclick = () => { scelte.has(v) ? scelte.delete(v) : scelte.add(v); c.classList.toggle("scelta", scelte.has(v)); invia.textContent = "Invia (" + scelte.size + ")"; invia.disabled = !scelte.size; };
      griglia.append(c);
    });
    const invia = el("button", "xf-invia primario", "Invia (0)"); invia.disabled = true;
    invia.onclick = () => {
      scelte.forEach((v) => manda("XSEND " + addr + " " + v.p + " " + v.mime));   // il percorso è già in esadecimale
      apri(S.soloRubrica, "out");   // si torna a File (senza chiudere: nell'app riporterebbe alla schermata iniziale)
      toast(scelte.size === 1 ? "File in partenza" : scelte.size + " file in partenza");
    };
    foglioFile("Galleria → " + nomeDi(addr), [griglia, invia]);
  }
  // le cartelle del telefono, in sola lettura: si toccano i file (anche più d'uno) e si mandano
  async function sfogliaCartelle(addr, d, scelte) {
    scelte = scelte || new Map();
    let r;
    try { r = await (await fetch("/api/sfoglia?d=" + d)).json(); } catch (e) { return toast("Cartelle non disponibili"); }
    if (r.errore) return toast(r.errore);
    if (!r.voci.length && !d) return toast("Cartelle del telefono non disponibili: in Termux serve «termux-setup-storage»");
    const elenco = el("div", "xf-sfoglia");
    if (d) elenco.append(vf("⬅️", r.su ? "Cartella superiore" : "Tutte le cartelle", () => sfogliaCartelle(addr, r.su || "", scelte)));
    const invia = el("button", "xf-invia primario", "Invia (" + scelte.size + ")"); invia.disabled = !scelte.size;
    r.voci.forEach((v) => {
      if (v.dir) { elenco.append(vf("📁", v.nome, () => sfogliaCartelle(addr, v.p, scelte))); return; }
      const b = vf(icona(v.mime), v.nome + "  ·  " + misura(v.dim), () => {
        scelte.has(v.p) ? scelte.delete(v.p) : scelte.set(v.p, v);
        b.classList.toggle("scelta", scelte.has(v.p));
        invia.textContent = "Invia (" + scelte.size + ")"; invia.disabled = !scelte.size;
      });
      b.classList.toggle("scelta", scelte.has(v.p));
      elenco.append(b);
    });
    invia.onclick = () => {
      scelte.forEach((v) => manda("XSEND " + addr + " " + v.p + " " + v.mime));   // percorso già in esadecimale
      const n = scelte.size;
      apri(S.soloRubrica, "out");   // si torna a File (senza chiudere: nell'app riporterebbe alla schermata iniziale)
      toast(n === 1 ? "File in partenza" : n + " file in partenza");
    };
    foglioFile((r.titolo || "Cartelle del telefono") + " → " + nomeDi(addr), [elenco, invia]);
  }

  // --- archivio PRIVATO dei file ricevuti: spazio, apri, condividi, elimina ------------------
  let arch = null;
  const scelteArch = new Set();
  async function caricaArchivio() {
    try { arch = await (await fetch("/api/archivio")).json(); } catch (e) { arch = { voci: [], totale: 0, libero: 0 }; }
    scelteArch.forEach((p) => { if (!arch.voci.some((v) => v.p === p)) scelteArch.delete(p); });
    disegna();
  }
  const bytesDaHex = (h) => new TextDecoder().decode(new Uint8Array(h.match(/../g).map((x) => parseInt(x, 16))));
  function disegnaArchivio(lista) {
    if (!arch) { lista.append(el("p", "xf-vuoto", "Carico l'archivio…")); caricaArchivio(); return; }
    const testa = el("div", "xf-spazio");
    testa.append(el("span", "", "Occupati " + misura(arch.totale)), el("span", "", "Liberi sul telefono " + misura(arch.libero)));
    lista.append(testa);
    if (!arch.voci.length) { lista.append(el("p", "xf-vuoto", "L'archivio è vuoto.\nQui finiscono i file che ricevi: restano privati, nessun'altra app li vede.")); return; }
    if (scelteArch.size) {
      const barra = el("div", "xf-azioni xf-selezione");
      const tot = arch.voci.filter((v) => scelteArch.has(v.p)).reduce((a, v) => a + v.dim, 0);
      const annulla = el("button", "", "Annulla selezione"); annulla.onclick = () => { scelteArch.clear(); disegna(); };
      const elimina = el("button", "pericolo", "Elimina " + scelteArch.size + " (" + misura(tot) + ")");
      elimina.onclick = () => conferma("Eliminare " + scelteArch.size + " file?", "Spariscono dall'archivio di Nyctalus e non si possono recuperare.", "Elimina", async () => {
        for (const p of [...scelteArch]) await post("/api/elimina?p=" + p);
        scelteArch.clear(); toast("Eliminati"); caricaArchivio();
      });
      barra.append(annulla, elimina); lista.append(barra);
    }
    arch.voci.forEach((v) => {
      const r = el("div", "xf-riga" + (scelteArch.has(v.p) ? " scelta" : ""));
      const sel = el("input"); sel.type = "checkbox"; sel.className = "xf-spunta"; sel.checked = scelteArch.has(v.p);
      sel.onchange = () => { sel.checked ? scelteArch.add(v.p) : scelteArch.delete(v.p); disegna(); };
      const corpo = el("div", "xf-corpo");
      const testaR = el("div", "xf-testa");
      testaR.append(el("span", "xf-nome", v.nome), el("span", "xf-sotto", misura(v.dim)));
      const quando = new Date(v.ts * 1000).toLocaleDateString([], { day: "numeric", month: "short" });
      corpo.append(testaR, el("div", "xf-stato", quando));
      const az = el("div", "xf-azioni");
      const b = (t, f, cls) => { const x = el("button", cls || "", t); x.onclick = f; az.append(x); };
      if (apribile(v.mime)) b("Apri", () => apriFile(bytesDaHex(v.p), v.mime));
      b("Condividi", () => condividi(v));
      b("Elimina", () => conferma("Eliminare «" + v.nome + "»?", "Sparisce dall'archivio di Nyctalus (" + misura(v.dim) + ") e non si può recuperare.", "Elimina", async () => {
        const e = await post("/api/elimina?p=" + v.p); toast(e.ok ? "Eliminato" : "✗ " + (e.errore || "non riuscito")); caricaArchivio();
      }), "pericolo");
      corpo.append(az);
      r.append(sel, el("div", "xf-ic", icona(v.mime)), corpo);
      lista.append(r);
    });
  }
  function condividi(v) {
    conferma("Condividere «" + v.nome + "»?", "Il file esce dall'archivio privato e va all'app che sceglierai (es. WhatsApp, Drive, la galleria). "
      + "L'anonimato della rete Nyctalus non cambia, ma da lì in poi il file è nelle mani di quell'app: può copiarlo, caricarlo online o leggerne i metadati (per esempio la posizione delle foto).",
      "Condividi", async () => { const r = await post("/api/condividi?p=" + v.p); if (!r.ok) toast("Condivisione non disponibile su questo dispositivo"); });
  }

  // --- impostazioni: soglia e collegamenti in parallelo ------------------------------------
  let conf = null;
  function impostazioniFile() {
    const s = el("input"); s.type = "number"; s.min = 0; s.max = 100000; s.value = conf ? Math.round(conf.soglia / 1048576) : 100;
    const c = el("input"); c.type = "number"; c.min = 1; c.max = 8; c.value = conf ? conf.canali : 3;
    dialogo("Impostazioni di File", [
      el("p", "", "Accetta da solo i file dei contatti fino a (MB):"), s,
      el("p", "", "Collegamenti in parallelo (più veloce, più carico sulla rete):"), c,
    ], [["Annulla", null], ["Salva", () => manda("XCONF soglia " + Math.max(0, +s.value || 0) * 1048576 + " canali " + Math.min(8, Math.max(1, +c.value || 3))), "primario"]]);
  }

  // --- la schermata ------------------------------------------------------------------------
  function apri(solo, verso) {
    if (verso) scheda = verso;
    if (pagina && pagina.isConnected) return disegna();
    if (solo && !S.soloRubrica) { S.soloRubrica = true; document.body.classList.add("solo-rubrica"); if (HOST && HOST.rubrica) HOST.rubrica(true); }
    pagina = el("div", "file-pagina");
    const testa = el("div", "xf-barra-alta");
    const invia = el("button", "xf-nuovo", "＋ Invia"); invia.onclick = () => scegliContatto(scegliOrigine);
    const ing = el("button", "xf-ing", "⚙"); ing.title = "Impostazioni di File"; ing.onclick = impostazioniFile;
    testa.append(invia, ing);
    const schede = el("div", "xf-schede");
    for (const [v, t] of [["in", "In arrivo"], ["out", "In uscita"], ["arch", "Archivio"]]) {
      const b = el("button", "", t); b.dataset.v = v; b.onclick = () => { scheda = v; if (v === "arch") arch = null; disegna(); }; schede.append(b);
    }
    pagina.append(testa, schede, el("div", "xf-lista"));
    apriFoglio("File", [pagina]);
    if (solo) {
      const ind = el("button", "indietro-rubrica", "←"); ind.onclick = chiudiFoglio;
      document.querySelector("#foglio .titolo").prepend(ind);
    }
    chiediElenco();
    disegna();
  }
  function chiediElenco() {
    if (attesaElenco) return;
    manda("XLIST"); manda("XCONF");
    // un motore vecchio non risponde con ENDX (risponde ERR): lo si dice invece di restare muti
    attesaElenco = setTimeout(() => { attesaElenco = 0; if (motoreOk === null) { motoreOk = false; disegna(); } }, 4000);
  }
  // da una conversazione: «Manda un file grande» a quel contatto
  function inviaA(addr) { scegliOrigine(addr); }

  // --- eventi del motore -----------------------------------------------------------------
  function evento(t, p) {
    if (t === "ENDX") { motoreOk = true; disegna(); return true; }
    if (t === "XCONF" && p.length >= 5) { conf = { soglia: +p[2], canali: +p[4] }; return true; }
    if (t === "XNEW" && p.length >= 9) {
      motoreOk = true;
      let nome = "?"; try { nome = new TextDecoder().decode(new Uint8Array(p[6].match(/../g).map((h) => parseInt(h, 16)))); } catch (e) { /* niente */ }
      const vecchio = T.get(p[1]);
      const d = Object.assign(vecchio || { ts: Date.now() / 1000 }, { id: p[1], verso: p[2], peer: p[3], dim: +p[4], mime: p[5], nome, stato: p[7], fatti: +p[8] });
      T.set(d.id, d);
      if (!vecchio && d.verso === "in" && d.stato === "chiede") toast("📦 " + nomeDi(d.peer) + " vuole mandarti «" + nome + "» (" + misura(d.dim) + "): apri File");
      aggiornaRiga(d);
      return true;
    }
    const d = T.get(p[1]);
    if (t === "XSTATO" && d) {
      d.stato = p[2];
      if (p[3]) { try { d.dettaglio = new TextDecoder().decode(new Uint8Array(p[3].match(/../g).map((h) => parseInt(h, 16)))); } catch (e) { /* niente */ } }
      if (d.stato === "fatto") d.fatti = d.dim;
      aggiornaRiga(d); return true;
    }
    if (t === "XPROG" && d) { d.fatti = +p[2]; d.bps = +p[4]; aggiornaRiga(d); return true; }
    if (t === "XFINE" && d) {
      try { d.percorso = new TextDecoder().decode(new Uint8Array(p[2].match(/../g).map((h) => parseInt(h, 16)))); } catch (e) { /* niente */ }
      d.stato = "fatto"; d.fatti = d.dim;
      if (d.verso === "in") { toast("📦 Arrivato da " + nomeDi(d.peer) + ": «" + d.nome + "» (nell'Archivio)"); arch = null; }
      aggiornaRiga(d); return true;
    }
    return ["XSTATO", "XPROG", "XFINE"].includes(t);
  }
  // all'avvio (motore collegato) si chiede l'elenco, così le notifiche delle proposte funzionano
  function collegato() { motoreOk = null; chiediElenco(); }

  return { apri, inviaA, evento, collegato, _T: T, _sfoglia: sfogliaCartelle, _stato: () => ({ motoreOk, scheda, conf }) };
})();
window.nyctApriFile = () => FileGrandi.apri(true);
