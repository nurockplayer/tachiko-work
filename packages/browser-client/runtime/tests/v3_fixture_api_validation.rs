//! Compile-only fixture/API validation; it does not assert candidate behavior.
#[path = "v3_acceptance_fixtures/mod.rs"]
mod fixtures;

use tachiko_designer_runtime::{DesignerError, DesignerRuntime, OpenedProjection};
use tachiko_storage::{decode_roproj_v3, encode_roproj_v3};

#[test]
fn valid_constraint_fixtures_roundtrip_in_the_existing_v3_storage_codec() {
    for index in [1, 2] {
        let expected = fixtures::constrained_v3_document(index);
        let tree = encode_roproj_v3(&expected).expect("v3 codec accepts valid field constraints");
        assert_eq!(decode_roproj_v3(&tree).unwrap(), expected);
        assert!(fixtures::frame_v3(&tree).starts_with(b"TWDPROJ1"));
        assert!(fixtures::constrained_v3_transfer(index).starts_with(b"TWDPROJ1"));
        let _typed_open_api: fn(&[u8]) -> Result<OpenedProjection, DesignerError> =
            fixtures::inspect_existing_api;
        let _typed_replace_api: fn(
            &mut Option<DesignerRuntime>,
            &[u8],
            &str,
        ) -> Result<OpenedProjection, DesignerError> = fixtures::open_existing_api;
    }
}
