use bytes::{Buf, BufMut, BytesMut};
use camino::Utf8Path;

use crate::runtime::{Request, canonicalize};
use crate::{EncounterId, Error, HostHandle};

/// ACT encounter interchange encoding. Both contain the same XML representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActFormat {
    Xml,
    /// A single gzip member, as used by ACT's `.act` files.
    Gzip,
}

fn packet(
    session: u64,
    command: u32,
    text: &str,
    id: u64,
    compressed: bool,
) -> Result<BytesMut, Error> {
    if text.contains('\0') || text.len() > 65536 - 64 || command == 4 && text.len() > 4096 {
        return Err(Error::InvalidArchiveRequest(
            "text contains NUL or exceeds its bound".into(),
        ));
    }
    let mut bytes = BytesMut::with_capacity(64 + text.len());
    bytes.put_u32_le((64 + text.len()) as u32);
    bytes.put_u32_le(1);
    bytes.put_u64_le(session);
    bytes.put_u32_le(command);
    bytes.put_u32_le(u32::from(compressed));
    bytes.put_u64_le(id);
    bytes.put_u32_le(text.len() as u32);
    bytes.resize(64, 0);
    bytes.extend_from_slice(text.as_bytes());
    Ok(bytes)
}

impl HostHandle {
    async fn archive_command(
        &self,
        command: u32,
        text: &str,
        id: u64,
        compressed: bool,
    ) -> Result<u32, Error> {
        let mut bytes = packet(self.session, command, text, id, compressed)?;
        self.request(Request::Archive, &mut bytes).await?;
        let mut result = bytes
            .get(48..56)
            .ok_or_else(|| Error::InvalidArchiveResponse("missing count".into()))?;
        u32::try_from(result.get_u64_le())
            .map_err(|error| Error::InvalidArchiveResponse(error.to_string()))
    }

    /// Reads ACT encounter XML or gzip `.act` into retained encounters and returns the added count.
    ///
    /// Initialize the parser's category registrations first and end live combat. This reconstructs
    /// saved swings directly; raw parser hooks, correction rules and live audio are not replayed.
    /// A malformed file adds no encounters. Each file and its expanded XML are limited to 64 MiB.
    pub async fn import_act(&self, path: impl AsRef<Utf8Path>) -> Result<u32, Error> {
        let path = canonicalize(path.as_ref())?;
        self.archive_command(1, path.as_str(), 0, false).await
    }

    /// Exports one retained encounter to a new file; refuses to overwrite an existing destination.
    ///
    /// End live combat first. ACT interchange uses second-resolution timestamps and display damage;
    /// use [`Self::save_history`] for the private database's precise numeric and timestamp values.
    pub async fn export_act(
        &self,
        id: EncounterId,
        path: impl AsRef<Utf8Path>,
        format: ActFormat,
    ) -> Result<(), Error> {
        if id.session != self.session || id.value == 0 {
            return Err(Error::InvalidArchiveRequest(
                "encounter belongs to another session".into(),
            ));
        }
        let path = path.as_ref();
        let name = path
            .file_name()
            .ok_or_else(|| Error::InvalidArchiveRequest("destination needs a file name".into()))?;
        let parent = path
            .parent()
            .filter(|parent| !parent.as_str().is_empty())
            .unwrap_or(Utf8Path::new("."));
        let destination = canonicalize(parent)?.join(name);
        self.archive_command(2, destination.as_str(), id.value, format == ActFormat::Gzip)
            .await?;
        Ok(())
    }

    /// Checkpoints all retained completed encounters to the private history database.
    ///
    /// Returns the total stored count. Call while live combat is idle. Payloads are immutable;
    /// the index is replaced atomically after every payload is flushed. Shutdown does not implicitly
    /// checkpoint, so crash/restart recovery reaches the last successful call.
    pub async fn save_history(&self) -> Result<u32, Error> {
        self.archive_command(3, "", 0, false).await
    }

    /// Restores saved encounters matching title, zone or character, ignoring case.
    ///
    /// An empty filter selects all. Returns the newly restored count; repeated restores skip
    /// already restored archives. Initialize the same parser categories before reconstruction,
    /// then inspect results with [`Self::encounters`] and [`Self::encounter_detail`]. IDs are new
    /// session IDs. A corrupt selected archive publishes no encounters from that call.
    pub async fn restore_history(&self, filter: &str) -> Result<u32, Error> {
        self.archive_command(4, filter, 0, false).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_packets_preserve_fields_and_reject_unbounded_text() {
        let request = packet(3, 2, "/로그.act", 7, true).expect("encode archive command");
        assert_eq!(request.get(16..20), Some(2_u32.to_le_bytes().as_slice()));
        assert_eq!(request.get(24..32), Some(7_u64.to_le_bytes().as_slice()));
        assert_eq!(request.get(64..), Some("/로그.act".as_bytes()));
        for (command, text) in [
            (1, "x\0y".to_owned()),
            (1, "x".repeat(65473)),
            (4, "x".repeat(4097)),
        ] {
            assert!(matches!(
                packet(1, command, &text, 0, false),
                Err(Error::InvalidArchiveRequest(_))
            ));
        }
    }
}
