const b=require('fs').readFileSync(process.argv[2]);let p=8;const names={0:'custom',1:'type',2:'import',3:'function',5:'memory',6:'global',7:'export',10:'code',11:'data'};
function leb(){let r=0,s=0,x;do{x=b[p++];r|=(x&0x7f)<<s;s+=7}while(x&0x80);return r>>>0}
while(p<b.length){const id=b[p++];const n=leb();console.log(String(names[id]||id).padEnd(9),(n/1024).toFixed(0),'KiB');p+=n}
