// Test-only local asset server; no installation, mutation or remote execution.
import http from 'node:http';
import path from 'node:path';
import {readFile,stat} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
const kit=process.env.WORK_CLIENT_KIT;
if(!kit){console.error('BLOCKED: WORK_CLIENT_KIT must name a complete, pinned exported kit.');process.exit(78);}
const kitRoot=path.resolve(kit);
try{await stat(path.join(kitRoot,'experimental-client.js'));await stat(path.join(kitRoot,'designer_runtime.wasm'));}
catch{console.error('BLOCKED: complete JS/Worker/WASM kit is missing.');process.exit(78);}
const html=fileURLToPath(new URL('../tests/runtime-canary.html',import.meta.url));
const mime={'.js':'text/javascript','.mjs':'text/javascript','.wasm':'application/wasm','.json':'application/json','.html':'text/html'};
const port=Number(process.env.PORT??4174);
if(!Number.isInteger(port)||port<1024||port>65535)throw new Error('PORT must be 1024..65535');
const server=http.createServer(async(req,res)=>{
 try{
  if(req.method!=='GET'){res.writeHead(405);res.end();return;}
  const name=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
  let file;
  if(name==='/')file=html;
  else if(name.startsWith('/kit/')){
   file=path.resolve(kitRoot,name.slice(5));
   if(!file.startsWith(kitRoot+path.sep))throw new Error('Outside kit');
  }else{res.writeHead(404);res.end();return;}
  const bytes=await readFile(file);
  res.writeHead(200,{'Content-Type':mime[path.extname(file)]??'application/octet-stream','Cache-Control':'no-store'});res.end(bytes);
 }catch{res.writeHead(400);res.end('Invalid or unavailable test asset');}
});
server.listen(port,'127.0.0.1',()=>console.log(`Runtime canary: http://127.0.0.1:${port}`));
