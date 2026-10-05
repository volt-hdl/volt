// Statik sunucu, gzip'li (GitHub Pages benzeri): node web/serve.mjs <dizin> <port>
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { gzipSync } from 'node:zlib';
import { join, extname, normalize } from 'node:path';
const [root, port] = process.argv.slice(2);
const types = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.wasm': 'application/wasm', '.volt': 'text/plain; charset=utf-8' };
createServer(async (req, res) => {
  const rel = normalize(decodeURIComponent(req.url.split('?')[0])).replace(/^(\.\.[/\\])+/, '');
  const path = join(root, rel.endsWith('/') ? rel + 'index.html' : rel);
  try {
    let body = await readFile(path);
    const headers = { 'content-type': types[extname(path)] ?? 'application/octet-stream', 'cache-control': 'no-store' };
    if ((req.headers['accept-encoding'] ?? '').includes('gzip')) { body = gzipSync(body, { level: 6 }); headers['content-encoding'] = 'gzip'; }
    res.writeHead(200, headers); res.end(body);
  } catch { res.writeHead(404); res.end('not found'); }
}).listen(Number(port));
