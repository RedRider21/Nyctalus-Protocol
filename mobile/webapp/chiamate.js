// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
// Chiamate vocali nella chat del telefono (stile Telegram). Segnali e voce passano dal motore
// dentro la sessione cifrata con il contatto, attraverso la rete Nyctalus (niente collegamento
// diretto: nessuno vede l'IP dell'altro).
//   →  CALL addr · CALLOK|CALLNO|CALLEND addr id · AUD addr seq hex
//   ←  CALLID · CALLIN · CALLOK · CALLNO · CALLEND addr id · AUD addr seq hex
// Voce: microfono → blocchi da 20 ms (worklet) → Opus (WebCodecs) → AUD; AUD → Opus → altoparlante
// con un piccolo margine; i pacchetti in ritardo si scartano (stesso formato della chat del PC).
"use strict";
const Chiamate = (() => {
  const SQUILLO = 45000;
  let c = null;                 // {addr, id, stato: "chiamo"|"squilla"|"attiva", inizio}
  let a = null;                 // audio: {ctx, mic, nodo, enc, dec, gain, prossimo, seq, ultimo, ricevuti, persi}
  let muto = false, vivavoce = false, ridotta = false, timer = 0;

  // --- schermata della chiamata ----------------------------------------------------------
  const sch = el("div", "chiamata nascosto");
  const riduci = el("button", "ch-riduci", "⌄"); riduci.title = "Riduci: la chiamata continua";
  const av = el("div", "ch-av"), nome = el("div", "ch-nome"), stato = el("div", "ch-stato"), qual = el("div", "ch-qualita");
  const tasto = (ic, testo, cls) => { const b = el("button", "ch-tasto " + (cls || "")); b.append(el("span", "ic", ic), el("span", "", testo)); return b; };
  const tMuto = tasto("🎤", "Muto"), tViva = tasto("🔊", "Vivavoce"), tVideo = tasto("🎥", "Video"), tFine = tasto("✆", "Riaggancia", "rosso");
  tVideo.disabled = true; tVideo.title = "Videochiamata: in arrivo";
  const tasti = el("div", "ch-tasti"); tasti.append(tMuto, tViva, tVideo, tFine);
  const tNo = tasto("✆", "Rifiuta", "rosso"), tSi = tasto("📞", "Rispondi", "verde");
  const arrivo = el("div", "ch-tasti ch-arrivo"); arrivo.append(tNo, tSi);
  sch.append(riduci, av, nome, stato, qual, tasti, arrivo);
  const mini = el("button", "ch-mini nascosto");
  document.body.append(sch, mini);

  const disponibile = () => !!(navigator.mediaDevices && navigator.mediaDevices.getUserMedia && window.AudioEncoder && window.AudioDecoder && window.AudioWorkletNode);
  const nomeDi = (addr) => S.contatti.get(addr) || addr.slice(0, 10);
  const durata = (s) => { s = Math.floor(s); return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0"); };
  const hex = (b) => Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
  const daHexB = (h) => { const b = new Uint8Array(h.length / 2); for (let i = 0; i < b.length; i++) b[i] = parseInt(h.substr(i * 2, 2), 16); return b; };

  function disegna() {
    if (!c) { sch.classList.add("nascosto"); mini.classList.add("nascosto"); return; }
    const n = nomeDi(c.addr);
    av.textContent = ""; av.appendChild(avatar(c.addr, n, true));
    nome.textContent = n;
    stato.textContent = c.stato === "chiamo" ? "Chiamo…" : c.stato === "squilla" ? "ti sta chiamando" : durata((Date.now() - c.inizio) / 1000);
    tasti.classList.toggle("nascosto", c.stato === "squilla");
    arrivo.classList.toggle("nascosto", c.stato !== "squilla");
    tMuto.classList.toggle("attivo", muto); tViva.classList.toggle("attivo", vivavoce);
    sch.classList.toggle("nascosto", ridotta);
    mini.classList.toggle("nascosto", !ridotta);
    mini.textContent = "📞 " + n + " · " + (c.stato === "attiva" ? durata((Date.now() - c.inizio) / 1000) : "Chiamo…") + " — tocca per tornare";
  }
  function avviaTimer() { if (!timer) timer = setInterval(tick, 1000); }
  function tick() {
    if (!c) { clearInterval(timer); timer = 0; return; }
    if (c.stato !== "attiva" && Date.now() - c.inizio > SQUILLO) {
      if (c.stato === "chiamo") return fine("Nessuna risposta");
      return rifiuta();
    }
    if (a && c.stato === "attiva") {
      const r = a.ricevuti - (a.r0 || 0), p = a.persi - (a.p0 || 0); a.r0 = a.ricevuti; a.p0 = a.persi;
      qual.textContent = r === 0 ? "📵 niente audio" : "📶 " + (p / (r + p) < 0.03 ? "linea buona" : p / (r + p) < 0.12 ? "linea discreta" : "linea scarsa");
    } else qual.textContent = "";
    disegna();
  }

  // --- azioni ----------------------------------------------------------------------------
  function chiama(addr) {
    if (c) return toast("C'è già una chiamata in corso");
    if (!disponibile()) return toast("Questo telefono non supporta ancora le chiamate (WebView troppo vecchia)");
    c = { addr, id: null, stato: "chiamo", inizio: Date.now() };
    ridotta = false; muto = false; vivavoce = false;
    manda("CALL " + addr);
    avviaTimer(); disegna();
  }
  async function rispondi() {
    if (!c || c.stato !== "squilla") return;
    manda("CALLOK " + c.addr + " " + c.id);
    c.stato = "attiva"; c.inizio = Date.now();
    disegna();
    await avviaAudio();
  }
  function rifiuta() {
    if (!c) return;
    if (c.id) manda("CALLNO " + c.addr + " " + c.id);
    chiudi(c.stato === "squilla" ? "" : "Chiamata rifiutata");
  }
  function fine(motivo) {
    if (!c) return;
    const era = c;
    if (era.id) manda("CALLEND " + era.addr + " " + era.id);
    chiudi(motivo || ("Chiamata terminata" + (era.stato === "attiva" ? " · " + durata((Date.now() - era.inizio) / 1000) : "")));
  }
  function chiudi(motivo) {
    const addr = c && c.addr;
    c = null; fermaAudio(); disegna();
    if (motivo && addr === S.attivo) avviso("📞 " + motivo); else if (motivo) toast(motivo);
  }

  // --- voce --------------------------------------------------------------------------------
  async function avviaAudio() {
    const addr = c.addr;
    try {
      if (HOST && HOST.chiamata) HOST.chiamata(true);            // l'app: audio da chiamata (auricolare)
      a = { seq: 0, ultimo: 0, ricevuti: 0, persi: 0, prossimo: 0, ts: 0 };
      a.ctx = new AudioContext({ sampleRate: 48000, latencyHint: "interactive" });
      a.gain = a.ctx.createGain(); a.gain.connect(a.ctx.destination);
      a.dec = new AudioDecoder({ output: suona, error: () => {} });
      a.dec.configure({ codec: "opus", sampleRate: 48000, numberOfChannels: 1 });
      a.mic = await navigator.mediaDevices.getUserMedia({ audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true, channelCount: 1 } });
      if (!c || c.addr !== addr) return fermaAudio();
      await a.ctx.audioWorklet.addModule("/microfono-worklet.js");
      a.enc = new AudioEncoder({
        output: (pezzo) => { const b = new Uint8Array(pezzo.byteLength); pezzo.copyTo(b); if (!muto && c && c.stato === "attiva") manda("AUD " + c.addr + " " + (++a.seq) + " " + hex(b)); },
        error: () => {},
      });
      a.enc.configure({ codec: "opus", sampleRate: 48000, numberOfChannels: 1, bitrate: 24000, opus: { frameDuration: 20000 } });
      a.nodo = new AudioWorkletNode(a.ctx, "microfono");
      a.nodo.port.onmessage = (e) => {
        if (!a || !a.enc || a.enc.state !== "configured") return;
        const d = new AudioData({ format: "f32-planar", sampleRate: 48000, numberOfFrames: e.data.length, numberOfChannels: 1, timestamp: a.ts, data: e.data });
        a.ts += 20000; a.enc.encode(d); d.close();
      };
      a.ctx.createMediaStreamSource(a.mic).connect(a.nodo);
    } catch (e) {
      fine("⚠ Microfono non disponibile: " + (e.message || e) + " (permesso negato?)");
    }
  }
  function ricevi(seq, h) {
    if (!a || !a.dec || a.dec.state !== "configured") return;
    if (seq <= a.ultimo) return;                              // arrivato dopo uno più nuovo: non serve
    if (a.ultimo && seq > a.ultimo + 1) a.persi += seq - a.ultimo - 1;
    a.ultimo = seq; a.ricevuti++;
    try { a.dec.decode(new EncodedAudioChunk({ type: "key", timestamp: seq * 20000, data: daHexB(h) })); } catch (e) { /* pacchetto rovinato */ }
  }
  function suona(dati) {
    if (!a || !a.ctx) return dati.close();
    const f = new Float32Array(dati.numberOfFrames);
    dati.copyTo(f, { planeIndex: 0, format: "f32-planar" });
    const buf = a.ctx.createBuffer(1, dati.numberOfFrames, dati.sampleRate); dati.close();
    buf.copyToChannel(f, 0);
    const s = a.ctx.createBufferSource(); s.buffer = buf; s.connect(a.gain);
    const ora = a.ctx.currentTime;
    if (a.prossimo < ora + 0.02 || a.prossimo > ora + 0.3) a.prossimo = ora + 0.06;   // margine piccolo, niente accumulo
    s.start(a.prossimo); a.prossimo += buf.duration;
    a.suonati = (a.suonati || 0) + 1;
  }
  function fermaAudio() {
    if (!a) return;
    try { if (a.mic) a.mic.getTracks().forEach((t) => t.stop()); } catch (e) { /* niente */ }
    try { if (a.enc && a.enc.state !== "closed") a.enc.close(); } catch (e) { /* niente */ }
    try { if (a.dec && a.dec.state !== "closed") a.dec.close(); } catch (e) { /* niente */ }
    try { if (a.ctx) a.ctx.close(); } catch (e) { /* niente */ }
    a = null;
    if (HOST && HOST.chiamata) HOST.chiamata(false);
  }

  // --- tasti -------------------------------------------------------------------------------
  tMuto.onclick = () => { muto = !muto; disegna(); };
  tViva.onclick = () => {
    vivavoce = !vivavoce;
    if (HOST && HOST.vivavoce) HOST.vivavoce(vivavoce);
    else toast(vivavoce ? "Vivavoce: scegli l'altoparlante dal telefono" : "Vivavoce spento");
    disegna();
  };
  tFine.onclick = () => fine();
  tNo.onclick = () => rifiuta();
  tSi.onclick = () => rispondi();
  riduci.onclick = () => { ridotta = true; disegna(); };
  mini.onclick = () => { ridotta = false; disegna(); };

  // --- eventi del motore ------------------------------------------------------------------
  function evento(t, p) {
    if (t === "AUD") { if (c && c.stato === "attiva" && p[1] === c.addr) ricevi(+p[2], p[3] || ""); return true; }
    if (!["CALLID", "CALLIN", "CALLOK", "CALLNO", "CALLEND"].includes(t) || p.length < 3) return false;
    const addr = p[1], id = p[2];
    if (t === "CALLID" && c && c.addr === addr && !c.id) c.id = id;
    else if (t === "CALLIN") {
      if (c && !(c.stato === "squilla" && c.id === id)) { manda("CALLNO " + addr + " " + id); return true; }   // occupato
      c = { addr, id, stato: "squilla", inizio: Date.now() }; ridotta = false; muto = false; vivavoce = false;
      avviaTimer(); disegna();
      if (navigator.vibrate) navigator.vibrate([400, 200, 400]);
    } else if (t === "CALLOK" && c && c.id === id && c.stato === "chiamo") {
      c.stato = "attiva"; c.inizio = Date.now(); disegna(); avviaAudio();
    } else if (t === "CALLNO" && c && c.id === id) { c.id = null; chiudi("Chiamata rifiutata o occupato"); }
    else if (t === "CALLEND" && c && c.id === id) {
      const persa = c.stato === "squilla"; c.id = null;
      chiudi(persa ? "Chiamata persa" : "Chiamata terminata");
    }
    return true;
  }
  function offline(addr) { if (c && c.addr === addr && c.stato === "attiva") { c.id = null; chiudi("Chiamata caduta: il contatto non è più in linea"); } }

  return { chiama, evento, offline, disponibile, _stato: () => ({ c, a, muto, vivavoce, ridotta }), _ricevi: ricevi };
})();
$("btn-chiama").onclick = () => S.attivo && Chiamate.chiama(S.attivo);
