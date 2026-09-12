pub(crate) mod host;
pub(crate) mod v1;
pub(crate) mod v2;

pub use host::{
    canonicalize_roproj, load_roproj, materialize_roproj, publish_canonicalized_roproj,
    publish_roproj, read_canonical_roproj,
};
#[cfg(feature = "issue-175-research")]
pub use v1::issue_175_admit_a0_a1;
pub use v1::{
    CanonicalRoProjectAdmissionError, CanonicalRoProjectFile, CanonicalRoProjectV1,
    ROPROJ_V1_FORMAT_VERSION, ROPROJ_V1_PATHS, decode as decode_roproj_v1,
    encode as encode_roproj_v1,
};
pub use v2::{
    CanonicalRoProjectFileV2, CanonicalRoProjectV2, ROPROJ_V2_FORMAT_VERSION, ROPROJ_V2_PATHS,
    decode as decode_roproj_v2, encode as encode_roproj_v2, migrate_v1 as migrate_roproj_v1_to_v2,
};
