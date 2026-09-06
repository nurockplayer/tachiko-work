// Deliberate acceptance seed. Replace only after the Steward records Ready.
console.error('PROBE_NOT_IMPLEMENTED: Tachiko to open-sheet export is not implemented.');
console.log(JSON.stringify({ status: 'rejected', code: 'PROBE_NOT_IMPLEMENTED', ledger: [] }));
process.exitCode = 2;
