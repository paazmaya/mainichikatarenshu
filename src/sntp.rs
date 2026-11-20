//! SNTP (Simple Network Time Protocol) packet creation and parsing.
//!
//! SNTP operates over UDP on port 123. It exchanges 48-byte packets.
//! The epoch offset between NTP (1900-01-01) and Unix (1970-01-01) is 2,208,988,800 seconds.

/// NTP timestamp epoch difference: seconds between 1900-01-01 and 1970-01-01.
pub const NTP_TO_UNIX_OFFSET_SECS: u64 = 2_208_988_800;

/// SNTP packet size in bytes.
pub const SNTP_PACKET_SIZE: usize = 48;

/// Standard NTP UDP port.
pub const NTP_PORT: u16 = 123;

/// Errors that can occur when parsing an SNTP response.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SntpError {
    /// The packet is shorter than 48 bytes.
    BufferTooShort,
    /// Stratum 0: a kiss-o'-death or unsynchronized server.
    InvalidStratum,
    /// The transmit timestamp predates the Unix epoch.
    InvalidTimestamp,
}

/// Creates a standard SNTP client request packet (48 bytes).
///
/// Header:
/// - LI (Leap Indicator) = 0 (no warning)
/// - VN (Version Number) = 4 (NTPv4)
/// - Mode = 3 (Client)
///
/// Byte 0 = (0 << 6) | (4 << 3) | 3 = 0x23 (or 0x1B for v3, 0x23 for v4).
pub fn create_request() -> [u8; SNTP_PACKET_SIZE] {
    let mut packet = [0u8; SNTP_PACKET_SIZE];
    packet[0] = 0x23; // LI = 0, Version = 4, Mode = 3 (client)
    packet
}

/// Parses a 48-byte SNTP response packet and returns the transmit timestamp as Unix seconds.
///
/// Transmit timestamp is stored at bytes 40..48:
/// - bytes 40..44: seconds since 1900-01-01 00:00:00 UTC (big-endian)
/// - bytes 44..48: fractional seconds
pub fn parse_response(packet: &[u8]) -> Result<u64, SntpError> {
    if packet.len() < SNTP_PACKET_SIZE {
        return Err(SntpError::BufferTooShort);
    }

    // Stratum 0 indicates Kiss-o'-Death (KoD) packet or invalid server
    let stratum = packet[1];
    if stratum == 0 {
        return Err(SntpError::InvalidStratum);
    }

    // Transmit Timestamp (seconds) is at bytes 40..44
    let ntp_seconds = u32::from_be_bytes([packet[40], packet[41], packet[42], packet[43]]) as u64;

    if ntp_seconds <= NTP_TO_UNIX_OFFSET_SECS {
        return Err(SntpError::InvalidTimestamp);
    }

    let unix_seconds = ntp_seconds - NTP_TO_UNIX_OFFSET_SECS;
    Ok(unix_seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_packet_has_correct_client_mode_and_version() {
        let req = create_request();
        assert_eq!(req.len(), 48);
        assert_eq!(req[0], 0x23);
        assert!(req[1..].iter().all(|&b| b == 0));
    }

    #[test]
    fn parse_valid_sntp_response() {
        let mut packet = [0u8; 48];
        packet[0] = 0x24; // Server response
        packet[1] = 2; // Stratum 2 server

        // 2026-11-20 10:00:00 UTC
        // Unix timestamp = 1795168800
        // NTP timestamp = 1795168800 + 2208988800 = 4004157600
        let unix_expected = 1_795_168_800u64;
        let ntp_time = (unix_expected + NTP_TO_UNIX_OFFSET_SECS) as u32;
        packet[40..44].copy_from_slice(&ntp_time.to_be_bytes());

        let result = parse_response(&packet);
        assert_eq!(result, Ok(unix_expected));
    }

    #[test]
    fn rejects_stratum_zero_or_invalid_buffer() {
        let mut packet = [0u8; 48];
        packet[1] = 0; // Kiss-o'-Death
        assert_eq!(parse_response(&packet), Err(SntpError::InvalidStratum));

        assert_eq!(
            parse_response(&packet[..30]),
            Err(SntpError::BufferTooShort)
        );
    }

    #[test]
    fn rejects_timestamp_before_unix_epoch() {
        let mut packet = [0u8; 48];
        packet[1] = 1;
        packet[40..44].copy_from_slice(&1000u32.to_be_bytes()); // Way before 1970
        assert_eq!(parse_response(&packet), Err(SntpError::InvalidTimestamp));
    }
}
