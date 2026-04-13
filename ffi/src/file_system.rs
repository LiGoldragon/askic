//! FileSystem FFI — file I/O operations.

use std::fs;

pub fn read_to_string(path: String) -> String {
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read_to_string({}): {}", path, e))
}

pub fn write_string(path: String, content: String) {
    fs::write(&path, &content)
        .unwrap_or_else(|e| panic!("write_string({}): {}", path, e));
}

pub fn write_bytes(path: String, bytes: Vec<u8>) {
    fs::write(&path, &bytes)
        .unwrap_or_else(|e| panic!("write_bytes({}): {}", path, e));
}

pub fn read_dir(path: String) -> Vec<String> {
    fs::read_dir(&path)
        .unwrap_or_else(|e| panic!("read_dir({}): {}", path, e))
        .filter_map(|e| e.ok())
        .map(|e| e.path().to_string_lossy().to_string())
        .collect()
}

pub fn file_exists(path: String) -> bool {
    std::path::Path::new(&path).exists()
}
