//! Line-based serial console: byte buffering and command parsing.
//!
//! Commands are ASCII lines terminated by `\r` or `\n`:
//!
//! | Command | Meaning |
//! |---|---|
//! | `TIME <UTC unix seconds>` | Set the clock |
//! | `LIST` | Print pending records |
//! | `ACK <YYYY-MM-DD>` | Remove the oldest pending record |
//! | `SSID <name>` | Stage a Wi-Fi network name (`wifi` builds) |
//! | `PASSWORD <passphrase>` | Stage a Wi-Fi passphrase (`wifi` builds) |
//! | `CONNECT` | Save and use the staged Wi-Fi credentials (`wifi` builds) |

use chrono::NaiveDate;
use heapless::String;

use crate::clock;

/// Why a received line was discarded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineError {
    /// The line exceeded the buffer capacity.
    TooLong,
    /// The line contained a non-ASCII byte.
    NotAscii,
}

/// Accumulates bytes until a line terminator.
///
/// After an error the rest of the line is discarded and the error is
/// reported once, at the terminator.
#[derive(Debug, Default)]
pub struct LineBuffer<const N: usize> {
    line: String<N>,
    error: Option<LineError>,
}

impl<const N: usize> LineBuffer<N> {
    /// Create an empty buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed one byte. Returns a complete non-empty line or its error.
    pub fn push(&mut self, byte: u8) -> Option<Result<String<N>, LineError>> {
        match byte {
            b'\r' | b'\n' => {
                if let Some(error) = self.error.take() {
                    self.line.clear();
                    Some(Err(error))
                } else if self.line.is_empty() {
                    None
                } else {
                    Some(Ok(core::mem::take(&mut self.line)))
                }
            }
            _ if self.error.is_some() => None,
            byte if byte.is_ascii() => {
                if self.line.push(char::from(byte)).is_err() {
                    self.error = Some(LineError::TooLong);
                }
                None
            }
            _ => {
                self.error = Some(LineError::NotAscii);
                None
            }
        }
    }
}

/// A parsed console command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command<'a> {
    /// Set the RTC to the given microseconds since the Unix epoch.
    SetTime(u64),
    /// Print pending records.
    List,
    /// Acknowledge the pending record for the given date.
    Acknowledge(NaiveDate),
    /// Stage a Wi-Fi SSID.
    Ssid(&'a str),
    /// Stage a Wi-Fi passphrase.
    Password(&'a str),
    /// Save and use the staged Wi-Fi credentials.
    Connect,
}

/// A line that is not a valid command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandError {
    /// `TIME` without a plausible UTC Unix timestamp.
    InvalidTime,
    /// `ACK` without a `YYYY-MM-DD` date.
    InvalidDate,
    /// Not a known command.
    Unknown,
}

/// Parse one console line.
pub fn parse_command(line: &str) -> Result<Command<'_>, CommandError> {
    if let Some(value) = line.strip_prefix("TIME ") {
        clock::parse_unix_secs(value)
            .map(Command::SetTime)
            .ok_or(CommandError::InvalidTime)
    } else if let Some(value) = line.strip_prefix("ACK ") {
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map(Command::Acknowledge)
            .map_err(|_| CommandError::InvalidDate)
    } else if let Some(value) = line.strip_prefix("SSID ") {
        Ok(Command::Ssid(value))
    } else if let Some(value) = line.strip_prefix("PASSWORD ") {
        Ok(Command::Password(value))
    } else {
        match line {
            "LIST" => Ok(Command::List),
            "CONNECT" => Ok(Command::Connect),
            _ => Err(CommandError::Unknown),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed<const N: usize>(
        buffer: &mut LineBuffer<N>,
        bytes: &[u8],
    ) -> Option<Result<String<N>, LineError>> {
        bytes.iter().filter_map(|byte| buffer.push(*byte)).last()
    }

    #[test]
    fn line_buffer_yields_lines_and_skips_blank_ones() {
        let mut buffer = LineBuffer::<8>::new();
        assert_eq!(feed(&mut buffer, b"\r\n\n"), None);
        assert_eq!(
            feed(&mut buffer, b"LIST\r\n"),
            Some(Ok("LIST".try_into().unwrap()))
        );
        assert_eq!(
            feed(&mut buffer, b"ACK\n"),
            Some(Ok("ACK".try_into().unwrap()))
        );
    }

    #[test]
    fn line_buffer_reports_and_recovers_from_invalid_lines() {
        let mut buffer = LineBuffer::<4>::new();
        assert_eq!(
            feed(&mut buffer, b"TOO LONG\n"),
            Some(Err(LineError::TooLong))
        );
        assert_eq!(
            feed(&mut buffer, &[b'A', 0xc3, 0xa4, b'\n']),
            Some(Err(LineError::NotAscii))
        );
        assert_eq!(
            feed(&mut buffer, b"LIST\n"),
            Some(Ok("LIST".try_into().unwrap()))
        );
    }

    #[test]
    fn parses_commands() {
        assert_eq!(
            parse_command("TIME 1767225600"),
            Ok(Command::SetTime(1_767_225_600_000_000))
        );
        assert_eq!(parse_command("TIME 5"), Err(CommandError::InvalidTime));
        assert_eq!(parse_command("LIST"), Ok(Command::List));
        assert_eq!(
            parse_command("ACK 2026-11-20"),
            Ok(Command::Acknowledge(
                NaiveDate::from_ymd_opt(2026, 11, 20).unwrap()
            ))
        );
        assert_eq!(
            parse_command("ACK 20.11.2026"),
            Err(CommandError::InvalidDate)
        );
        assert_eq!(parse_command("SSID my net"), Ok(Command::Ssid("my net")));
        assert_eq!(
            parse_command("PASSWORD secret 1"),
            Ok(Command::Password("secret 1"))
        );
        assert_eq!(parse_command("CONNECT"), Ok(Command::Connect));
        assert_eq!(parse_command("list"), Err(CommandError::Unknown));
    }
}
