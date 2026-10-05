const fs=require('fs'),z=require('zlib');
for (const f of process.argv.slice(2)) {
  const b=fs.readFileSync(f);
  const g=z.gzipSync(b,{level:9}).length;
  const br=z.brotliCompressSync(b,{params:{[z.constants.BROTLI_PARAM_QUALITY]:11}}).length;
  console.log(f.split('/').slice(-2).join('/').padEnd(28), 'ham', (b.length/1024).toFixed(0).padStart(5),'KiB  gzip-9',(g/1024).toFixed(0).padStart(5),'KiB  brotli-11',(br/1024).toFixed(0).padStart(5),'KiB');
}
