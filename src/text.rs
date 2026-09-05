//! Counting text the way Qt counts it. Rust indexes a string in bytes, harper in
//! characters, and a QString in UTF-16 units; every offset that crosses between them
//! comes through here.

/// Where the UTF-16 position `at` falls in `text`, which Rust counts in bytes.
pub fn byte_offset(text: &str, at: i32) -> usize {
    let Ok(at) = usize::try_from(at) else {
        return text.len();
    };
    let mut units = 0;
    for (offset, character) in text.char_indices() {
        if units >= at {
            return offset;
        }
        units += character.len_utf16();
    }
    text.len()
}

/// Where a byte offset into `text` stands, counted in UTF-16 units.
pub fn utf16_at(text: &str, byte: usize) -> u32 {
    text[..byte].encode_utf16().count() as u32
}

/// Where each character of `text` starts, counted in UTF-16 units. Harper counts in
/// characters and Qt counts in UTF-16 units, and an emoji is one of the first and two of
/// the second.
pub fn utf16_offsets(text: &str) -> Vec<u32> {
    let mut offsets = Vec::with_capacity(text.len() + 1);
    let mut units = 0;
    for character in text.chars() {
        offsets.push(units);
        units += character.len_utf16() as u32;
    }
    offsets.push(units);
    offsets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_an_emoji_as_two() {
        assert_eq!(utf16_at("🙂 a", 5), 3);
        assert_eq!(byte_offset("🙂 a", 3), 5);
        assert_eq!(utf16_offsets("🙂a"), [0, 2, 3]);
    }

    #[test]
    fn stops_at_the_end_rather_than_running_past_it() {
        assert_eq!(byte_offset("ab", 9), 2);
        assert_eq!(byte_offset("ab", -1), 2);
    }
}
