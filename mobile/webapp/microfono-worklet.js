// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Daniele Deplano (RedRider21). Parte di Nyctalus Protocol.
// Microfono delle chiamate: raccoglie blocchi da 20 ms (960 campioni a 48 kHz) da passare al
// codificatore Opus della pagina.
class Microfono extends AudioWorkletProcessor {
  constructor() { super(); this.b = new Float32Array(960); this.n = 0; }
  process(ingressi) {
    const canale = ingressi[0] && ingressi[0][0];
    if (!canale) return true;
    for (let i = 0; i < canale.length; i++) {
      this.b[this.n++] = canale[i];
      if (this.n === 960) { this.port.postMessage(this.b, [this.b.buffer]); this.b = new Float32Array(960); this.n = 0; }
    }
    return true;
  }
}
registerProcessor("microfono", Microfono);
