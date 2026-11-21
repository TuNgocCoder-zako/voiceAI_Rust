pub mod normalize;
pub mod patterns;
pub mod fuzzy;
pub mod router;

pub use normalize::{clean_user_input, strip_vietnamese_accents};
pub use router::fast_path_nlu;
