//! Validation for RAM-only Wi-Fi station provisioning.

use heapless::String;

pub struct Credentials {
    pub ssid: String<32>,
    pub password: String<63>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum CredentialError {
    InvalidSsid,
    InvalidPassword,
}

impl Credentials {
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
