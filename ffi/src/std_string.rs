//! StdString FFI — string operations.

pub fn lines(s: &str) -> Vec<String> {
    s.lines().map(|l| l.to_string()).collect()
}

pub fn trim(s: &str) -> String {
    s.trim().to_string()
}

pub fn char_at(s: &str, idx: u32) -> u32 {
    s.chars().nth(idx as usize).unwrap_or('\0') as u32
}

pub fn substring(s: &str, start: u32, end: u32) -> String {
    s.chars().skip(start as usize).take((end - start) as usize).collect()
}

pub fn starts_with(s: &str, prefix: &str) -> bool {
    s.starts_with(prefix)
}

pub fn len(s: &str) -> u32 {
    s.len() as u32
}

pub fn is_empty(s: &str) -> bool {
    s.is_empty()
}

pub fn find(s: &str, needle: &str) -> Option<u32> {
    s.find(needle).map(|i| i as u32)
}

pub fn contains(s: &str, needle: &str) -> bool {
    s.contains(needle)
}

pub fn replace(s: &str, from: &str, to: &str) -> String {
    s.replace(from, to)
}

pub fn to_lowercase(s: &str) -> String {
    s.to_lowercase()
}

pub fn split_newlines(s: &str) -> Vec<String> {
    s.split('\n').map(|l| l.to_string()).collect()
}
