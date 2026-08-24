#!/usr/bin/env node

import net from "node:net";

const server = net.createServer();
const timer = setTimeout(() => {
  console.error("network denial probe timed out");
  process.exit(1);
}, 5_000);

server.once("listening", () => {
  clearTimeout(timer);
  server.close();
  console.error("network denial probe unexpectedly opened a socket");
  process.exit(1);
});
server.once("error", (error) => {
  clearTimeout(timer);
  if (["EPERM", "EACCES"].includes(error.code)) {
    console.log(`network-denied:${error.code}`);
    process.exit(0);
  }
  console.error(`network denial probe failed with ${error.code ?? error.message}`);
  process.exit(1);
});
server.listen({host: "127.0.0.1", port: 0});
