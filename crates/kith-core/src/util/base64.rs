//! Encode-only base64 (the standard RFC 4648 alphabet, padded).
//!
//! Kith embeds image bytes as `data:` URLs in the self-contained HTML export
//! (the export caller resolves a portrait to a data URL so `render::html` stays
//! pure). That is the *only* base64 need, so this is **encode-only** — there is
//! no decoder to carry — and hand-rolled, keeping the dependency manifest at
//! zero new runtime crates (the project's grain: a hand-rolled tidy tree, date
//! parser, and GEDCOM engine).

/// The standard base64 alphabet (RFC 4648 §4).
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes `bytes` as standard, padded base64.
///
/// The output is pre-sized to the exact encoded length (`4 * ceil(n / 3)`), so
/// the `String` allocates exactly once.
pub(crate) fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(4 * bytes.len().div_ceil(3));
    for chunk in bytes.chunks(3) {
        // `chunks(3)` never yields an empty chunk, so index 0 is always present;
        // the remaining two bytes are zero-padded when the chunk is short.
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let triple = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        match chunk.len() {
            1 => out.push_str("=="),
            2 => {
                out.push(ALPHABET[((triple >> 6) & 0x3f) as usize] as char);
                out.push('=');
            }
            _ => {
                out.push(ALPHABET[((triple >> 6) & 0x3f) as usize] as char);
                out.push(ALPHABET[(triple & 0x3f) as usize] as char);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::encode;

    /// The canonical RFC 4648 §10 test vectors — they exercise all three padding
    /// cases (0, 1, and 2 trailing bytes).
    #[test]
    fn rfc4648_vectors() {
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(encode(input.as_bytes()), expected, "input {input:?}");
        }
    }

    /// A byte with the high bit set must still map into the alphabet (not panic
    /// or produce non-ASCII): `0xFF, 0xFF, 0xFF` → all-ones sextets.
    #[test]
    fn high_bytes_encode_within_the_alphabet() {
        assert_eq!(encode(&[0xff, 0xff, 0xff]), "////");
        assert_eq!(encode(&[0x00, 0x00, 0x00]), "AAAA");
    }
}
