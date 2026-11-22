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
mod tests {
    use super::*;

    fn retained() -> InspectedSource {
        InspectedSource::from_preview(
            SourceIdentity::new(b"xlsx", ImportOptions::default(), false),
            DesignerWireReply::Ok {
                response: DesignerResponse::ImportPreview(Box::new(SourceWorkbook {
                    sheets: vec![],
                    ledger: vec![],
                })),
            },
        )
    }

    #[test]
    fn reuse_moves_the_same_decoded_source() {
        let source = retained();
        let address = std::ptr::from_ref(source.workbook.as_ref());
        let workbook = source
            .consume(b"xlsx", &ImportOptions::default(), false, true)
            .unwrap();
        assert_eq!(address, std::ptr::from_ref(workbook.as_ref()));
    }

    #[test]
    fn every_source_input_must_match() {
        let options = ImportOptions::default();
        assert!(
            retained()
                .consume(b"other", &options, false, true)
                .is_none()
        );
        assert!(retained().consume(b"xlsx", &options, true, true).is_none());
        assert!(
            retained()
                .consume(b"xlsx", &options, false, false)
                .is_none()
        );
        for changed in [
            ImportOptions {
                delimiter: ';',
                ..options.clone()
            },
            ImportOptions {
                header: false,
                ..options.clone()
            },
            ImportOptions {
                header_rows: vec![Some(2)],
                ..options.clone()
            },
            ImportOptions {
                keep_summary_rows: true,
                ..options.clone()
            },
        ] {
            assert!(retained().consume(b"xlsx", &changed, false, true).is_none());
        }
    }
}
