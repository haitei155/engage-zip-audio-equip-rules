use std::{collections::{hash_map::DefaultHasher, HashMap}, ffi::CString, hash::Hasher, io, path::{Path, PathBuf}};

pub const AUDIO_ROOT: &str = "Data/StreamingAssets/Audio/GeneratedSoundBanks/Switch";

pub fn key(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

pub fn is_audio(path: &str) -> bool {
    let path = key(path);
    path.starts_with(&format!("{}/", AUDIO_ROOT.to_lowercase()))
        && (path.ends_with(".bnk") || path.ends_with(".wem"))
        && !path.split('/').any(|part| part == ".." || part == "." || part.is_empty())
}

// The full virtual path (including language) and content both affect identity.
// Recheck existing bytes on every launch; interrupted writes are never trusted.
pub fn materialize(root: &Path, logical: &str, bytes: &[u8]) -> io::Result<PathBuf> {
    if !is_audio(logical) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a Wwise audio member"));
    }
    let mut digest = DefaultHasher::new();
    digest.write(key(logical).as_bytes());
    digest.write_u64(bytes.len() as u64);
    digest.write(bytes);
    let extension = logical.rsplit('.').next().unwrap().to_ascii_lowercase();
    let path = root.join(format!("{:016x}-{}.{}", digest.finish(), bytes.len(), extension));
    std::fs::create_dir_all(root)?;
    if std::fs::read(&path).is_ok_and(|found| found == bytes) { return Ok(path); }
    let temporary = path.with_extension("tmp");
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    if std::fs::read(&temporary)? != bytes {
        return Err(io::Error::other("audio cache verification failed"));
    }
    if path.exists() { std::fs::remove_file(&path)?; }
    std::fs::rename(&temporary, &path)?;
    Ok(path)
}

#[derive(Default)]
pub struct Routes(HashMap<String, CString>);

impl Routes {
    pub fn insert(&mut self, logical: &str, physical: &str, cached: &str) -> Result<(), std::ffi::NulError> {
        let destination = CString::new(cached)?;
        // Cobalt core calls OpenFile with ZIP/member, not rom:/member.
        self.0.insert(key(physical), destination.clone());
        self.0.insert(key(&format!("rom:/{logical}")), destination);
        Ok(())
    }

    pub fn get(&self, path: &str, mode: i32) -> Option<&CString> {
        // Never redirect create/write operations or guess by filename alone.
        if mode != 1 { return None; }
        self.0.get(&key(path))
    }
}
