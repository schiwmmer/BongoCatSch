// The wasm bridge, wrapped so the rest of the page never touches a pointer.
//
// The module exports six `extern "C"` functions and one shared input buffer.
// Every call is: encode the argument into that buffer, call, decode the output
// buffer. Memory grows between calls, so the views are rebuilt every time.

const SAVE_KEY = 'bongocat.pomodoro.web.v1';

export class PomodoroWasm {
  constructor(exports) {
    this.ex = exports;
    this.encoder = new TextEncoder();
    this.decoder = new TextDecoder();
  }

  static async load(url) {
    const response = await fetch(url);
    const bytes = await response.arrayBuffer();
    const { instance } = await WebAssembly.instantiate(bytes, {});
    return new PomodoroWasm(instance.exports);
  }

  get view() {
    return new Uint8Array(this.ex.memory.buffer);
  }

  // Copy `text` into the shared buffer and answer how long it is.
  write(text) {
    const bytes = this.encoder.encode(text);
    const pointer = this.ex.web_input_ptr();
    this.view.set(bytes, pointer);
    return bytes.length;
  }

  read() {
    const pointer = this.ex.web_output_ptr();
    const length = this.ex.web_output_len();
    return this.decoder.decode(this.view.slice(pointer, pointer + length));
  }

  // The offset the cultivation page reads its "day" from, in seconds.
  static get utcOffsetSeconds() {
    return -new Date().getTimezoneOffset() * 60;
  }

  static now() {
    return Date.now() / 1000;
  }

  boot(documentText) {
    const length = documentText ? this.write(documentText) : 0;
    const seed = crypto.getRandomValues(new Uint32Array(2));
    const seedValue = seed[0] * 0x100000000 + seed[1];
    this.ex.web_boot(length, PomodoroWasm.utcOffsetSeconds, PomodoroWasm.now(), seedValue);
  }

  command(payload) {
    const length = this.write(JSON.stringify(payload));
    return this.ex.web_command(length, PomodoroWasm.now());
  }

  tick(elapsedMs) {
    return this.ex.web_tick(PomodoroWasm.now(), Math.max(0, Math.round(elapsedMs)));
  }

  render() {
    this.ex.web_render(PomodoroWasm.now());
    return JSON.parse(this.read());
  }

  revision() {
    return this.ex.web_revision();
  }

  export() {
    this.ex.web_export();
    return this.read();
  }
}
