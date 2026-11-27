use super::{
    Entry, ZIP_CENTRAL_SIGNATURE, ZIP_DOS_DATE, ZIP_DOS_TIME, ZIP_STORE_METHOD, ZIP_UTF8_FLAG,
    ZIP_VERSION_MADE_BY, ZIP_VERSION_NEEDED, crc32, push_u16, push_u32,
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
