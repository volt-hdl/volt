import { readFileSync } from 'node:fs';
import { join } from 'node:path';
const pkg = process.argv[2];
const mod = await import(join(pkg, 'volt_play.js'));
mod.initSync({ module: readFileSync(join(pkg, 'volt_play_bg.wasm')) });
const gens = {
  binop: (n) => `module Deep {\n  in a : u8\n  out b : u8\n  b = ${'(a + '.repeat(n)}a${')'.repeat(n)}\n}\n`,
  ifs: (n) => `module Deep {\n  in clk : clock\n  in c : bool\n  out q : u8\n  reg r : u8 = 0\n  on clk {\n${'if c {\n'.repeat(n)}r <= r + 1\n${'}\n'.repeat(n)}  }\n  q = r\n}\n`,
  ternary: (n) => `module Deep {\n  in a : u8\n  in c : bool\n  out b : u8\n  b = ${'if c { a } else { '.repeat(n)}a${' }'.repeat(n)}\n}\n`,
};
for (const [k, g] of Object.entries(gens)) {
  for (const n of [64, 128, 200, 250, 255]) {
    try {
      const out = JSON.parse(mod.compile_json(g(n), 'en'));
      console.log(k, n, 'ok', out.diagnostics.map((d) => d.code).join(',') || '-', out.sv ? 'sv' : 'no-sv');
    } catch (e) { console.log(k, n, 'TRAP:', String(e).slice(0, 80)); process.exit(0); }
  }
}
