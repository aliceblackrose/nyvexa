use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};

pub(crate) const CHALLENGE_PREFIX: &str = "NYVEXA-";

pub(crate) fn generate_challenge() -> String {
    let mut bytes = [0_u8; 12];
    OsRng.fill_bytes(&mut bytes);

    let mut value = String::with_capacity(CHALLENGE_PREFIX.len() + bytes.len() * 2);
    value.push_str(CHALLENGE_PREFIX);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut value, "{byte:02X}").expect("writing to String cannot fail");
    }
    value
}

pub(crate) fn hash_token(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

pub(crate) fn biography_contains_challenge(biography: &str, expected_hash: &str) -> bool {
    biography
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-')
        .filter(|candidate| candidate.starts_with(CHALLENGE_PREFIX))
        .any(|candidate| hash_token(candidate) == expected_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_challenges_have_stable_shape() {
        let challenge = generate_challenge();
        assert!(challenge.starts_with(CHALLENGE_PREFIX));
        assert_eq!(challenge.len(), CHALLENGE_PREFIX.len() + 24);
        assert!(challenge[CHALLENGE_PREFIX.len()..]
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_lowercase()));
    }

    #[test]
    fn finds_token_inside_normal_biography_text() {
        let value = "NYVEXA-0123456789ABCDEF01234567";
        let digest = hash_token(value);
        assert!(biography_contains_challenge(
            "Hello! Verification: NYVEXA-0123456789ABCDEF01234567. Nice to meet you.",
            &digest
        ));
        assert!(!biography_contains_challenge("NYVEXA-WRONG", &digest));
    }
}
