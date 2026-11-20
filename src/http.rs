//! Bounded HTTP/1.x response reading and validation for the network clients.

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

/// A validated HTTP response.
#[derive(Debug, Eq, PartialEq)]
pub struct Response<'a> {
    /// HTTP status code.
    pub status: u16,
    /// Decoded response body, after the headers.
    pub body: &'a [u8],
}

impl Response<'_> {
    /// Whether the response has a 2xx status.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// A response cannot be safely interpreted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseError {
    /// Headers or chunk framing are invalid or exceed limits.
    Malformed,
    /// The headers or chunked body are incomplete.
    Incomplete,
    /// Informational responses are not supported.
    Informational,
    /// Transfer encoding is unsupported (or chunked in immutable parsing).
    UnsupportedTransferEncoding,
    /// Content-Length is invalid, duplicated, or combined with Transfer-Encoding.
    InvalidContentLength,
    /// The received body length does not match framing headers.
    LengthMismatch,
    /// A response that must not have a body contains one.
    UnexpectedBody,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BodyFraming {
    None,
    ContentLength(usize),
    Chunked,
    CloseDelimited,
}

struct ParsedHead {
    status: u16,
    body_start: usize,
    framing: BodyFraming,
}

fn parse_head(buffer: &[u8]) -> Result<Option<ParsedHead>, ResponseError> {
    let mut headers = [httparse::EMPTY_HEADER; 48];
    let mut parsed = httparse::Response::new(&mut headers);
    let body_start = match parsed.parse(buffer).map_err(|_| ResponseError::Malformed)? {
        httparse::Status::Complete(length) => length,
        httparse::Status::Partial => return Ok(None),
    };
    let status = parsed.code.ok_or(ResponseError::Malformed)?;
    if !(100..600).contains(&status) {
        return Err(ResponseError::Malformed);
    }
    if status < 200 {
        return Err(ResponseError::Informational);
    }
    let mut content_length = None;
    let mut is_chunked = false;
    for header in parsed.headers.iter() {
        if header.name.eq_ignore_ascii_case("transfer-encoding") {
            if is_chunked {
                return Err(ResponseError::UnsupportedTransferEncoding);
            }
            let value = core::str::from_utf8(header.value)
                .map_err(|_| ResponseError::UnsupportedTransferEncoding)?
                .trim_matches([' ', '\t']);
            if !value.eq_ignore_ascii_case("chunked") {
                return Err(ResponseError::UnsupportedTransferEncoding);
            }
            is_chunked = true;
        } else if header.name.eq_ignore_ascii_case("content-length") {
            if content_length.is_some() {
                return Err(ResponseError::InvalidContentLength);
            }
            let value = core::str::from_utf8(header.value)
                .map_err(|_| ResponseError::InvalidContentLength)?
                .trim_matches([' ', '\t']);
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(ResponseError::InvalidContentLength);
            }
            content_length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| ResponseError::InvalidContentLength)?,
            );
        }
    }
    if is_chunked && content_length.is_some() {
        return Err(ResponseError::InvalidContentLength);
    }
    let framing = if matches!(status, 204 | 304) {
        if is_chunked || content_length.is_some_and(|len| len > 0) {
            return Err(ResponseError::UnexpectedBody);
        }
        BodyFraming::None
    } else if is_chunked {
        BodyFraming::Chunked
    } else if let Some(length) = content_length {
        BodyFraming::ContentLength(length)
    } else {
        BodyFraming::CloseDelimited
    };
    Ok(Some(ParsedHead {
        status,
        body_start,
        framing,
    }))
}

/// Validate an unchunked response read through EOF.
///
/// Responses with a `Content-Length` must contain exactly that many body bytes.
/// Transfer encodings are rejected here; use [`parse_response_mut`] or
/// [`read_and_parse_response`] to decode `Transfer-Encoding: chunked`.
pub fn parse_response(buffer: &[u8]) -> Result<Response<'_>, ResponseError> {
    let head = parse_head(buffer)?.ok_or(ResponseError::Incomplete)?;
    let body = &buffer[head.body_start..];
    match head.framing {
        BodyFraming::None => {
            if !body.is_empty() {
                return Err(ResponseError::UnexpectedBody);
            }
        }
        BodyFraming::ContentLength(length) => {
            if body.len() != length {
                return Err(ResponseError::LengthMismatch);
            }
        }
        BodyFraming::Chunked => return Err(ResponseError::UnsupportedTransferEncoding),
        BodyFraming::CloseDelimited => {}
    }
    Ok(Response {
        status: head.status,
        body,
    })
}

/// Validate a complete response in `buffer`, decoding `Transfer-Encoding: chunked` in place.
pub fn parse_response_mut(buffer: &mut [u8]) -> Result<Response<'_>, ResponseError> {
    let head = parse_head(buffer)?.ok_or(ResponseError::Incomplete)?;
    try_finish_response(&head, buffer, true)?.ok_or(ResponseError::Incomplete)
}

fn try_finish_response<'b>(
    head: &ParsedHead,
    buffer: &'b mut [u8],
    eof: bool,
) -> Result<Option<Response<'b>>, ResponseError> {
    let body = &mut buffer[head.body_start..];
    match head.framing {
        BodyFraming::None => {
            if !body.is_empty() {
                return Err(ResponseError::UnexpectedBody);
            }
            Ok(Some(Response {
                status: head.status,
                body: &[],
            }))
        }
        BodyFraming::ContentLength(expected) => {
            if body.len() > expected || (eof && body.len() < expected) {
                return Err(ResponseError::LengthMismatch);
            }
            if body.len() == expected {
                Ok(Some(Response {
                    status: head.status,
                    body,
                }))
            } else {
                Ok(None)
            }
        }
        BodyFraming::Chunked => match inspect_chunked(body)? {
            Some(_) => {
                let decoded_len = decode_chunked_in_place(body)?;
                Ok(Some(Response {
                    status: head.status,
                    body: &body[..decoded_len],
                }))
            }
            None if eof => Err(ResponseError::Incomplete),
            None => Ok(None),
        },
        BodyFraming::CloseDelimited => {
            if eof {
                Ok(Some(Response {
                    status: head.status,
                    body,
                }))
            } else {
                Ok(None)
            }
        }
    }
}

fn find_crlf(bytes: &[u8]) -> Result<Option<usize>, ResponseError> {
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' => {
                if index + 1 >= bytes.len() {
                    return Ok(None);
                }
                if bytes[index + 1] == b'\n' {
                    return Ok(Some(index));
                }
                return Err(ResponseError::Malformed);
            }
            b'\n' => return Err(ResponseError::Malformed),
            _ => index += 1,
        }
    }
    Ok(None)
}

fn parse_chunk_size(line: &[u8]) -> Result<usize, ResponseError> {
    let size_part = line.split(|&byte| byte == b';').next().unwrap_or(&[]);
    let size_part = match core::str::from_utf8(size_part) {
        Ok(text) => text.trim_matches([' ', '\t']),
        Err(_) => return Err(ResponseError::Malformed),
    };
    if size_part.is_empty() {
        return Err(ResponseError::Malformed);
    }
    let mut value = 0usize;
    for byte in size_part.bytes() {
        let digit = match byte {
            b'0'..=b'9' => usize::from(byte - b'0'),
            b'a'..=b'f' => usize::from(byte - b'a' + 10),
            b'A'..=b'F' => usize::from(byte - b'A' + 10),
            _ => return Err(ResponseError::Malformed),
        };
        value = value
            .checked_mul(16)
            .and_then(|acc| acc.checked_add(digit))
            .ok_or(ResponseError::Malformed)?;
    }
    Ok(value)
}

/// Inspect a chunked body, returning `Ok(None)` if more bytes are needed or
/// `Ok(Some(decoded_len))` when the terminating chunk and trailers end cleanly at `raw.len()`.
fn inspect_chunked(raw: &[u8]) -> Result<Option<usize>, ResponseError> {
    const MAX_CHUNK_LINE_LEN: usize = 128;
    let mut pos = 0;
    let mut decoded_len = 0usize;
    loop {
        let Some(line_offset) = find_crlf(&raw[pos..])? else {
            if raw.len() - pos > MAX_CHUNK_LINE_LEN {
                return Err(ResponseError::Malformed);
            }
            return Ok(None);
        };
        if line_offset > MAX_CHUNK_LINE_LEN {
            return Err(ResponseError::Malformed);
        }
        let chunk_size = parse_chunk_size(&raw[pos..pos + line_offset])?;
        pos += line_offset + 2;
        if chunk_size == 0 {
            loop {
                let Some(trailer_offset) = find_crlf(&raw[pos..])? else {
                    return Ok(None);
                };
                if trailer_offset == 0 {
                    pos += 2;
                    if pos != raw.len() {
                        return Err(ResponseError::LengthMismatch);
                    }
                    return Ok(Some(decoded_len));
                }
                if !raw[pos..pos + trailer_offset].contains(&b':') {
                    return Err(ResponseError::Malformed);
                }
                pos += trailer_offset + 2;
            }
        }
        decoded_len = decoded_len
            .checked_add(chunk_size)
            .ok_or(ResponseError::Malformed)?;
        let chunk_end = pos
            .checked_add(chunk_size)
            .ok_or(ResponseError::Malformed)?;
        let crlf_end = chunk_end.checked_add(2).ok_or(ResponseError::Malformed)?;
        if raw.len() < crlf_end {
            if raw.len() > chunk_end && raw[chunk_end] != b'\r' {
                return Err(ResponseError::Malformed);
            }
            return Ok(None);
        }
        if &raw[chunk_end..crlf_end] != b"\r\n" {
            return Err(ResponseError::Malformed);
        }
        pos = crlf_end;
    }
}

fn decode_chunked_in_place(raw: &mut [u8]) -> Result<usize, ResponseError> {
    let mut read_pos = 0;
    let mut write_pos = 0;
    loop {
        let line_offset = find_crlf(&raw[read_pos..])?.ok_or(ResponseError::Incomplete)?;
        let chunk_size = parse_chunk_size(&raw[read_pos..read_pos + line_offset])?;
        read_pos += line_offset + 2;
        if chunk_size == 0 {
            return Ok(write_pos);
        }
        let chunk_end = read_pos + chunk_size;
        raw.copy_within(read_pos..chunk_end, write_pos);
        write_pos += chunk_size;
        read_pos = chunk_end + 2;
    }
}

/// Failure while reading and validating an HTTP response from a stream.
#[cfg(any(feature = "wifi", feature = "test-utils"))]
#[derive(Debug, Eq, PartialEq)]
pub enum ReadResponseError<E> {
    /// The peer sent more data than fits in the response buffer.
    TooLarge,
    /// The stream failed before the response completed.
    Read(E),
    /// The received HTTP message was malformed, truncated or unsupported.
    Protocol(ResponseError),
}

/// Read until EOF, rejecting I/O errors and responses larger than `buffer`.
#[cfg(any(feature = "wifi", feature = "test-utils"))]
pub async fn read_response<R: embedded_io_async::Read>(
    reader: &mut R,
    buffer: &mut [u8],
) -> Result<usize, ReadResponseError<R::Error>> {
    let mut length = 0;
    loop {
        let mut overflow = [0];
        let destination = if length == buffer.len() {
            &mut overflow[..]
        } else {
            &mut buffer[length..]
        };
        let read = reader
            .read(destination)
            .await
            .map_err(ReadResponseError::Read)?;
        if read == 0 {
            return Ok(length);
        }
        if length == buffer.len() {
            return Err(ReadResponseError::TooLarge);
        }
        length += read;
    }
}

/// Read and validate a complete HTTP/1.x response, stopping once self-delimited
/// bodies (`Content-Length`, `Transfer-Encoding: chunked`, `204`/`304`) complete.
#[cfg(any(feature = "wifi", feature = "test-utils"))]
pub async fn read_and_parse_response<'b, R: embedded_io_async::Read>(
    reader: &mut R,
    buffer: &'b mut [u8],
) -> Result<Response<'b>, ReadResponseError<R::Error>> {
    let capacity = buffer.len();
    let mut length = 0usize;
    let mut head: Option<ParsedHead> = None;

    loop {
        if head.is_none() {
            head = parse_head(&buffer[..length]).map_err(ReadResponseError::Protocol)?;
            if let Some(ParsedHead {
                body_start,
                framing: BodyFraming::ContentLength(expected),
                ..
            }) = head
            {
                if body_start
                    .checked_add(expected)
                    .is_none_or(|total| total > capacity)
                {
                    return Err(ReadResponseError::TooLarge);
                }
            }
        }

        if let Some(ref parsed_head) = head {
            let ready = match parsed_head.framing {
                BodyFraming::None => true,
                BodyFraming::ContentLength(expected) => length - parsed_head.body_start >= expected,
                BodyFraming::Chunked => inspect_chunked(&buffer[parsed_head.body_start..length])
                    .map_err(ReadResponseError::Protocol)?
                    .is_some(),
                BodyFraming::CloseDelimited => false,
            };
            if ready {
                return try_finish_response(parsed_head, &mut buffer[..length], true)
                    .map_err(ReadResponseError::Protocol)?
                    .ok_or(ReadResponseError::Protocol(ResponseError::Incomplete));
            }
        }

        let mut overflow = [0u8; 1];
        let destination = if length == capacity {
            &mut overflow[..]
        } else {
            &mut buffer[length..]
        };
        let read = reader
            .read(destination)
            .await
            .map_err(ReadResponseError::Read)?;
        if read == 0 {
            let parsed_head = head.ok_or(ReadResponseError::Protocol(ResponseError::Incomplete))?;
            return try_finish_response(&parsed_head, &mut buffer[..length], true)
                .map_err(ReadResponseError::Protocol)?
                .ok_or(ReadResponseError::Protocol(ResponseError::Incomplete));
        }
        if length == capacity {
            return Err(ReadResponseError::TooLarge);
        }
        length += read;
    }
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

    #[test]
    fn validates_complete_and_close_delimited_responses() {
        let response = parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabc").unwrap();
        assert!(response.is_success());
        assert_eq!(response.body, b"abc");
        let response = parse_response(b"HTTP/1.0 204 No Content\r\n\r\n").unwrap();
        assert!(response.is_success());
        assert!(response.body.is_empty());
        let response = parse_response(b"HTTP/1.1 400 Bad Request\r\n\r\nerror").unwrap();
        assert!(!response.is_success());
        assert_eq!(response.body, b"error");
    }

    #[test]
    fn rejects_incomplete_and_ambiguous_responses() {
        for (response, error) in [
            (&b"HTTP/1.1 200 OK\r\n"[..], ResponseError::Incomplete),
            (
                &b"HTTP/1.1 200 OK\r\nBad Header\r\n\r\n"[..],
                ResponseError::Malformed,
            ),
            (
                &b"HTTP/1.1 100 Continue\r\n\r\n"[..],
                ResponseError::Informational,
            ),
            (
                &b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nabc"[..],
                ResponseError::LengthMismatch,
            ),
            (
                &b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nabc"[..],
                ResponseError::LengthMismatch,
            ),
            (
                &b"HTTP/1.1 200 OK\r\nContent-Length: +1\r\n\r\na"[..],
                ResponseError::InvalidContentLength,
            ),
            (
                &b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\ncontent-length: 1\r\n\r\na"[..],
                ResponseError::InvalidContentLength,
            ),
            (
                &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"[..],
                ResponseError::UnsupportedTransferEncoding,
            ),
            (
                &b"HTTP/1.1 204 No Content\r\n\r\nx"[..],
                ResponseError::UnexpectedBody,
            ),
        ] {
            assert_eq!(parse_response(response), Err(error), "{response:?}");
        }
    }

    #[cfg(feature = "test-utils")]
    mod response_reads {
        use super::*;
        use embassy_futures::block_on;
        use embedded_io_async::{ErrorType, Read};

        struct Reader<'a> {
            chunks: &'a [&'a [u8]],
            offset: usize,
            fail_after_chunks: bool,
        }

        impl ErrorType for Reader<'_> {
            type Error = embedded_io_async::ErrorKind;
        }

        impl Read for Reader<'_> {
            async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, Self::Error> {
                if let Some((chunk, remaining)) = self.chunks.split_first() {
                    let length = (chunk.len() - self.offset).min(buffer.len());
                    buffer[..length].copy_from_slice(&chunk[self.offset..self.offset + length]);
                    self.offset += length;
                    if self.offset == chunk.len() {
                        self.chunks = remaining;
                        self.offset = 0;
                    }
                    Ok(length)
                } else if self.fail_after_chunks {
                    Err(embedded_io_async::ErrorKind::Other)
                } else {
                    Ok(0)
                }
            }
        }

        #[test]
        fn reads_fragmented_and_exact_capacity_responses() {
            let mut reader = Reader {
                chunks: &[b"ab", b"cd"],
                offset: 0,
                fail_after_chunks: false,
            };
            let mut buffer = [0; 4];
            assert_eq!(block_on(read_response(&mut reader, &mut buffer)), Ok(4));
            assert_eq!(&buffer, b"abcd");
        }

        #[test]
        fn rejects_oversized_and_failed_reads() {
            let mut buffer = [0; 4];
            let mut oversized = Reader {
                chunks: &[b"abcd", b"e"],
                offset: 0,
                fail_after_chunks: false,
            };
            assert_eq!(
                block_on(read_response(&mut oversized, &mut buffer)),
                Err(ReadResponseError::TooLarge)
            );
            let mut failed = Reader {
                chunks: &[b"abc"],
                offset: 0,
                fail_after_chunks: true,
            };
            assert_eq!(
                block_on(read_response(&mut failed, &mut buffer)),
                Err(ReadResponseError::Read(embedded_io_async::ErrorKind::Other))
            );
        }

        #[test]
        fn decodes_fragmented_chunked_and_self_delimited_responses() {
            let mut reader = Reader {
                chunks: &[
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: ",
                    b"chunked\r\n\r\n4;ext=1\r\n{\"pr",
                    b"\r\n9\r\nice\":7.5}\r\n0\r\nX-Trailer: ok\r\n\r\n",
                ],
                offset: 0,
                fail_after_chunks: true, // Stops at chunk end before hitting post-chunk error!
            };
            let mut buffer = [0u8; 256];
            let response = block_on(read_and_parse_response(&mut reader, &mut buffer)).unwrap();
            assert_eq!(response.status, 200);
            assert_eq!(response.body, br#"{"price":7.5}"#);

            let mut empty_chunked = Reader {
                chunks: &[b"HTTP/1.1 201 Created\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"],
                offset: 0,
                fail_after_chunks: true,
            };
            let response =
                block_on(read_and_parse_response(&mut empty_chunked, &mut buffer)).unwrap();
            assert_eq!(response.status, 201);
            assert!(response.body.is_empty());

            // Self-delimited Content-Length and 204 stop immediately without reading further.
            let mut content_len = Reader {
                chunks: &[b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}"],
                offset: 0,
                fail_after_chunks: true,
            };
            let response =
                block_on(read_and_parse_response(&mut content_len, &mut buffer)).unwrap();
            assert_eq!(response.body, b"{}");

            let mut no_content = Reader {
                chunks: &[b"HTTP/1.1 204 No Content\r\n\r\n"],
                offset: 0,
                fail_after_chunks: true,
            };
            let response = block_on(read_and_parse_response(&mut no_content, &mut buffer)).unwrap();
            assert_eq!(response.status, 204);
            assert!(response.body.is_empty());

            // Close-delimited response reads until clean EOF, including uppercase hex in chunked.
            let mut close_delimited = Reader {
                chunks: &[b"HTTP/1.1 200 OK\r\n\r\nhello", b" world"],
                offset: 0,
                fail_after_chunks: false,
            };
            let response =
                block_on(read_and_parse_response(&mut close_delimited, &mut buffer)).unwrap();
            assert_eq!(response.body, b"hello world");

            let mut upper_hex =
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nA\r\n0123456789\r\n0\r\n\r\n"
                    .to_vec();
            assert_eq!(
                parse_response_mut(&mut upper_hex).unwrap().body,
                b"0123456789"
            );
        }

        #[test]
        fn rejects_invalid_chunked_smuggling_and_truncated_streams() {
            for (raw, expected) in [
                (
                    &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 0\r\n\r\n0\r\n\r\n"[..],
                    ResponseError::InvalidContentLength,
                ),
                (
                    &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip\r\n\r\n"[..],
                    ResponseError::UnsupportedTransferEncoding,
                ),
                (
                    &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"[..],
                    ResponseError::UnsupportedTransferEncoding,
                ),
                (
                    &b"HTTP/1.1 204 No Content\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n"[..],
                    ResponseError::UnexpectedBody,
                ),
                (
                    &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nab"[..],
                    ResponseError::Incomplete,
                ),
                (
                    &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n;ext\r\n\r\n"[..],
                    ResponseError::Malformed,
                ),
                (
                    &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\n\r\n"[..],
                    ResponseError::Malformed,
                ),
                (
                    &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\rx\r\n"[..],
                    ResponseError::Malformed,
                ),
                (
                    &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\r\nbad-trailer\r\n\r\n"[..],
                    ResponseError::Malformed,
                ),
                (
                    &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n\r\n"[..],
                    ResponseError::Malformed,
                ),
                (
                    &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nabXX0\r\n\r\n"[..],
                    ResponseError::Malformed,
                ),
                (
                    &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\nextra"[..],
                    ResponseError::LengthMismatch,
                ),
            ] {
                let mut copy = raw.to_vec();
                assert_eq!(parse_response_mut(&mut copy), Err(expected), "{raw:?}");
            }

            let mut small = [0u8; 48];
            let mut declared_huge = Reader {
                chunks: &[b"HTTP/1.1 200 OK\r\nContent-Length: 999\r\n\r\n{}"],
                offset: 0,
                fail_after_chunks: false,
            };
            assert_eq!(
                block_on(read_and_parse_response(&mut declared_huge, &mut small)),
                Err(ReadResponseError::TooLarge)
            );
        }
    }
}
