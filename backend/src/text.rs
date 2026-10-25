//! Petits outils de texte partagés.

/// Décode les séquences `%XX` (les autres octets restent tels quels). Travaille sur les
/// octets : un `%` suivi d'un caractère multi-octets ne fait pas paniquer.
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 3 <= b.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_without_panicking() {
        assert_eq!(percent_decode("caf%C3%A9.html"), "café.html");
        assert_eq!(percent_decode("%€x"), "%€x");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("%2Fcal%20x/"), "/cal x/");
    }
}
