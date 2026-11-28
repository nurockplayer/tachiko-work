use super::{
    CentralRecord, EndRecord, Entry, PortablePackageError, ZIP_CENTRAL_SIGNATURE, ZIP_DOS_DATE,
    ZIP_DOS_TIME, ZIP_STORE_METHOD, ZIP_UTF8_FLAG, ZIP_VERSION_MADE_BY, ZIP_VERSION_NEEDED, crc32,
    invalid_container, invalid_container_error, parse_central_record, push_u16, push_u32,
};

pub(super) fn write_member(archive: &mut Vec<u8>, entry: &Entry<'_>, local_offset: u32) {
    let size = u32::try_from(entry.body.len()).expect("preflight proves ZIP32 body length");
    let name_length = u16::try_from(entry.name.len()).expect("fixed package name lengths fit u16");
    push_u32(archive, ZIP_CENTRAL_SIGNATURE);
    push_u16(archive, ZIP_VERSION_MADE_BY);
    push_u16(archive, ZIP_VERSION_NEEDED);
    push_u16(archive, ZIP_UTF8_FLAG);
    push_u16(archive, ZIP_STORE_METHOD);
    push_u16(archive, ZIP_DOS_TIME);
    push_u16(archive, ZIP_DOS_DATE);
    push_u32(archive, crc32(entry.body));
    push_u32(archive, size);
    push_u32(archive, size);
    push_u16(archive, name_length);
    push_u16(archive, 0);
    push_u16(archive, 0);
    push_u16(archive, 0);
    push_u16(archive, 0);
    push_u32(archive, 0);
    push_u32(archive, local_offset);
    archive.extend_from_slice(entry.name.as_bytes());
}

pub(super) fn read_central_directory<'a>(
    source: &'a [u8],
    end: &EndRecord,
    central_offset: usize,
    end_offset: usize,
) -> Result<Vec<CentralRecord<'a>>, PortablePackageError> {
    let central_size = usize::try_from(end.central_size)
        .map_err(|_| invalid_container_error("central size does not fit this host"))?;
    if central_offset.checked_add(central_size) != Some(end_offset) {
        return invalid_container("central directory does not exactly precede the end record");
    }

    let mut central_records = Vec::with_capacity(usize::from(end.total_entries));
    let mut record_offset = central_offset;
    for _ in 0..end.total_entries {
        let record = parse_central_record(source, record_offset)?;
        if record.end > end_offset {
            return invalid_container("central entry overlaps the end record");
        }
        record_offset = record.end;
        central_records.push(record);
    }
    if record_offset != end_offset {
        return invalid_container("central records do not consume the declared directory");
    }
    if central_records
        .iter()
        .any(|record| record.disk_number_start != 0)
    {
        return invalid_container("a central entry selects another start disk");
    }
    Ok(central_records)
}
