//! One host-owned XLSX source, moved out of the encoded inspect reply.
use crate::interop_adapter::{ImportOptions, SourceWorkbook};
use crate::{DesignerResponse, DesignerWireReply};

pub(crate) struct SourceIdentity {
    bytes: Vec<u8>,
    options: ImportOptions,
    working: bool,
}

impl SourceIdentity {
    pub(crate) fn new(bytes: &[u8], options: ImportOptions, working: bool) -> Self {
        Self {
            bytes: bytes.to_vec(),
            options,
            working,
        }
    }
}

pub(crate) struct InspectedSource {
    identity: SourceIdentity,
    workbook: Box<SourceWorkbook>,
}

impl InspectedSource {
    pub(crate) fn from_preview(identity: SourceIdentity, reply: DesignerWireReply) -> Self {
        let DesignerWireReply::Ok {
            response: DesignerResponse::ImportPreview(workbook),
        } = reply
        else {
            unreachable!("Only a successful XLSX inspect retains a source")
        };
        Self { identity, workbook }
    }

    pub(crate) fn consume(
        self,
        bytes: &[u8],
        options: &ImportOptions,
        working: bool,
        xlsx: bool,
    ) -> Option<Box<SourceWorkbook>> {
        (xlsx
            && self.identity.bytes == bytes
            && self.identity.options == *options
            && self.identity.working == working)
            .then_some(self.workbook)
    }
}

#[cfg(test)]
mod tests;
