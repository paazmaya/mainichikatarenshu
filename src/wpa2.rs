//! WPA2-Personal credential preparation; no radio or connection is implemented.

/// Invalid WPA2-Personal SSID or passphrase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CredentialError {
    InvalidSsidLength,
    InvalidPassphrase,
}

/// Derive a WPA2-Personal 256-bit pairwise master key from a passphrase.
///
/// The SSID is its exact byte representation (1–32 bytes). A passphrase must
/// consist of 8–63 printable ASCII characters. A caller must protect the
/// resulting key and erase it when no longer needed.
pub fn wpa2_personal_key(ssid: &[u8], passphrase: &str) -> Result<[u8; 32], CredentialError> {
    if ssid.is_empty() || ssid.len() > 32 {
        return Err(CredentialError::InvalidSsidLength);
    }
    let password = passphrase.as_bytes();
    if !(8..=63).contains(&password.len()) || !password.iter().all(|b| (0x20..=0x7e).contains(b)) {
        return Err(CredentialError::InvalidPassphrase);
    }
    let mut key = [0; 32];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password, ssid, 4096, &mut key);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_wpa2_reference_key() {
        assert_eq!(
            wpa2_personal_key(b"IEEE", "password").unwrap(),
            [
                0xf4, 0x2c, 0x6f, 0xc5, 0x2d, 0xf0, 0xeb, 0xef, 0x9e, 0xbb, 0x4b, 0x90, 0xb3, 0x8a,
                0x5f, 0x90, 0x2e, 0x83, 0xfe, 0x1b, 0x13, 0x5a, 0x70, 0xe2, 0x3a, 0xed, 0x76, 0x2e,
                0x97, 0x10, 0xa1, 0x2e,
            ]
        );
    }

    #[test]
    fn validates_credentials() {
        assert_eq!(
            wpa2_personal_key(b"", "password"),
            Err(CredentialError::InvalidSsidLength)
        );
        assert_eq!(
            wpa2_personal_key(&[b'x'; 33], "password"),
            Err(CredentialError::InvalidSsidLength)
        );
        for passphrase in ["short", "not\nprintable", "型型型型"] {
            assert_eq!(
                wpa2_personal_key(b"ssid", passphrase),
                Err(CredentialError::InvalidPassphrase)
            );
        }
        assert_eq!(
            wpa2_personal_key(b"ssid", &"x".repeat(64)),
            Err(CredentialError::InvalidPassphrase)
        );
        assert!(wpa2_personal_key(&[b'x'; 32], &"x".repeat(63)).is_ok());
    }
}
