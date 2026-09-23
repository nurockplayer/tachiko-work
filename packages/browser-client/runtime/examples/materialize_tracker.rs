//! Materialize the operational tracker through the same creation/paste/export adapter as Drivers.
use std::{
    env,
    io::{self, Cursor, Read},
    path::PathBuf,
};

use tachiko_designer_runtime::{DesignerRequest, DesignerResponse, DesignerRuntime};
use tachiko_storage::{CanonicalRoProjectV1, publish_roproj};

const OCCURRENCE: &str = "4d9475a3-9ba3-4a61-a7f5-852a84e82257";
const ROWS: &str = include_str!("../../e2e/fixtures/operations-tracker.tsv");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os().skip(1);
    let destination = arguments
        .next()
        .map(PathBuf::from)
        .filter(|_| arguments.next().is_none())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: materialize_tracker <destination.roproj>",
            )
        })?;
    let mut runtime = DesignerRuntime::tracker(OCCURRENCE)?;
    let response = runtime.handle(DesignerRequest::PasteCells {
        expected_revision: "resident/0".to_owned(),
        collection: "tracker".to_owned(),
        start_entity: None,
        start_field: "task".to_owned(),
        rows: ROWS
            .lines()
            .map(|row| row.split('\t').map(str::to_owned).collect())
            .collect(),
    })?;
    let DesignerResponse::Published(publication) = response else {
        return Err(io::Error::other("paste did not publish").into());
    };
    let export = runtime.export_project(&publication.resulting_revision)?;
    let tree = exported_tree(&export.bytes)?;
    publish_roproj(&destination, &tree)?;
    println!("materialized {}", destination.display());
    Ok(())
}

fn exported_tree(bytes: &[u8]) -> Result<CanonicalRoProjectV1, Box<dyn std::error::Error>> {
    let mut cursor = Cursor::new(bytes);
    let mut magic = [0; 8];
    cursor.read_exact(&mut magic)?;
    if &magic != b"TWDPROJ1" {
        return Err(io::Error::other("unexpected private project export").into());
    }
    let mut count = [0; 4];
    cursor.read_exact(&mut count)?;
    let mut files = Vec::new();
    for _ in 0..u32::from_le_bytes(count) {
        let mut path_length = [0; 2];
        let mut byte_length = [0; 4];
        cursor.read_exact(&mut path_length)?;
        cursor.read_exact(&mut byte_length)?;
        let mut path = vec![0; usize::from(u16::from_le_bytes(path_length))];
        let mut content = vec![0; usize::try_from(u32::from_le_bytes(byte_length))?];
        cursor.read_exact(&mut path)?;
        cursor.read_exact(&mut content)?;
        files.push((String::from_utf8(path)?, content));
    }
    if cursor.position() != u64::try_from(bytes.len())? {
        return Err(io::Error::other("trailing export bytes").into());
    }
    Ok(CanonicalRoProjectV1::try_from_files(files)?)
}
