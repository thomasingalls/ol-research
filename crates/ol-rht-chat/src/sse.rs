//! Upstream SSE framing. Bytes may split anywhere, including inside UTF-8
//! or CRLF. Decode only complete events; an unfinished stream is not success.

const MAX_EVENT_BYTES: usize = 256 * 1024;

#[derive(Debug, Default)]
pub(crate) struct Decoder {
    line: Vec<u8>,
    data: Vec<u8>,
    after_cr: bool,
}

impl Decoder {
    pub(crate) fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, &'static str> {
        let mut events = Vec::new();
        for &byte in bytes {
            if self.after_cr && byte == b'\n' {
                self.after_cr = false;
                continue;
            }
            self.after_cr = byte == b'\r';
            if byte == b'\r' || byte == b'\n' {
                self.end_line(&mut events)?;
            } else {
                self.line.push(byte);
            }
            if self.line.len() + self.data.len() > MAX_EVENT_BYTES {
                return Err("Model stream event exceeds the size limit.");
            }
        }
        Ok(events)
    }

    fn end_line(&mut self, events: &mut Vec<String>) -> Result<(), &'static str> {
        if self.line.is_empty() {
            if !self.data.is_empty() {
                self.data.pop(); // SSE joins data fields with a newline.
                events.push(
                    String::from_utf8(std::mem::take(&mut self.data))
                        .map_err(|_| "Model stream contains invalid UTF-8.")?,
                );
            }
        } else if let Some(data) = self.line.strip_prefix(b"data:") {
            self.data
                .extend_from_slice(data.strip_prefix(b" ").unwrap_or(data));
            self.data.push(b'\n');
        }
        self.line.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn arbitrary_network_boundaries_preserve_events(
            sizes in prop::collection::vec(1_usize..32, 1..40),
            text in "[a-zéλ😀]{1,80}",
            newline in prop::sample::select(vec!["\n", "\r\n", "\r"]),
        ) {
            let wire = format!(": keepalive{newline}data: {text}{newline}data: second line{newline}{newline}data: [DONE]{newline}{newline}");
            let mut decoder = Decoder::default();
            let mut found = Vec::new();
            let mut offset = 0;
            for size in sizes.iter().cycle() {
                let end = (offset + size).min(wire.len());
                found.extend(decoder.push(&wire.as_bytes()[offset..end]).unwrap());
                offset = end;
                if offset == wire.len() { break; }
            }
            prop_assert_eq!(found, vec![format!("{text}\nsecond line"), "[DONE]".to_owned()]);
        }
    }

    #[test]
    fn oversized_and_invalid_events_are_errors() {
        assert!(
            Decoder::default()
                .push(&vec![b'x'; MAX_EVENT_BYTES + 1])
                .is_err()
        );
        assert!(Decoder::default().push(b"data: \xff\n\n").is_err());
    }
}
