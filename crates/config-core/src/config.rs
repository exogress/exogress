use exogress_entities::MountPointName;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::hash::Hash;

pub trait Config: Serialize + DeserializeOwned + Debug + Clone + Hash {
    type Error: std::error::Error;

    fn checksum(&self) -> u64;
    fn check_mount_points(&self, existing: &[MountPointName]) -> Result<(), Self::Error>;
    fn validate(&self) -> Result<(), Self::Error>;
}
