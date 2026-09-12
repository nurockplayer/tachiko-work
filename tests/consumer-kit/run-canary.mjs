import {spawn} from "node:child_process";
const [serve,canary]=process.argv.slice(2); if(!serve||!canary) throw new Error("usage: SERVE CANARY");
const port=process.env.TACHIKO_CANARY_PORT??"4186";
const server=spawn(process.execPath,[serve],{env:{...process.env,PORT:port},stdio:["ignore","pipe","pipe"]});
let output="",stdoutBuffer="",exited=false;
const exitedServer=new Promise(resolve=>server.once("exit",(code,signal)=>{exited=true;resolve({code,signal});}));
for(const stream of [server.stdout,server.stderr]) stream.on("data",chunk=>{output+=chunk;process.stderr.write(chunk);});
const ready=new Promise((resolve,reject)=>{
  let done=false;
  const finish=(error)=>{if(done)return;done=true;clearTimeout(timer);server.off("error",onError);server.off("exit",onExit);server.stdout.off("data",onStdout);error?reject(error):resolve();};
  const onError=error=>finish(error);
  const onExit=(code,signal)=>finish(new Error("canary server exited "+(signal??code)+": "+output));
  const onStdout=chunk=>{stdoutBuffer+=chunk.toString();if(stdoutBuffer.includes("Runtime canary: http://127.0.0.1:"+port))finish();};
  const timer=setTimeout(()=>finish(new Error("canary server did not report readiness")),30000);
  server.once("error",onError);server.once("exit",onExit);server.stdout.on("data",onStdout);
});
async function stopServer(){
  if(exited||!server.pid)return;
  server.kill("SIGTERM");
  let timeout;
  const grace=await Promise.race([exitedServer,new Promise(resolve=>{timeout=setTimeout(resolve,5000);})]);
  clearTimeout(timeout);
  if(grace===undefined&&!exited){server.kill("SIGKILL");await exitedServer;}
}
try { await ready; const child=spawn(process.execPath,[canary,"--canary"],{env:{...process.env,WORK_CLIENT_URL:"http://127.0.0.1:"+port},stdio:"inherit"}); const code=await new Promise((resolve,reject)=>{child.on("error",reject);child.on("exit",resolve);}); if(code!==0) process.exitCode=code??1; } finally { await stopServer(); }
