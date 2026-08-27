pub(crate) mod v1;

pub use v1::{
    CanonicalRoProjectFile, CanonicalRoProjectV1, ROPROJ_V1_FORMAT_VERSION, ROPROJ_V1_PATHS,
    encode as encode_roproj_v1,
};
