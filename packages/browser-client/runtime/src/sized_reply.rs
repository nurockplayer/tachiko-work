use super::{DesignerError, DesignerWireReply, SessionProfile, wire_reply_byte_limit};

// Count and encode in one pass. Once over budget, count the rest without
// retaining more bytes so refusals keep their exact actual/maximum values.
struct SizedReplyWriter {
    bytes: Vec<u8>,
    actual: usize,
    maximum: usize,
}

impl std::io::Write for SizedReplyWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.actual = self.actual.saturating_add(bytes.len());
        if self.actual <= self.maximum {
            let capacity = self.bytes.capacity();
            if self.actual > capacity && capacity > self.maximum / 2 {
                // Growing a large preview by doubling would allocate past
                // the wire limit and retain that excess in the WASM arena.
                self.bytes.reserve_exact(self.maximum - self.bytes.len());
            }
            self.bytes.extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) fn encode_sized_reply(
    reply: &DesignerWireReply,
    profile: SessionProfile,
    mut bytes: Vec<u8>,
) -> Result<Vec<u8>, DesignerError> {
    bytes.clear();
    let maximum = wire_reply_byte_limit(reply, profile);
    let mut writer = SizedReplyWriter {
        bytes,
        actual: 0,
        maximum,
    };
    serde_json::to_writer(&mut writer, reply).map_err(|error| {
        DesignerError::UnsupportedProject {
            message: format!("a Designer projection could not be encoded: {error}"),
        }
    })?;
    tachiko_workspace_engine::instrumentation::record_json_bytes(
        u64::try_from(writer.actual).unwrap_or(u64::MAX),
    );
    if writer.actual > maximum {
        return Err(DesignerError::ProjectionTooLarge {
            actual: writer.actual,
            maximum,
        });
    }
    Ok(writer.bytes)
}
