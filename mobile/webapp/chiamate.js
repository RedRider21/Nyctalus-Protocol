// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
// Chiamate e videochiamate nella chat del telefono (stile Telegram). Segnali, voce e immagini
// passano dal motore dentro la sessione cifrata con il contatto, attraverso la rete Nyctalus
// (niente collegamento diretto: nessuno vede l'IP dell'altro).
//   →  CALL addr [v] · CALLOK|CALLNO|CALLEND addr id · AUD addr seq hex
//      VID addr seq key hex · VIDKEY addr · VIDSTATO addr 0|1
//   ←  CALLID · CALLIN addr id [v] · CALLOK · CALLNO · CALLEND addr id · AUD addr seq hex
//      VID addr seq key hex · VIDKEY addr · VIDSTATO addr 0|1
// Voce: microfono → blocchi da 20 ms (worklet) → Opus (WebCodecs) → AUD; AUD → Opus → altoparlante
// con un piccolo margine; i pacchetti in ritardo si scartano (stesso formato della chat del PC).
// Video: telecamera → 320×240 (o 240×320 in verticale) a 15 immagini/s → VP8 ~200 kbit/s → VID;
// un'immagine completa ogni 2 s o quando l'altro la chiede (VIDKEY), così si riparte dopo un buco.
"use strict";
const Chiamate = (() => {
  const SQUILLO = 45000;
  let c = null;                 // {addr, id, stato: "chiamo"|"squilla"|"attiva", inizio, video}
  // «Rispondi» toccato nella notifica (app chiusa): si risponde appena la chiamata compare
  let rispondiA = location.hash.startsWith("#rispondi=") ? location.hash.slice(10) : "";
  let a = null;                 // audio: {ctx, mic, nodo, enc, dec, gain, prossimo, seq, ultimo, ricevuti, persi}
  let v = null;                 // video: {flusso, enc, dec, giro, seq, ultimo, serveKey, ...}
  let muto = false, vivavoce = false, ridotta = false, timer = 0;
  let micPronto = null, camPronta = null;   // chiesti PRIMA di chiamare/rispondere (permessi di Android)
  let remCam = false;           // l'altro ha la telecamera accesa
  let principale = "remoto";    // chi occupa lo schermo intero quando ci sono due video
  let quieto = false;           // video: comandi nascosti (si tocca lo schermo per riaverli)
  const MUTA_MAX = 15;          // secondi senza voce dall'altro: la chiamata si chiude
  const VINCOLI = { audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: false, channelCount: 1 } };
  // autoGainControl: NO — nelle pause alzava sempre più il rumore di fondo (suono metallico)
  const VIDEO_MAX = 60000, OGNI_KEY = 30;
  // Qualità di chiamate/videochiamate, dalle Impostazioni → tab «Chiamate» (si applica alla prossima chiamata)
  let FPS = 15, VIDEO_KBPS = 200, AUDIO_KBPS = 24, RIS_CAM = 480;
  const leggiQualita = () => {
    const q = (typeof salvato === "function" ? salvato("chiamate", {}) : {}) || {};
    FPS = [10, 15, 20, 30].includes(q.fps) ? q.fps : 15;
    VIDEO_KBPS = q.videoKbps >= 80 && q.videoKbps <= 1500 ? q.videoKbps : 200;
    AUDIO_KBPS = q.audioKbps >= 10 && q.audioKbps <= 64 ? q.audioKbps : 24;
    RIS_CAM = [360, 480, 720].includes(q.risoluzione) ? q.risoluzione : 480;
  };
  leggiQualita();

  // --- icone (tratto sottile, colore del testo) --------------------------------------------
  const ICONE = {
    mic: '<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21"/>',
    micno: '<rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21M4 4l16 16"/>',
    alto: '<path d="M4 9.5h3.5L12 6v12l-4.5-3.5H4z"/><path d="M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11"/>',
    video: '<rect x="3" y="6.5" width="12.5" height="11" rx="2.8"/><path d="M15.5 10.5l5.5-3v9l-5.5-3z"/>',
    tel: '<path d="M6.6 3.5h2.6l1.7 4.6-2.2 1.4a11.5 11.5 0 0 0 5.8 5.8l1.4-2.2 4.6 1.7v2.6a2 2 0 0 1-2 2A16.5 16.5 0 0 1 4.6 5.5a2 2 0 0 1 2-2z"/>',
    gira: '<path d="M4 8.5h3l1.8-2.5h6.4L17 8.5h3V19H4z"/><path d="M9 13a3 3 0 0 1 5.4-1.8M15 13.5a3 3 0 0 1-5.4 1.8"/><path d="M14.6 9.6v1.8h-1.8M9.4 17.2v-1.8h1.8"/>',
    giu: '<path d="M6 9.5l6 6 6-6"/>',
  };
  const svg = (n) => '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">' + ICONE[n] + "</svg>";

  // --- schermata della chiamata ----------------------------------------------------------
  const sch = el("div", "chiamata nascosto");
  const scena = el("div", "ch-scena");
  const remBox = el("div", "ch-vid remoto nascosto"), remFondo = document.createElement("canvas"), remTela = document.createElement("canvas");
  remFondo.className = "ch-fondo-sfocato"; remBox.append(remFondo, remTela);
  const locBox = el("div", "ch-vid locale nascosto"), locVideo = document.createElement("video");
  locVideo.muted = true; locVideo.playsInline = true; locVideo.autoplay = true; locVideo.setAttribute("playsinline", "");
  locBox.append(locVideo);
  scena.append(remBox, locBox);
  const riduci = el("button", "ch-riduci"); riduci.innerHTML = svg("giu"); riduci.title = "Riduci: la chiamata continua";
  const av = el("div", "ch-av"), nome = el("div", "ch-nome"), stato = el("div", "ch-stato"), qual = el("div", "ch-qualita");
  const info = el("div", "ch-info"); info.append(av, nome, stato, qual);
  const tasto = (ic, testo, cls) => {
    const b = el("button", "ch-tasto " + (cls || "")), i = el("span", "ic"), t = el("span", "et", testo);
    i.innerHTML = svg(ic); b.append(i, t);
    b.cambia = (ic2, testo2) => { if (b.dataset.ic !== ic2) { i.innerHTML = svg(ic2); b.dataset.ic = ic2; } t.textContent = testo2; };
    b.dataset.ic = ic;
    return b;
  };
  const tViva = tasto("alto", "Vivavoce", "ch-viva"), tVideo = tasto("video", "Video", "ch-video"), tGira = tasto("gira", "Gira", "ch-gira");
  const tMuto = tasto("mic", "Muto", "ch-muto"), tFine = tasto("tel", "Termina", "rosso ch-fine");
  const tasti = el("div", "ch-tasti"); tasti.append(tViva, tVideo, tGira, tMuto, tFine);
  const tNo = tasto("tel", "Rifiuta", "rosso ch-fine"), tVoce = tasto("tel", "Solo voce", "ch-solovoce"), tSi = tasto("tel", "Rispondi", "verde ch-si");
  const arrivo = el("div", "ch-tasti ch-arrivo"); arrivo.append(tNo, tVoce, tSi);
  sch.append(scena, riduci, info, tasti, arrivo);
  const mini = el("button", "ch-mini nascosto");
  document.body.append(sch, mini);

  const disponibile = () => !!(navigator.mediaDevices && navigator.mediaDevices.getUserMedia && window.AudioEncoder && window.AudioDecoder && window.AudioWorkletNode);
  const videoDisponibile = () => disponibile() && !!(window.VideoEncoder && window.VideoDecoder && window.VideoFrame);
  const nomeDi = (addr) => S.contatti.get(addr) || addr.slice(0, 10);
  const durata = (s) => { s = Math.floor(s); return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0"); };
  const hex = (b) => { let s = ""; for (let i = 0; i < b.length; i++) s += (b[i] < 16 ? "0" : "") + b[i].toString(16); return s; };
  const daHexB = (h) => { const b = new Uint8Array(h.length >> 1); for (let i = 0; i < b.length; i++) b[i] = parseInt(h.substr(i * 2, 2), 16); return b; };

  // squillo: suoneria e vibrazione di sistema (app) per la chiamata in arrivo; tono d'attesa
  // «tuu… tuu» (425 Hz, 1 s ogni 4 s, come in Italia) per quella in uscita
  let attesa = null;
  function squillo(quale) {
    zitto();
    if (quale === "arrivo") {
      if (HOST && HOST.squilla) HOST.squilla(true);
      else if (navigator.vibrate) navigator.vibrate([800, 600, 800, 600, 800]);
    } else if (quale === "uscita") {
      try {
        const ctx = new AudioContext(), o = ctx.createOscillator(), g = ctx.createGain();
        o.frequency.value = 425; g.gain.value = 0; o.connect(g); g.connect(ctx.destination); o.start();
        const t0 = ctx.currentTime + 0.1;
        for (let i = 0; i < 12; i++) { g.gain.setValueAtTime(0.12, t0 + i * 4); g.gain.setValueAtTime(0, t0 + i * 4 + 1); }
        attesa = ctx;
      } catch (e) { /* senza audio: niente tono */ }
    }
  }
  function zitto() {
    if (HOST && HOST.squilla) HOST.squilla(false);
    if (attesa) { try { attesa.close(); } catch (e) { /* niente */ } attesa = null; }
  }

  function disegna() {
    if (!c) { sch.classList.add("nascosto"); mini.classList.add("nascosto"); scenaVideo(); return; }
    const n = nomeDi(c.addr);
    if (av.dataset.addr !== c.addr) { av.textContent = ""; av.appendChild(avatar(c.addr, n, true)); av.dataset.addr = c.addr; }
    nome.textContent = n;
    stato.textContent = c.stato === "chiamo" ? (c.video ? "Videochiamata…" : "Chiamo…")
      : c.stato === "squilla" ? (c.video ? "videochiamata in arrivo" : "ti sta chiamando")
      : durata((Date.now() - c.inizio) / 1000);
    sch.classList.toggle("squilla", c.stato !== "attiva");
    tasti.classList.toggle("nascosto", c.stato === "squilla");
    arrivo.classList.toggle("nascosto", c.stato !== "squilla");
    tVoce.classList.toggle("nascosto", !c.video);
    tSi.cambia(c.video ? "video" : "tel", "Rispondi");
    tMuto.classList.toggle("attivo", muto); tMuto.cambia(muto ? "micno" : "mic", "Muto");
    tViva.classList.toggle("attivo", vivavoce);
    const camAccesa = !!(v && v.flusso);
    tVideo.classList.toggle("nascosto", !videoDisponibile());
    tVideo.cambia(camAccesa ? "tel" : "video", camAccesa ? "Solo voce" : "Video");
    tGira.classList.toggle("nascosto", !camAccesa);
    sch.classList.toggle("nascosto", ridotta);
    mini.classList.toggle("nascosto", !ridotta);
    mini.textContent = (c.video || camAccesa || remCam ? "📹 " : "📞 ") + n + " · " + (c.stato === "attiva" ? durata((Date.now() - c.inizio) / 1000) : "Chiamo…") + " — tocca per tornare";
    scenaVideo();
  }

  // --- disposizione dei video (Telegram): uno a tutto schermo, l'altro nel riquadro ----------
  // Il riquadro si trascina (si aggancia all'angolo più vicino), si tocca per scambiarlo con lo
  // schermo intero, si ingrandisce con due dita (o la rotella) e torna com'era col doppio tocco.
  const pip = { angolo: "ad", scala: 1, el: null };     // angolo: a/b (alto/basso) + s/d (sinistra/destra)
  function scenaVideo() {
    const loc = !!(c && v && v.flusso), rem = !!(c && remCam && c.stato === "attiva");
    const video = loc || rem;
    sch.classList.toggle("video", video);
    sch.classList.toggle("quieto", video && quieto);
    let pieno = null, piccolo = null;
    if (loc && rem) { pieno = principale === "remoto" ? remBox : locBox; piccolo = pieno === remBox ? locBox : remBox; }
    else if (loc) pieno = locBox;
    else if (rem) pieno = remBox;
    for (const b of [remBox, locBox]) {
      b.classList.toggle("nascosto", b !== pieno && b !== piccolo);
      b.classList.toggle("pieno", b === pieno);
      b.classList.toggle("pip", b === piccolo);
      if (b !== piccolo) { b.style.left = b.style.top = b.style.width = b.style.height = ""; }
    }
    pip.el = piccolo;
    if (piccolo) posaPip(true);
  }
  function areaPip() {
    const r = sch.getBoundingClientRect(), su = quieto ? 18 : 84, giu = quieto ? 18 : 150;
    const w = Math.round(Math.min(Math.max(r.width * 0.3 * pip.scala, 84), r.width * 0.62));
    return { r, w, h: Math.round(w * 4 / 3), su, giu, m: 14 };
  }
  function posaPip(animato) {
    const b = pip.el; if (!b) return;
    const A = areaPip();
    const x = pip.angolo[1] === "s" ? A.m : A.r.width - A.w - A.m;
    const y = pip.angolo[0] === "a" ? A.su : Math.max(A.su, A.r.height - A.h - A.giu);
    b.classList.toggle("in-moto", !animato);
    b.style.width = A.w + "px"; b.style.height = A.h + "px"; b.style.left = x + "px"; b.style.top = y + "px";
  }
  function scambia() { principale = principale === "remoto" ? "locale" : "remoto"; scenaVideo(); }
  (function gestiPip() {
    const dita = new Map();
    let base = null, tocco = 0, attesaTocco = 0;
    for (const b of [remBox, locBox]) {
      b.addEventListener("pointerdown", (e) => {
        if (b !== pip.el) return;
        e.stopPropagation();
        try { b.setPointerCapture(e.pointerId); } catch (x) { /* niente */ }
        dita.set(e.pointerId, { x: e.clientX, y: e.clientY });
        const r = b.getBoundingClientRect(), s = sch.getBoundingClientRect();
        if (!base) base = { x0: e.clientX, y0: e.clientY, l: r.left - s.left, t: r.top - s.top, t0: Date.now(), mosso: false };
        if (dita.size === 2) { const [p, q] = [...dita.values()]; base.dist = Math.hypot(p.x - q.x, p.y - q.y); base.scala = pip.scala; base.mosso = true; }
      });
      b.addEventListener("pointermove", (e) => {
        if (b !== pip.el || !dita.has(e.pointerId) || !base) return;
        dita.set(e.pointerId, { x: e.clientX, y: e.clientY });
        if (dita.size >= 2 && base.dist) {             // due dita: ridimensiona
          const [p, q] = [...dita.values()];
          pip.scala = Math.min(Math.max(base.scala * Math.hypot(p.x - q.x, p.y - q.y) / base.dist, 0.6), 2);
          posaPip(false); return;
        }
        const dx = e.clientX - base.x0, dy = e.clientY - base.y0;
        if (!base.mosso && Math.hypot(dx, dy) < 8) return;
        base.mosso = true;
        b.classList.add("in-moto");
        b.style.left = (base.l + dx) + "px"; b.style.top = (base.t + dy) + "px";
      });
      const lascia = (e) => {
        if (!dita.has(e.pointerId)) return;
        dita.delete(e.pointerId);
        if (dita.size || !base) return;
        const era = base; base = null;
        if (b !== pip.el) return;
        if (era.mosso) {                                 // si aggancia all'angolo più vicino
          const r = b.getBoundingClientRect(), s = sch.getBoundingClientRect();
          pip.angolo = (r.top + r.height / 2 < s.top + s.height / 2 ? "a" : "b") + (r.left + r.width / 2 < s.left + s.width / 2 ? "s" : "d");
          posaPip(true);
        } else if (Date.now() - era.t0 < 400) {         // tocco: scambia; doppio tocco: misura normale
          if (tocco && Date.now() - tocco < 320) { clearTimeout(attesaTocco); tocco = 0; pip.scala = 1; posaPip(true); }
          else { tocco = Date.now(); attesaTocco = setTimeout(() => { tocco = 0; scambia(); }, 320); }
        }
      };
      b.addEventListener("pointerup", lascia);
      b.addEventListener("pointercancel", lascia);
      b.addEventListener("click", (e) => e.stopPropagation());
      b.addEventListener("wheel", (e) => {
        if (b !== pip.el) return;
        e.preventDefault(); pip.scala = Math.min(Math.max(pip.scala * (e.deltaY < 0 ? 1.1 : 0.9), 0.6), 2); posaPip(true);
      }, { passive: false });
    }
  })();
  // tocco sul video grande: nasconde/mostra i comandi (come in Telegram)
  scena.addEventListener("click", () => {
    if (!sch.classList.contains("video")) return;
    quieto = !quieto; scenaVideo();
  });
  window.addEventListener("resize", () => { if (pip.el) posaPip(true); });

  function avviaTimer() { if (!timer) timer = setInterval(tick, 1000); }
  function tick() {
    if (!c) { clearInterval(timer); timer = 0; return; }
    if (c.stato !== "attiva" && Date.now() - c.inizio > SQUILLO) {
      if (c.stato === "chiamo") return fine("Nessuna risposta");
      return rifiuta();
    }
    if (a && c.stato === "attiva") {
      const r = a.ricevuti - (a.r0 || 0), p = a.persi - (a.p0 || 0); a.r0 = a.ricevuti; a.p0 = a.persi;
      a.muta = r === 0 ? (a.muta || 0) + 1 : 0;
      if (a.muta >= MUTA_MAX) return fine("Chiamata chiusa: dall'altra parte non arriva più voce");
      const q = r === 0 ? 0 : p / (r + p) < 0.03 ? 3 : p / (r + p) < 0.12 ? 2 : 1;
      qual.textContent = ["niente audio", "linea scarsa", "linea discreta", "linea buona"][q];
      qual.dataset.livello = q;
    } else { qual.textContent = ""; qual.dataset.livello = ""; }
    disegna();
  }

  // --- azioni ----------------------------------------------------------------------------
  // microfono (e telecamera) PRIMA di tutto: se Android deve chiedere il permesso, la chiamata
  // non parte finché non è dato (prima la chiamata partiva, il microfono mancava e si chiudeva)
  async function preparaMicrofono() {
    if (micPronto) return true;
    try { micPronto = await navigator.mediaDevices.getUserMedia(VINCOLI); return true; }
    catch (e) { toast("🎤 Senza il permesso del microfono non si può chiamare (" + (e.name || e) + ")"); return false; }
  }
  const RIS_WH = { 360: [480, 360], 480: [640, 480], 720: [1280, 720] };
  const vincoliCam = (giro) => ({ video: { facingMode: giro, width: { ideal: RIS_WH[RIS_CAM][0] }, height: { ideal: RIS_WH[RIS_CAM][1] }, frameRate: { ideal: FPS, max: 30 } } });
  async function preparaCamera(giro) {
    if (camPronta) return true;
    try { camPronta = await navigator.mediaDevices.getUserMedia(vincoliCam(giro || "user")); return true; }
    catch (e) { toast("📷 Telecamera non disponibile (" + (e.name || e) + "): si continua solo con la voce"); return false; }
  }
  async function chiama(addr, video) {
    if (c) return toast("C'è già una chiamata in corso");
    if (!disponibile()) return toast("Questo telefono non supporta ancora le chiamate (WebView troppo vecchia)");
    if (video && !videoDisponibile()) { toast("Questo telefono non supporta le videochiamate: chiamata vocale"); video = false; }
    if (!(await preparaMicrofono())) return;
    if (video && !(await preparaCamera())) video = false;
    if (c) return;
    c = { addr, id: null, stato: "chiamo", inizio: Date.now(), video: !!video };
    ridotta = false; muto = false; vivavoce = false; remCam = false; principale = "remoto"; quieto = false;
    manda("CALL " + addr + (video ? " v" : ""));
    squillo("uscita");
    avviaTimer(); disegna();
    if (video) await accendiCamera();              // ti vedi mentre squilla (non si manda ancora niente)
  }
  async function rispondi(conVideo) {
    if (!c || c.stato !== "squilla") return;
    zitto();
    if (!(await preparaMicrofono())) return rifiuta();
    if (conVideo && !(await preparaCamera())) conVideo = false;
    if (!c || c.stato !== "squilla") return;
    manda("CALLOK " + c.addr + " " + c.id);
    c.stato = "attiva"; c.inizio = Date.now();
    disegna();
    await avviaAudio();
    if (conVideo) await accendiCamera();
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
    zitto();
    const addr = c && c.addr;
    c = null; fermaVideo(false); fermaAudio(); remCam = false; disegna();
    if (motivo && addr === S.attivo) avviso("📞 " + motivo); else if (motivo) toast(motivo);
  }

  // --- voce --------------------------------------------------------------------------------
  async function avviaAudio() {
    const addr = c.addr;
    try {
      if (HOST && HOST.chiamata) HOST.chiamata(true);            // l'app: audio da chiamata (auricolare)
      a = { seq: 0, ultimo: 0, ricevuti: 0, persi: 0, prossimo: 0, ts: 0 };
      a.ctx = new AudioContext({ sampleRate: 48000, latencyHint: "interactive" });
      // limitatore: se si forma un anello (due apparecchi nella stessa stanza) il suono non sale
      // all'infinito, resta contenuto
      a.limite = a.ctx.createDynamicsCompressor();
      a.limite.threshold.value = -20; a.limite.knee.value = 6; a.limite.ratio.value = 20;
      a.limite.attack.value = 0.003; a.limite.release.value = 0.25;
      a.ingresso = a.ctx.createGain(); a.ingresso.connect(a.limite);
      a.gain = a.ctx.createGain(); a.limite.connect(a.gain);
      await uscitaTelefonata(a);
      a.dec = new AudioDecoder({ output: suona, error: () => {} });
      a.dec.configure({ codec: "opus", sampleRate: 48000, numberOfChannels: 1 });
      a.mic = micPronto || await navigator.mediaDevices.getUserMedia(VINCOLI);
      micPronto = null;
      if (!c || c.addr !== addr) return fermaAudio();
      await a.ctx.audioWorklet.addModule("/microfono-worklet.js");
      a.enc = new AudioEncoder({
        output: (pezzo) => { const b = new Uint8Array(pezzo.byteLength); pezzo.copyTo(b); if (!muto && c && c.stato === "attiva") manda("AUD " + c.addr + " " + (++a.seq) + " " + hex(b)); },
        error: () => {},
      });
      a.enc.configure({ codec: "opus", sampleRate: 48000, numberOfChannels: 1, bitrate: AUDIO_KBPS * 1000, opus: { frameDuration: 20000 } });
      a.nodo = new AudioWorkletNode(a.ctx, "microfono");
      a.nodo.port.onmessage = (e) => {
        if (!a || !a.enc || a.enc.state !== "configured") return;
        const d = new AudioData({ format: "f32-planar", sampleRate: 48000, numberOfFrames: e.data.length, numberOfChannels: 1, timestamp: a.ts, data: e.data });
        a.ts += 20000; a.enc.encode(d); d.close();
      };
      a.ctx.createMediaStreamSource(a.mic).connect(a.nodo);
      // la chiamata è partita: se la telecamera era già accesa (videochiamata), lo si dice all'altro
      if (v && v.flusso && c && c.stato === "attiva") { manda("VIDSTATO " + c.addr + " 1"); v.forzaKey = true; }
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
    const s = a.ctx.createBufferSource(); s.buffer = buf; s.connect(a.ingresso);
    const ora = a.ctx.currentTime;
    if (a.prossimo < ora + 0.02 || a.prossimo > ora + 0.3) a.prossimo = ora + 0.06;   // margine piccolo, niente accumulo
    s.start(a.prossimo); a.prossimo += buf.duration;
    a.suonati = (a.suonati || 0) + 1;
  }
  // La voce dell'altro esce da un collegamento WebRTC INTERNO alla pagina (niente rete): così
  // Android la tratta come telefonata (auricolare/vivavoce veri) e la cancellazione dell'eco la
  // conosce. Con l'uscita «musica» dell'AudioContext il microfono la riprendeva e la rimandava
  // indietro: un'eco che si richiudeva su se stessa, sempre più forte.
  async function uscitaTelefonata(a) {
    try {
      const dest = a.ctx.createMediaStreamDestination();
      a.gain.connect(dest);
      const p1 = new RTCPeerConnection({ iceServers: [] }), p2 = new RTCPeerConnection({ iceServers: [] });
      p1.onicecandidate = (e) => { if (e.candidate) p2.addIceCandidate(e.candidate).catch(() => {}); };
      p2.onicecandidate = (e) => { if (e.candidate) p1.addIceCandidate(e.candidate).catch(() => {}); };
      a.lettore = new Audio(); a.lettore.autoplay = true;
      p2.ontrack = (e) => { a.lettore.srcObject = e.streams[0]; a.lettore.play().catch(() => {}); };
      dest.stream.getTracks().forEach((t) => p1.addTrack(t, dest.stream));
      const offerta = await p1.createOffer(); await p1.setLocalDescription(offerta); await p2.setRemoteDescription(offerta);
      const risposta = await p2.createAnswer(); await p2.setLocalDescription(risposta); await p1.setRemoteDescription(risposta);
      a.p1 = p1; a.p2 = p2; a.uscita = "telefonata";
    } catch (e) {
      a.gain.connect(a.ctx.destination); a.uscita = "diretta";   // senza WebRTC: come prima
    }
  }
  function fermaAudio() {
    if (micPronto) { micPronto.getTracks().forEach((t) => t.stop()); micPronto = null; }
    if (!a) return;
    try { if (a.p1) a.p1.close(); if (a.p2) a.p2.close(); if (a.lettore) { a.lettore.srcObject = null; } } catch (e) { /* niente */ }
    try { if (a.mic) a.mic.getTracks().forEach((t) => t.stop()); } catch (e) { /* niente */ }
    try { if (a.enc && a.enc.state !== "closed") a.enc.close(); } catch (e) { /* niente */ }
    try { if (a.dec && a.dec.state !== "closed") a.dec.close(); } catch (e) { /* niente */ }
    try { if (a.ctx) a.ctx.close(); } catch (e) { /* niente */ }
    a = null;
    if (HOST && HOST.chiamata) HOST.chiamata(false);
  }

  // --- video -------------------------------------------------------------------------------
  function statoVideo() {
    if (!v) v = { seq: 0, ultimo: 0, serveKey: true, chiestaKey: 0, n: 0, ricevute: 0, mandate: 0, giro: "user" };
    return v;
  }
  async function accendiCamera() {
    if (!c || !videoDisponibile()) return;
    const s = statoVideo();
    if (s.flusso) return;
    if (!(await preparaCamera(s.giro))) { disegna(); return; }
    if (!c || !v) { if (camPronta) camPronta.getTracks().forEach((t) => t.stop()); camPronta = null; return; }
    s.flusso = camPronta; camPronta = null;
    locVideo.srcObject = s.flusso;
    locVideo.classList.toggle("specchio", s.giro === "user");
    locVideo.play().catch(() => {});
    s.tela = document.createElement("canvas");
    s.ctx2 = s.tela.getContext("2d", { alpha: false });
    s.forzaKey = true;
    s.cattura = setInterval(scatta, Math.round(1000 / FPS));
    if (!vivavoce && HOST && HOST.vivavoce) { vivavoce = true; HOST.vivavoce(true); }   // video: vivavoce, come Telegram
    if (c.stato === "attiva" && a) manda("VIDSTATO " + c.addr + " 1");
    disegna();
  }
  function codificatore(w, h) {
    const s = v;
    try { if (s.enc && s.enc.state !== "closed") s.enc.close(); } catch (e) { /* niente */ }
    s.enc = new VideoEncoder({
      output: (pezzo) => {
        if (!c || c.stato !== "attiva" || !v || !v.flusso) return;
        if (pezzo.byteLength > VIDEO_MAX) { v.forzaKey = true; return; }   // troppo grande: si salta
        const b = new Uint8Array(pezzo.byteLength); pezzo.copyTo(b);
        manda("VID " + c.addr + " " + (++v.seq) + " " + (pezzo.type === "key" ? 1 : 0) + " " + hex(b));
        v.mandate++;
      },
      error: () => { if (v) v.enc = null; },
    });
    s.enc.configure({ codec: "vp8", width: w, height: h, bitrate: VIDEO_KBPS * 1000, framerate: FPS, latencyMode: "realtime" });
    s.w = w; s.h = h; s.forzaKey = true;
  }
  function scatta() {
    const s = v;
    if (!s || !s.flusso || !c || c.stato !== "attiva" || !a) return;   // squilla: ti vedi, ma non si manda niente
    const vw = locVideo.videoWidth, vh = locVideo.videoHeight;
    if (locVideo.readyState < 2 || !vw || !vh) return;
    const verticale = vh > vw, w = verticale ? 240 : 320, h = verticale ? 320 : 240;
    if (!s.enc || s.w !== w || s.h !== h) codificatore(w, h);
    if (s.enc.state !== "configured" || s.enc.encodeQueueSize > 2) return;
    if (s.tela.width !== w || s.tela.height !== h) { s.tela.width = w; s.tela.height = h; }
    // ritaglio al centro («cover»), come si vede nel riquadro
    const k = Math.max(w / vw, h / vh), dw = vw * k, dh = vh * k;
    s.ctx2.drawImage(locVideo, (w - dw) / 2, (h - dh) / 2, dw, dh);
    const f = new VideoFrame(s.tela, { timestamp: s.n * Math.round(1e6 / FPS) });
    const key = s.forzaKey || s.n % OGNI_KEY === 0;
    s.forzaKey = false; s.n++;
    try { s.enc.encode(f, { keyFrame: key }); } catch (e) { s.enc = null; }
    f.close();
  }
  function spegniCamera() {
    if (!v || !v.flusso) return;
    if (c && c.stato === "attiva") manda("VIDSTATO " + c.addr + " 0");
    fermaVideo(true);
    disegna();
  }
  /** Spegne la telecamera; con «tieniRicezione» il video dell'altro continua ad arrivare. */
  function fermaVideo(tieniRicezione) {
    if (camPronta) { camPronta.getTracks().forEach((t) => t.stop()); camPronta = null; }
    if (!v) return;
    clearInterval(v.cattura); v.cattura = 0;
    try { if (v.flusso) v.flusso.getTracks().forEach((t) => t.stop()); } catch (e) { /* niente */ }
    v.flusso = null; locVideo.srcObject = null;
    try { if (v.enc && v.enc.state !== "closed") v.enc.close(); } catch (e) { /* niente */ }
    v.enc = null;
    if (!tieniRicezione) {
      try { if (v.dec && v.dec.state !== "closed") v.dec.close(); } catch (e) { /* niente */ }
      v = null;
    }
  }
  async function giraCamera() {
    if (!v || !v.flusso) return;
    const nuovo = v.giro === "user" ? "environment" : "user";
    try {
      const f = await navigator.mediaDevices.getUserMedia(vincoliCam(nuovo));
      if (!v || !v.flusso) { f.getTracks().forEach((t) => t.stop()); return; }
      v.flusso.getTracks().forEach((t) => t.stop());
      v.flusso = f; v.giro = nuovo; locVideo.srcObject = f; locVideo.play().catch(() => {});
      locVideo.classList.toggle("specchio", nuovo === "user");
      v.forzaKey = true;
    } catch (e) { toast("📷 Non riesco a cambiare telecamera"); }
  }
  function decodificatore() {
    const s = statoVideo();
    try { if (s.dec && s.dec.state !== "closed") s.dec.close(); } catch (e) { /* niente */ }
    s.dec = new VideoDecoder({ output: mostra, error: () => { if (v) { v.dec = null; v.serveKey = true; } } });
    s.dec.configure({ codec: "vp8", optimizeForLatency: true });
    s.serveKey = true;
  }
  function chiediKey() {
    const s = statoVideo();
    if (!c || Date.now() - s.chiestaKey < 1000) return;
    s.chiestaKey = Date.now(); manda("VIDKEY " + c.addr);
  }
  function riceviVideo(seq, key, h) {
    if (!c || c.stato !== "attiva") return;
    const s = statoVideo();
    if (!s.dec || s.dec.state === "closed") decodificatore();
    if (seq <= s.ultimo) return;
    if (s.ultimo && seq > s.ultimo + 1) s.serveKey = true;    // buco: si aspetta un'immagine completa
    s.ultimo = seq;
    if (s.serveKey && !key) return chiediKey();
    s.serveKey = false;
    if (!remCam) { remCam = true; disegna(); }
    try { s.dec.decode(new EncodedVideoChunk({ type: key ? "key" : "delta", timestamp: seq * Math.round(1e6 / FPS), data: daHexB(h) })); }
    catch (e) { s.serveKey = true; chiediKey(); }
  }
  const fondo2 = remFondo.getContext("2d"), tela2 = remTela.getContext("2d", { alpha: false });
  function mostra(f) {
    if (!v) return f.close();
    const w = f.displayWidth, h = f.displayHeight;
    if (remTela.width !== w || remTela.height !== h) { remTela.width = w; remTela.height = h; remFondo.width = 48; remFondo.height = Math.round(48 * h / w); }
    tela2.drawImage(f, 0, 0);
    if ((v.ricevute++ & 7) === 0) fondo2.drawImage(remTela, 0, 0, remFondo.width, remFondo.height);   // sfondo sfocato
    f.close();
  }

  // --- tasti -------------------------------------------------------------------------------
  tMuto.onclick = () => { muto = !muto; disegna(); };
  tViva.onclick = () => {
    vivavoce = !vivavoce;
    if (HOST && HOST.vivavoce) HOST.vivavoce(vivavoce);
    else toast(vivavoce ? "Vivavoce: scegli l'altoparlante dal telefono" : "Vivavoce spento");
    disegna();
  };
  tVideo.onclick = () => { if (v && v.flusso) spegniCamera(); else accendiCamera(); };
  tGira.onclick = () => giraCamera();
  tFine.onclick = () => fine();
  tNo.onclick = () => rifiuta();
  tSi.onclick = () => rispondi(!!(c && c.video));
  tVoce.onclick = () => rispondi(false);
  riduci.onclick = () => { ridotta = true; disegna(); };
  mini.onclick = () => { ridotta = false; disegna(); };

  // --- eventi del motore ------------------------------------------------------------------
  function evento(t, p) {
    if (t === "AUD") { if (c && c.stato === "attiva" && p[1] === c.addr) ricevi(+p[2], p[3] || ""); return true; }
    if (t === "VID") { if (c && p[1] === c.addr && p.length >= 5) riceviVideo(+p[2], p[3] === "1", p[4]); return true; }
    if (t === "VIDKEY") { if (c && p[1] === c.addr && v) v.forzaKey = true; return true; }
    if (t === "VIDSTATO") {
      if (c && p[1] === c.addr) {
        remCam = p[2] === "1";
        const s = statoVideo(); s.serveKey = true; s.ultimo = 0;
        if (remCam) { s.chiestaKey = 0; chiediKey(); }
        disegna();
      }
      return true;
    }
    if (!["CALLID", "CALLIN", "CALLOK", "CALLNO", "CALLEND"].includes(t) || p.length < 3) return false;
    const addr = p[1], id = p[2];
    if (t === "CALLID" && c && c.addr === addr && !c.id) c.id = id;
    else if (t === "CALLIN") {
      if (c && c.addr === addr && c.id !== id) { c.id = null; chiudi(""); }   // ci richiama: la vecchia era morta
      if (c && !(c.stato === "squilla" && c.id === id)) { manda("CALLNO " + addr + " " + id); return true; }   // occupato
      c = { addr, id, stato: "squilla", inizio: Date.now(), video: p[3] === "v" && videoDisponibile() };
      ridotta = false; muto = false; vivavoce = false; remCam = false; principale = "remoto"; quieto = false;
      avviaTimer(); disegna();
      if (rispondiA === addr) { rispondiA = ""; rispondi(c.video); return true; }   // dalla notifica
      squillo("arrivo");
    } else if (t === "CALLOK" && c && c.id === id && c.stato === "chiamo") {
      zitto();
      c.stato = "attiva"; c.inizio = Date.now(); disegna(); avviaAudio();
    } else if (t === "CALLNO" && c && c.id === id) { c.id = null; chiudi("Chiamata rifiutata o occupato"); }
    else if (t === "CALLEND" && c && c.id === id) {
      const persa = c.stato === "squilla"; c.id = null;
      chiudi(persa ? "Chiamata persa" : "Chiamata terminata");
    }
    return true;
  }
  // dalla notifica: se sta già squillando si risponde subito, altrimenti appena arriva il CALLIN
  function rispondiDaNotifica(addr) {
    if (c && c.addr === addr && c.stato === "squilla") return rispondi(c.video);
    rispondiA = addr;
    setTimeout(() => { if (rispondiA === addr) rispondiA = ""; }, 60000);   // non oltre lo squillo
  }
  function offline(addr) { if (c && c.addr === addr && c.stato === "attiva") { c.id = null; chiudi("Chiamata caduta: il contatto non è più in linea"); } }

  return {
    chiama, evento, offline, disponibile, videoDisponibile, rispondiDaNotifica, aggiornaQualita: leggiQualita,
    _stato: () => ({ c, a, v, muto, vivavoce, ridotta, remCam, principale, quieto, pip: { angolo: pip.angolo, scala: pip.scala } }),
    _ricevi: ricevi, _scambia: scambia,
  };
})();
window.nyctRispondiChiamata = (addr) => { if (/^[a-z2-7]{56}\.nyct$/.test(addr)) Chiamate.rispondiDaNotifica(addr); };
