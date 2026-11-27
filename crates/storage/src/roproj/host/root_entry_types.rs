use std::path::Path;

use super::{ROPROJ_V3_PRESENTATION_PATH, require_directory, require_regular_file};
use crate::FormatError;

pub(super) fn require_root_entry_types(
    root: &Path,
    definitions: bool,
    presentation: bool,
) -> Result<(), FormatError> {
    require_regular_file(&root.join("manifest.json"), "manifest.json")?;
    require_regular_file(&root.join("schemas.json"), "schemas.json")?;
    if definitions {
        require_regular_file(&root.join("definitions.json"), "definitions.json")?;
    }
    if presentation {
        require_regular_file(
            &root.join(ROPROJ_V3_PRESENTATION_PATH),
            ROPROJ_V3_PRESENTATION_PATH,
        )?;
    }
    require_directory(&root.join("entities"), "entities directory")
}
