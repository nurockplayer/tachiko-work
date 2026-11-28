use super::{
    LocalRecord, PortablePackageError, ZIP_DOS_DATE, ZIP_DOS_TIME, ZIP_STORE_METHOD, ZIP_UTF8_FLAG,
    ZIP_VERSION_NEEDED, noncanonical_container,
};

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
