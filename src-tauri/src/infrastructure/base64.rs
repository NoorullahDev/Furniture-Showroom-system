const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Minimal RFC 4648 base64 encoder (with padding). No external dependency.
pub fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// Content type for a small set of image magic bytes; defaults to WebP.
pub fn image_mime(data: &[u8]) -> &'static str {
    if data.len() >= 8 && &data[..8] == b"\x89PNG\r\n\x1a\n" {
        "image/png"
    } else if data.len() >= 3 && &data[..3] == b"\xff\xd8\xff" {
        "image/jpeg"
    } else if (data.len() >= 6 && &data[..6] == b"GIF87a")
        || (data.len() >= 6 && &data[..6] == b"GIF89a")
    {
        "image/gif"
    } else if data.len() >= 2 && &data[..2] == b"BM" {
        "image/bmp"
    } else {
        "image/webp"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_known_vectors() {
        assert_eq!(encode(b"Man"), "TWFu");
        assert_eq!(encode(b"Ma"), "TWE=");
        assert_eq!(encode(b"M"), "TQ==");
        assert_eq!(encode(b"Hello, world!"), "SGVsbG8sIHdvcmxkIQ==");
    }
}
