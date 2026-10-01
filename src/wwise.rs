use std::{ffi::CStr, path::Path, sync::{OnceLock, atomic::{AtomicUsize, Ordering}}};
use crate::audio_cache::{self, Routes, AUDIO_ROOT};

static ROUTES: OnceLock<Routes> = OnceLock::new();

pub fn prepare() -> usize {
    let mut routes = Routes::default();
    let (mut ready, mut loose, mut errors) = (0, 0, 0);
    let mut report = String::from("Cobalt ZIP Audio startup preparation (not a playback test)\n");
    // Finish ZIP reads and cache writes BEFORE installing the filesystem hook.
    // The callback only consults an immutable map, so no filesystem recursion,
    // archive mutex, decompression, or cache write happens on Wwise's thread.
    for logical in mods::list_recursive(AUDIO_ROOT) {
        if !audio_cache::is_audio(logical.as_str()) { continue; }
        let Some(physical) = mods::disk_path(logical) else { continue; };
        if physical.is_file() { loose += 1; continue; }
        let result = mods::read(logical).map_err(|e| e.to_string()).and_then(|bytes| {
            audio_cache::materialize(Path::new("sd:/engage/.cache/wwise-zip-audio"), logical.as_str(), &bytes)
                .map_err(|e| e.to_string())
        });
        match result {
            Ok(cached) => {
                if let Some(cached) = cached.to_str() {
                    if routes.insert(logical.as_str(), physical.as_str(), cached).is_ok() {
                        ready += 1;
                        report.push_str(&format!("READY {logical}\n"));
                        continue;
                    }
                }
                errors += 1;
                report.push_str(&format!("FAILED invalid cache path: {logical}\n"));
                crate::checkpoint(&format!("[ZIP Audio] invalid cache path: {logical}\n"));
            }
            Err(error) => {
                errors += 1;
                report.push_str(&format!("FAILED {logical}: {error}\n"));
                crate::checkpoint(&format!("[ZIP Audio] cache FAILED: {logical}: {error}\n"));
            }
        }
    }
    let _ = ROUTES.set(routes);
    let summary = format!("[ZIP Audio] ready={ready}, loose={loose}, errors={errors}\n");
    report.push_str(&summary);
    crate::checkpoint(&summary);
    // A Switch user can inspect this file on the SD card without emulator logs.
    let _ = std::fs::create_dir_all("sd:/engage/.cache/wwise-zip-audio");
    if let Err(error) = std::fs::write("sd:/engage/.cache/wwise-zip-audio/startup-status.txt", report) {
        crate::checkpoint(&format!("[ZIP Audio] status write failed: {error}\n"));
    }
    ready
}

// Intercept the native file service used by Cobalt's own Wwise hook. This also
// handles its sd:/engage/mods/name.zip/Data/... pseudo paths. No game offset.
#[skyline::hook(replace = skyline::nn::fs::OpenFile)]
pub fn fee_zip_audio_open_file(handle: *mut skyline::nn::fs::FileHandle, path: *const u8, mode: i32) -> u32 {
    if mode == 1 && !path.is_null() {
        if let Ok(request) = unsafe { CStr::from_ptr(path.cast()) }.to_str() {
            if let Some(destination) = ROUTES.get().and_then(|routes| routes.get(request, mode)) {
                let result = call_original!(handle, destination.as_ptr().cast(), mode);
                static REDIRECTS: AtomicUsize = AtomicUsize::new(0);
                if REDIRECTS.fetch_add(1, Ordering::Relaxed) < 12 || result != 0 {
                    crate::checkpoint(&format!("[ZIP Audio] OpenFile result={result:#x}: {request}\n"));
                }
                if result == 0 { return result; }
            }
        }
    }
    call_original!(handle, path, mode)
}
