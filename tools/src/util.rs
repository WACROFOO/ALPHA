//! Character-safe slicing helpers (Rust strings are indexed by bytes).

/// Byte index `n` characters before `pos` (clamped at 0).
pub fn back(s: &str, pos: usize, n: usize) -> usize {
    if n == 0 {
        return pos;
    }
    s[..pos].char_indices().rev().nth(n.saturating_sub(1)).map(|(i, _)| i).unwrap_or(0)
        .min(pos)
}

/// Byte index `n` characters after `pos` (clamped at `s.len()`).
pub fn fwd(s: &str, pos: usize, n: usize) -> usize {
    s[pos..].char_indices().nth(n).map(|(i, _)| pos + i).unwrap_or(s.len())
}

/// First `n` characters of `s`.
pub fn take(s: &str, n: usize) -> &str {
    &s[..fwd(s, 0, n)]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slicing() {
        let s = "àéîôü abc";
        assert_eq!(&s[back(s, s.len(), 3)..], "abc");
        assert_eq!(&s[..fwd(s, 0, 2)], "àé");
        assert_eq!(take(s, 100), s);
        assert_eq!(back(s, 2, 10), 0);
    }
}
