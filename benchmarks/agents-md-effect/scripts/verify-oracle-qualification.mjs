#!/usr/bin/env node

import {readFile} from "node:fs/promises";
import {isAbsolute, resolve} from "node:path";
import {verifyQualificationReceipt} from "./oracle-qualification-normalization.mjs";

function usage() {
  console.error("usage: node verify-oracle-qualification.mjs --receipt /abs/oracles.json");
  process.exit(2);
}

const argv = process.argv.slice(2);
if (argv.length !== 2 || argv[0] !== "--receipt" || !isAbsolute(argv[1])) usage();
const receipt = JSON.parse(await readFile(resolve(argv[1]), "utf8"));
const verified = verifyQualificationReceipt(receipt);
console.log(JSON.stringify({verified: true, ...verified}));
