use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{Error, Result};

/// Dossier de cache : enchères, royaumes et fiches d'objets.
#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// Lit un fichier `.json.gz`. Un fichier absent ou corrompu vaut `None` :
    /// le cache se reconstruit tout seul.
    pub fn read<T: DeserializeOwned>(&self, name: &str) -> Option<T> {
        let file = std::fs::File::open(self.path(name)).ok()?;
        let mut json = Vec::new();
        GzDecoder::new(file).read_to_end(&mut json).ok()?;
        serde_json::from_slice(&json).ok()
    }

    /// Écrit via un fichier temporaire puis un renommage, pour ne jamais laisser
    /// un fichier à moitié écrit si l'app est fermée pendant un scan.
    pub fn write<T: Serialize>(&self, name: &str, value: &T) -> Result<()> {
        let path = self.path(name);
        let io = |source| Error::Cache { path: path.clone(), source };
        std::fs::create_dir_all(&self.root).map_err(io)?;

        let json = serde_json::to_vec(value).expect("les types du cache sont sérialisables");
        // Un nom par écriture : deux écritures simultanées ne se marchent pas dessus.
        static NEXT_TMP: AtomicU64 = AtomicU64::new(0);
        let tmp = path.with_extension(format!("{}.{}.tmp", std::process::id(), NEXT_TMP.fetch_add(1, Ordering::Relaxed)));
        let mut encoder = GzEncoder::new(std::fs::File::create(&tmp).map_err(io)?, Compression::fast());
        encoder.write_all(&json).map_err(io)?;
        encoder.finish().map_err(io)?;
        std::fs::rename(&tmp, &path).map_err(|source| {
            let _ = std::fs::remove_file(&tmp);
            io(source)
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Secondes depuis l'epoch Unix.
pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_missing() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().join("nested"));
        assert_eq!(cache.read::<Vec<u32>>("x.json.gz"), None);
        cache.write("x.json.gz", &vec![1u32, 2, 3]).unwrap();
        assert_eq!(cache.read::<Vec<u32>>("x.json.gz"), Some(vec![1, 2, 3]));
    }
}
