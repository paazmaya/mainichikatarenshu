//! Validation and types for Wi-Fi station provisioning.

use heapless::String;
use serde::{Deserialize, Serialize};

/// Validated WPA2-Personal station credentials.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Credentials {
    /// Network name: 1–32 printable ASCII bytes.
    pub ssid: String<32>,
    /// Passphrase: 8–63 printable ASCII bytes.
    pub password: String<63>,
}

/// Which credential failed validation.
#[derive(Debug, Eq, PartialEq)]
pub enum CredentialError {
    /// The SSID is empty, too long or not printable ASCII.
    InvalidSsid,
    /// The passphrase is not 8–63 printable ASCII bytes.
    InvalidPassword,
}

impl Credentials {
    /// Validate and copy `ssid` and `password`.
    pub fn new(ssid: &str, password: &str) -> Result<Self, CredentialError> {
        if ssid.is_empty() || ssid.len() > 32 || !ssid.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
            return Err(CredentialError::InvalidSsid);
        }
        if !(8..=63).contains(&password.len())
            || !password.bytes().all(|b| (0x20..=0x7e).contains(&b))
        {
            return Err(CredentialError::InvalidPassword);
        }
        Ok(Self {
            ssid: String::try_from(ssid).map_err(|_| CredentialError::InvalidSsid)?,
            password: String::try_from(password).map_err(|_| CredentialError::InvalidPassword)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_station_credentials() {
        assert!(Credentials::new("network name", "password").is_ok());
        assert!(Credentials::new(&"x".repeat(32), &"p".repeat(63)).is_ok());
        assert!(matches!(
            Credentials::new("", "password"),
            Err(CredentialError::InvalidSsid)
        ));
        assert!(matches!(
            Credentials::new(&"x".repeat(33), "password"),
            Err(CredentialError::InvalidSsid)
        ));
        assert!(matches!(
            Credentials::new("network", "short"),
            Err(CredentialError::InvalidPassword)
        ));
        assert!(matches!(
            Credentials::new("network", &"p".repeat(64)),
            Err(CredentialError::InvalidPassword)
        ));
        assert!(matches!(
            Credentials::new("network", "invalid\npassword"),
            Err(CredentialError::InvalidPassword)
        ));
    }
}
