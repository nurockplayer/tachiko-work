use std::collections::HashMap;

use super::{
    CentralOffsetIndex, LocalRecord, PortablePackageError, ZIP_DOS_DATE, ZIP_DOS_TIME,
    ZIP_STORE_METHOD, ZIP_UTF8_FLAG, ZIP_VERSION_NEEDED, noncanonical_container,
};

#[derive(Clone, Copy)]
pub(super) struct LocalDirectoryContext<'a> {
    pub(super) local_section_end: usize,
    pub(super) expected_next_offset: Option<usize>,
    pub(super) central_by_local_offset: &'a HashMap<usize, CentralOffsetIndex>,
}

impl LocalDirectoryContext<'_> {
    fn has_declared_boundary(self, end: Option<usize>) -> bool {
        end.is_some_and(|end| {
            end == self.local_section_end
                || self
                    .central_by_local_offset
                    .get(&end)
                    .is_some_and(|matching| matching.record_index.is_some())
        })
    }
}

pub(super) fn select_ordinary_data_size(
    data_start: usize,
    local_size: u32,
    central_size: Option<u32>,
    directory: LocalDirectoryContext<'_>,
) -> u32 {
    let Some(central_size) = central_size else {
        return local_size;
    };
    if local_size == central_size {
        return local_size;
    }
    let local_end = ordinary_data_end(data_start, local_size, directory.local_section_end);
    let central_end = ordinary_data_end(data_start, central_size, directory.local_section_end);
    // Stored data can contain local-header signature bytes. Resolve a size
    // disagreement from the parsed central offset chain, never from a payload
    // byte pattern that merely resembles the next record.
    if let Some(expected_next_offset) = directory.expected_next_offset {
        if local_end == Some(expected_next_offset) && central_end != Some(expected_next_offset) {
            return local_size;
        }
        if central_end == Some(expected_next_offset) && local_end != Some(expected_next_offset) {
            return central_size;
        }
    }
    let local_declared = directory.has_declared_boundary(local_end);
    let central_declared = directory.has_declared_boundary(central_end);
    if central_declared && !local_declared {
        central_size
    } else {
        local_size
    }
}

fn ordinary_data_end(data_start: usize, size: u32, local_section_end: usize) -> Option<usize> {
    let size = usize::try_from(size).ok()?;
    let end = data_start.checked_add(size)?;
    (end <= local_section_end).then_some(end)
}

pub(super) fn validate_canonical_metadata(
    local: &LocalRecord<'_>,
    expected_name: &str,
) -> Result<(), PortablePackageError> {
    if local.version_needed != ZIP_VERSION_NEEDED
        || local.flags != ZIP_UTF8_FLAG
        || local.method != ZIP_STORE_METHOD
        || local.dos_time != ZIP_DOS_TIME
        || local.dos_date != ZIP_DOS_DATE
        || local.extra_length != 0
    {
        return noncanonical_container(&format!(
            "'{expected_name}' local header has noncanonical metadata"
        ));
    }
    Ok(())
}
