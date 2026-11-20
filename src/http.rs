//! Minimal HTTP/1.x response inspection for the plain-text network clients.

/// The status code of an HTTP/1.x response.
pub fn status_code(response: &str) -> Option<u16> {
    let rest = response
        .strip_prefix("HTTP/1.1 ")
        .or_else(|| response.strip_prefix("HTTP/1.0 "))?;
    rest.get(..3)?.parse().ok()
}

/// Whether an HTTP response has a 2xx status.
pub fn is_success(response: &str) -> bool {
    status_code(response).is_some_and(|status| (200..300).contains(&status))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_status_codes() {
        assert_eq!(status_code("HTTP/1.1 201 Created\r\n"), Some(201));
        assert_eq!(status_code("HTTP/1.0 301 Moved Permanently\r\n"), Some(301));
        assert_eq!(status_code("HTTP/2 200\r\n"), None);
        assert_eq!(status_code(""), None);
    }

    #[test]
    fn only_2xx_is_success() {
        assert!(is_success("HTTP/1.1 200 OK\r\n"));
        assert!(is_success("HTTP/1.0 204 No Content\r\n"));
        assert!(!is_success("HTTP/1.1 301 Moved Permanently\r\n"));
        assert!(!is_success("HTTP/1.1 400 Bad Request\r\n"));
        assert!(!is_success("garbage"));
    }
}
