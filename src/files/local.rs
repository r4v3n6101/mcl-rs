use std::path::Path;

use super::Type;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Index {
    pub path: Box<Path>,
    pub r#type: Type,
    pub size: u64,
}

// get the list
// then download or load version info
// then check and download files
// then find asset index
// then check assets and download them
// then check natives and unpack them

pub async fn cycle() {}
