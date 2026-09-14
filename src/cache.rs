use crate::asn::Asn;
use crate::classifier::Classification;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};

const FORMAT_VERSION: u8 = 6;

#[derive(Clone, Copy, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub(crate) struct CacheKey {
    ignore_private_asn: bool,
    origin_only: bool,
    domestic_policy_fingerprint: Option<u64>,
    operator_asns_fingerprint: u64,
}

impl CacheKey {
    pub(crate) fn new(
        ignore_private_asn: bool,
        origin_only: bool,
        domestic_policy_fingerprint: Option<u64>,
        operator_asns: &HashSet<Asn>,
    ) -> Self {
        let mut operators: Vec<Asn> = operator_asns.iter().copied().collect();
        operators.sort_unstable();
        let mut hasher = DefaultHasher::new();
        operators.hash(&mut hasher);
        Self {
            ignore_private_asn,
            origin_only,
            domestic_policy_fingerprint,
            operator_asns_fingerprint: hasher.finish(),
        }
    }

    pub(crate) fn path(self, mrt_files: &[PathBuf]) -> PathBuf {
        let mut sources: Vec<String> = mrt_files
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        sources.sort();

        let mut hasher = DefaultHasher::new();
        FORMAT_VERSION.hash(&mut hasher);
        self.hash(&mut hasher);
        sources.hash(&mut hasher);
        PathBuf::from(format!("cache-{:016x}.bin", hasher.finish()))
    }
}

#[derive(Deserialize, Serialize)]
struct CachedClassification {
    version: u8,
    key: CacheKey,
    classification: Classification,
}

pub(crate) fn load(path: &Path, key: CacheKey) -> Option<Classification> {
    let cache: CachedClassification =
        bincode::deserialize_from(BufReader::new(File::open(path).ok()?)).ok()?;
    (cache.version == FORMAT_VERSION && cache.key == key).then_some(cache.classification)
}

pub(crate) fn save(path: &Path, key: CacheKey, classification: Classification) -> Classification {
    let cache = CachedClassification {
        version: FORMAT_VERSION,
        key,
        classification,
    };
    if let Ok(file) = File::create(path) {
        let _ = bincode::serialize_into(BufWriter::new(file), &cache);
    }
    cache.classification
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_path_changes_when_origin_only_changes() {
        let mrt_files = vec![PathBuf::from("rib-a.gz"), PathBuf::from("rib-b.gz")];
        assert_ne!(
            CacheKey::new(false, false, None, &HashSet::new()).path(&mrt_files),
            CacheKey::new(false, true, None, &HashSet::new()).path(&mrt_files)
        );
    }

    #[test]
    fn cache_path_changes_with_domestic_policy() {
        let mrt_files = vec![PathBuf::from("rib-a.gz")];
        assert_ne!(
            CacheKey::new(false, true, None, &HashSet::new()).path(&mrt_files),
            CacheKey::new(false, true, Some(1), &HashSet::new()).path(&mrt_files)
        );
        assert_ne!(
            CacheKey::new(false, true, Some(1), &HashSet::new()).path(&mrt_files),
            CacheKey::new(false, true, Some(2), &HashSet::new()).path(&mrt_files)
        );
    }

    #[test]
    fn cache_path_tracks_operator_asns_independently_of_order() {
        let mrt_files = vec![PathBuf::from("rib-a.gz")];
        let operators = HashSet::from([64496.into(), 64497.into()]);
        let reversed = HashSet::from([64497.into(), 64496.into()]);
        assert_eq!(
            CacheKey::new(false, false, None, &operators).path(&mrt_files),
            CacheKey::new(false, false, None, &reversed).path(&mrt_files)
        );
        assert_ne!(
            CacheKey::new(false, false, None, &operators).path(&mrt_files),
            CacheKey::new(false, false, None, &HashSet::from([64496.into()])).path(&mrt_files)
        );
    }
}
