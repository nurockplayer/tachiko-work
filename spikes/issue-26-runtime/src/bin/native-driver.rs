//! Native JSONL driver for exact differential comparison with the WASM adapter.

use std::io::{self, BufRead, Write};

use tachiko_issue_26_runtime_spike::{ResidentRuntime, process_wire_request};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    let mut runtime: Option<ResidentRuntime> = None;

    for line in stdin.lock().lines() {
        let line = line?;
        let response = process_wire_request(&mut runtime, line.as_bytes());
        stdout.write_all(&response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}
