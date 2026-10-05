// Node'da wasm çekirdeğini koşar: örnekler, süreler, yığın sınırı.
//   node web/node-smoke.mjs <pkg-dizini> <örnek-dizini>
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { performance } from 'node:perf_hooks';

const [pkg, examples] = process.argv.slice(2);
const mod = await import(join(pkg, 'volt_play.js'));
const bytes = readFileSync(join(pkg, 'volt_play_bg.wasm'));

let t = performance.now();
const compiled = await WebAssembly.compile(bytes);
const tCompile = performance.now() - t;
t = performance.now();
mod.initSync({ module: compiled });
const tInit = performance.now() - t;
console.log(`wasm: ${(bytes.length / 1024).toFixed(0)} KiB, WebAssembly.compile ${tCompile.toFixed(0)} ms, init ${tInit.toFixed(1)} ms`);

function run(name, src, lang = 'en') {
  const t0 = performance.now();
  const out = JSON.parse(mod.compile_json(src, lang));
  const ms = performance.now() - t0;
  const codes = out.diagnostics.map((d) => d.code).join(',') || '-';
  const sv = out.sv ? `${out.sv.split('\n').length} satır SV` : 'SV yok';
  console.log(`${name.padEnd(22)} ${ms.toFixed(1).padStart(7)} ms  hata=${out.summary.errors} uyarı=${out.summary.warnings} kodlar=${codes}  ${sv}`);
  return out;
}

for (const f of readdirSync(examples).filter((f) => f.endsWith('.volt')).sort()) {
  run(f, readFileSync(join(examples, f), 'utf8'));
}
// İkinci koşu (ısınmış).
for (const f of readdirSync(examples).filter((f) => f.endsWith('.volt')).sort()) {
  run(`${f} (2.)`, readFileSync(join(examples, f), 'utf8'));
}

const cdc = run('cdc_error tr', readFileSync(join(examples, 'cdc_error.volt'), 'utf8'), 'tr');
console.log('--- rendered[0] (tr) ---\n' + cdc.rendered[0]);
console.log('--- diagnostics[0] (JSON, kısaltılmış) ---');
const d = cdc.diagnostics[0];
console.log(JSON.stringify({ code: d.code, severity: d.severity, message: d.message, start: d.spans[0].start, help: d.help, suggestions: d.suggestions.length, explain_url: d.explain_url }));

const blinky = run('blinky', readFileSync(join(examples, 'blinky.volt'), 'utf8'));
console.log('--- blinky SV (ilk 12 satır) ---\n' + blinky.sv.split('\n').slice(0, 12).join('\n'));

// Sözdizimi hatası ve boş girdi.
run('syntax error', 'module M {\n  in a : u8\n  out b : u8\n  b = a +\n}\n');
run('empty', '');

// Parser derinlik sınırına yakın iç içe ifade (ADR-0080): wasm yığını yeter mi?
for (const depth of [50, 150, 250, 300]) {
  const expr = '('.repeat(depth) + 'a' + ')'.repeat(depth);
  const src = `module Deep {\n  in a : u8\n  out b : u8\n  b = ${expr}\n}\n`;
  try {
    run(`nesting ${depth}`, src);
  } catch (e) {
    console.log(`nesting ${depth}: TRAP ${e}`);
    break;
  }
}

// Büyük girdi: 200 modül.
let big = '';
for (let i = 0; i < 200; i++) {
  big += `pub module C${i} {\n  in clk : clock\n  in en : bool\n  out q : u16\n  reg r : u16 = 0\n  on clk {\n    if en { r <= r + 1 }\n  }\n  q = r\n}\n\n`;
}
run('200 modül', big);
