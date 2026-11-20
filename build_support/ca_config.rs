use std::error::Error;

const BEGIN_CERT: &str = "-----BEGIN CERTIFICATE-----";
const END_CERT: &str = "-----END CERTIFICATE-----";

/// Validate a PEM root CA bundle and return normalized PEM blocks without comments.
pub fn parse_pem_bundle(contents: &str) -> Result<String, Box<dyn Error>> {
    if contents.contains('\0') {
        return Err("CA bundle must not contain NUL bytes".into());
    }

    let mut normalized = String::new();
    let mut in_cert = false;
    let mut cert_b64_len = 0usize;
    let mut cert_count = 0usize;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if !in_cert {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line == BEGIN_CERT {
                in_cert = true;
                cert_b64_len = 0;
                normalized.push_str(BEGIN_CERT);
                normalized.push('\n');
            } else {
                return Err(
                    format!("unexpected content outside certificate block: '{line}'").into(),
                );
            }
        } else if line == END_CERT {
            if cert_b64_len == 0 || !cert_b64_len.is_multiple_of(4) {
                return Err("certificate block has invalid base64 payload".into());
            }
            in_cert = false;
            cert_count += 1;
            normalized.push_str(END_CERT);
            normalized.push('\n');
        } else {
            if line.is_empty()
                || !line
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
            {
                return Err(format!("invalid base64 line in certificate block: '{line}'").into());
            }
            cert_b64_len += line.len();
            normalized.push_str(line);
            normalized.push('\n');
        }
    }

    if in_cert {
        return Err("unterminated certificate block in CA bundle".into());
    }
    if cert_count == 0 {
        return Err("CA bundle must contain at least one certificate".into());
    }

    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_commented_pem_bundles() {
        let pem = "# Root 1\n-----BEGIN CERTIFICATE-----\nQUJD\n-----END CERTIFICATE-----\n";
        assert_eq!(
            parse_pem_bundle(pem).unwrap(),
            "-----BEGIN CERTIFICATE-----\nQUJD\n-----END CERTIFICATE-----\n"
        );
    }

    #[test]
    fn rejects_empty_or_malformed_bundles() {
        for invalid in [
            "",
            "# comments only\n",
            "garbage",
            "-----BEGIN CERTIFICATE-----\n-----END CERTIFICATE-----",
            "-----BEGIN CERTIFICATE-----\nQUJ\n-----END CERTIFICATE-----",
            "-----BEGIN CERTIFICATE-----\nQUJD\n",
            "-----BEGIN CERTIFICATE-----\nQUJD\0\n-----END CERTIFICATE-----",
            "-----BEGIN CERTIFICATE-----\nQUJD$\n-----END CERTIFICATE-----",
        ] {
            assert!(parse_pem_bundle(invalid).is_err(), "{invalid:?}");
        }
    }
}
