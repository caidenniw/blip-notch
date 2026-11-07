//! Image sources: `blip-img:N` (cover art pushed by the media service) and `data:` URLs.
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
pub struct ImageStore {
    blobs: HashMap<u32, Vec<u8>>,
    order: VecDeque<u32>,
    next: u32,
}

impl ImageStore {
    /// Stores encoded bytes and returns their `src`; keeps the last 4.
    pub fn put(&mut self, bytes: Vec<u8>) -> String {
        self.next += 1;
        let id = self.next;
        self.blobs.insert(id, bytes);
        self.order.push_back(id);
        while self.order.len() > 4 {
            if let Some(old) = self.order.pop_front() {
                self.blobs.remove(&old);
            }
        }
        format!("blip-img:{id}")
    }

    pub fn bytes(&self, src: &str) -> Option<Vec<u8>> {
        if let Some(n) = src.strip_prefix("blip-img:") {
            return self.blobs.get(&n.parse().ok()?).cloned();
        }
        let rest = src.strip_prefix("data:")?;
        let (meta, data) = rest.split_once(',')?;
        if meta.ends_with(";base64") {
            base64(data)
        } else {
            Some(data.as_bytes().to_vec())
        }
    }
}

pub fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b' ' | b'\n' | b'\r' => continue,
            _ => return None,
        } as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn decodes() {
        assert_eq!(super::base64("aGVsbG8=").unwrap(), b"hello");
        let mut s = super::ImageStore::default();
        let src = s.put(vec![1, 2]);
        assert_eq!(s.bytes(&src).unwrap(), vec![1, 2]);
        for _ in 0..4 {
            s.put(vec![0]);
        }
        assert!(s.bytes(&src).is_none());
        assert_eq!(s.bytes("data:text/plain;base64,aGk=").unwrap(), b"hi");
    }
}
