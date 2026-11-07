// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
//
// Nyctalus Browser · schede 🦇: IMPRONTA UNIFORME. Iniettato all'avvio di ogni pagina e di ogni
// cornice, prima degli script del sito. Obiettivo: due utenti di Nyctalus su PC diversi devono
// risultare IDENTICI a un sito (il sito sa già che arrivano da Nyctalus: l'uscita è un nodo).
// Valori di tutti: ora UTC, lingua en-US, macOS/Safari (coerente con il motore WebKit),
// 2 processori, schermo arrotondato, niente lettura delle immagini disegnate (canvas).
// Limiti onesti: i Web Worker non vedono queste modifiche; un sito molto determinato può
// riconoscere che i valori sono «coperti». Per questo per i casi delicati c'è Nyctalus Firefox.
(function () {
  "use strict";
  if (window.__nyctImpronta) return;
  Object.defineProperty(window, "__nyctImpronta", { value: true });

  // le funzioni sostituite devono sembrare quelle originali anche a chi le ispeziona
  const nativi = new WeakMap();
  const toStringOrig = Function.prototype.toString;
  const toStringNuovo = function toString() {
    if (nativi.has(this)) return "function " + nativi.get(this) + "() {\n    [native code]\n}";
    return toStringOrig.call(this);
  };
  nativi.set(toStringNuovo, "toString");
  Function.prototype.toString = toStringNuovo;
  const finto = (f, nome) => { nativi.set(f, nome); return f; };
  const getter = (oggetto, prop, valore) => {
    try {
      const g = finto(function () { return typeof valore === "function" ? valore.call(this) : valore; }, "get " + prop);
      Object.defineProperty(oggetto, prop, { get: g, configurable: true, enumerable: true });
    } catch (e) { /* proprietà non ridefinibile: si lascia */ }
  };

  // --- identità: Safari su macOS, come il motore WebKit -------------------------------------
  const N = Navigator.prototype;
  getter(N, "platform", "MacIntel");
  getter(N, "language", "en-US");
  getter(N, "languages", Object.freeze(["en-US", "en"]));
  getter(N, "hardwareConcurrency", 2);
  getter(N, "maxTouchPoints", 0);
  getter(N, "doNotTrack", null);
  if ("deviceMemory" in N) getter(N, "deviceMemory", 8);
  if ("connection" in N) getter(N, "connection", undefined);
  if ("getBattery" in N) N.getBattery = finto(function getBattery() { return Promise.reject(new DOMException("", "NotAllowedError")); }, "getBattery");

  // --- schermo: misure arrotondate e uguali fra schermo e finestra --------------------------
  const arrotonda = (v, passo) => Math.max(passo, Math.floor(v / passo) * passo);
  const larg = () => arrotonda(window.innerWidth || 1000, 200);
  const alt = () => arrotonda(window.innerHeight || 700, 100);
  const S = Screen.prototype;
  getter(S, "width", larg); getter(S, "height", alt);
  getter(S, "availWidth", larg); getter(S, "availHeight", alt);
  getter(S, "availLeft", 0); getter(S, "availTop", 0);
  getter(S, "colorDepth", 24); getter(S, "pixelDepth", 24);
  getter(window, "devicePixelRatio", 1);
  getter(window, "outerWidth", () => window.innerWidth);
  getter(window, "outerHeight", () => window.innerHeight);
  getter(window, "screenX", 0); getter(window, "screenY", 0);
  getter(window, "screenLeft", 0); getter(window, "screenTop", 0);

  // --- ora: sempre UTC, qualunque sia il fuso del PC ----------------------------------------
  const D = Date, P = D.prototype;
  const coppie = [["getHours", "getUTCHours"], ["getMinutes", "getUTCMinutes"], ["getSeconds", "getUTCSeconds"],
    ["getMilliseconds", "getUTCMilliseconds"], ["getDate", "getUTCDate"], ["getDay", "getUTCDay"],
    ["getMonth", "getUTCMonth"], ["getFullYear", "getUTCFullYear"], ["setHours", "setUTCHours"],
    ["setMinutes", "setUTCMinutes"], ["setSeconds", "setUTCSeconds"], ["setMilliseconds", "setUTCMilliseconds"],
    ["setDate", "setUTCDate"], ["setMonth", "setUTCMonth"], ["setFullYear", "setUTCFullYear"]];
  for (const [locale, utc] of coppie) {
    const f = P[utc];
    P[locale] = finto(function () { return f.apply(this, arguments); }, locale);
  }
  P.getTimezoneOffset = finto(function getTimezoneOffset() { return 0; }, "getTimezoneOffset");
  const DTF = Intl.DateTimeFormat;
  const conUTC = (o) => Object.assign({}, o || {}, (o && o.timeZone) ? {} : { timeZone: "UTC" });
  const NuovoDTF = finto(function DateTimeFormat(l, o) {
    return new.target ? Reflect.construct(DTF, ["en-US", conUTC(o)], new.target) : DTF("en-US", conUTC(o));
  }, "DateTimeFormat");
  NuovoDTF.prototype = DTF.prototype;
  NuovoDTF.supportedLocalesOf = DTF.supportedLocalesOf;
  Intl.DateTimeFormat = NuovoDTF;
  const formatta = (d, o) => DTF("en-US", conUTC(o)).format(d);
  P.toLocaleString = finto(function toLocaleString(l, o) { return formatta(this, Object.assign({ year: "numeric", month: "numeric", day: "numeric", hour: "numeric", minute: "2-digit", second: "2-digit" }, o)); }, "toLocaleString");
  P.toLocaleDateString = finto(function toLocaleDateString(l, o) { return formatta(this, Object.assign({ year: "numeric", month: "numeric", day: "numeric" }, o)); }, "toLocaleDateString");
  P.toLocaleTimeString = finto(function toLocaleTimeString(l, o) { return formatta(this, Object.assign({ hour: "numeric", minute: "2-digit", second: "2-digit" }, o)); }, "toLocaleTimeString");
  const testo = P.toUTCString;
  const giorni = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"], mesi = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  const due = (n) => String(n).padStart(2, "0");
  const parteData = (d) => giorni[d.getUTCDay()] + " " + mesi[d.getUTCMonth()] + " " + due(d.getUTCDate()) + " " + d.getUTCFullYear();
  const parteOra = (d) => due(d.getUTCHours()) + ":" + due(d.getUTCMinutes()) + ":" + due(d.getUTCSeconds()) + " GMT+0000 (Coordinated Universal Time)";
  P.toString = finto(function toString() { return isNaN(this) ? "Invalid Date" : parteData(this) + " " + parteOra(this); }, "toString");
  P.toDateString = finto(function toDateString() { return isNaN(this) ? "Invalid Date" : parteData(this); }, "toDateString");
  P.toTimeString = finto(function toTimeString() { return isNaN(this) ? "Invalid Date" : parteOra(this); }, "toTimeString");
  void testo;
  // new Date(anno, mese, …) e Date() vanno letti in UTC
  const NuovaData = finto(function Date() {
    if (!new.target) return new D().toString();
    const a = arguments;
    if (a.length > 1) return Reflect.construct(D, [D.UTC.apply(null, a)], new.target);
    return Reflect.construct(D, Array.prototype.slice.call(a), new.target);
  }, "Date");
  NuovaData.prototype = P;
  NuovaData.now = D.now; NuovaData.UTC = D.UTC; NuovaData.parse = D.parse;
  Object.defineProperty(P, "constructor", { value: NuovaData, writable: true, configurable: true });
  window.Date = NuovaData;

  // --- immagini disegnate (canvas): niente lettura, uguale per tutti ------------------------
  const C = HTMLCanvasElement.prototype;
  const vuota = (w, h) => { const c = document.createElement("canvas"); c.width = w; c.height = h; return c; };
  const daURL = C.toDataURL, aBlob = C.toBlob;
  C.toDataURL = finto(function toDataURL() { return daURL.apply(vuota(this.width, this.height), arguments); }, "toDataURL");
  C.toBlob = finto(function toBlob() { return aBlob.apply(vuota(this.width, this.height), arguments); }, "toBlob");
  const C2 = CanvasRenderingContext2D.prototype;
  C2.getImageData = finto(function getImageData(x, y, w, h) { return new ImageData(Math.max(1, Math.abs(w)), Math.max(1, Math.abs(h))); }, "getImageData");
  C2.measureText = (function (orig) {
    return finto(function measureText(t) {
      const m = orig.call(this, t);
      // larghezze arrotondate: le piccole differenze dei caratteri installati non si vedono
      return { width: Math.round(m.width), actualBoundingBoxLeft: 0, actualBoundingBoxRight: Math.round(m.width),
        actualBoundingBoxAscent: 0, actualBoundingBoxDescent: 0, fontBoundingBoxAscent: 0, fontBoundingBoxDescent: 0 };
    }, "measureText");
  })(C2.measureText);
})();
