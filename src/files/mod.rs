pub mod local;
pub mod remote;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum Type {
    AssetIndex,
    Asset,

    Library,
    NativeLibrary,
    ClientJar,

    VersionInfo,

    Other,
}
