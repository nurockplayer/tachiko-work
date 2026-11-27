use super::{InspectedSource, SourceIdentity};
use crate::interop_adapter::{ImportOptions, SourceWorkbook};
use crate::{DesignerResponse, DesignerWireReply};

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
