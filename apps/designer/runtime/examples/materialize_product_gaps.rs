#[path = "../fixtures/product_gaps.rs"]
mod product_gaps;

use std::{env, io, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let destination = arguments.next().map(PathBuf::from).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: materialize_product_gaps <destination.roproj>",
        )
    })?;
    if arguments.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: materialize_product_gaps <destination.roproj>",
        )
        .into());
    }

    tachiko_storage::materialize_roproj(&destination, &product_gaps::document())?;
    println!("materialized {}", destination.display());
    Ok(())
}
