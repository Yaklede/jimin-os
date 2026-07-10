use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::{Error, Result};

pub const DEFAULT_MAX_LINE_BYTES: usize = 1024 * 1024;

pub(crate) struct JsonLineTransport<R, W> {
    reader: R,
    writer: W,
    max_line_bytes: usize,
}

impl<R, W> JsonLineTransport<R, W>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    pub(crate) fn new(reader: R, writer: W, max_line_bytes: usize) -> Self {
        Self {
            reader,
            writer,
            max_line_bytes,
        }
    }

    pub(crate) async fn read_value(&mut self) -> Result<Value> {
        let mut line = Vec::with_capacity(4096.min(self.max_line_bytes));

        loop {
            let available = self.reader.fill_buf().await.map_err(|source| Error::Io {
                operation: "read",
                source,
            })?;

            if available.is_empty() {
                return Err(Error::UnexpectedEof);
            }

            if let Some(newline_index) = available.iter().position(|byte| *byte == b'\n') {
                if line.len() + newline_index > self.max_line_bytes {
                    return Err(Error::LineTooLong {
                        max_bytes: self.max_line_bytes,
                    });
                }
                line.extend_from_slice(&available[..newline_index]);
                self.reader.consume(newline_index + 1);
                break;
            }

            if line.len() + available.len() > self.max_line_bytes {
                return Err(Error::LineTooLong {
                    max_bytes: self.max_line_bytes,
                });
            }

            let available_len = available.len();
            line.extend_from_slice(available);
            self.reader.consume(available_len);
        }

        if line.last() == Some(&b'\r') {
            line.pop();
        }

        let text = std::str::from_utf8(&line).map_err(|_| Error::InvalidUtf8)?;
        serde_json::from_str(text).map_err(|_| Error::MalformedJson)
    }

    pub(crate) async fn write<T>(&mut self, message: &T) -> Result<()>
    where
        T: Serialize,
    {
        let encoded = serde_json::to_vec(message).map_err(|_| Error::InvalidProtocolMessage)?;
        if encoded.len() > self.max_line_bytes {
            return Err(Error::LineTooLong {
                max_bytes: self.max_line_bytes,
            });
        }

        self.writer
            .write_all(&encoded)
            .await
            .map_err(|source| Error::Io {
                operation: "write",
                source,
            })?;
        self.writer
            .write_all(b"\n")
            .await
            .map_err(|source| Error::Io {
                operation: "write",
                source,
            })?;
        self.writer.flush().await.map_err(|source| Error::Io {
            operation: "flush",
            source,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::io::{AsyncWriteExt, BufReader, split};

    use super::{DEFAULT_MAX_LINE_BYTES, JsonLineTransport};
    use crate::error::Error;

    #[tokio::test]
    async fn reads_fragmented_and_adjacent_frames() {
        let (client, mut peer) = tokio::io::duplex(256);
        let (reader, writer) = split(client);
        let mut transport =
            JsonLineTransport::new(BufReader::new(reader), writer, DEFAULT_MAX_LINE_BYTES);

        let peer_task = tokio::spawn(async move {
            peer.write_all(b"{\"id\":").await.expect("first fragment");
            peer.write_all(b"1}\n{\"method\":\"ready\"}\n")
                .await
                .expect("remaining frames");
        });

        assert_eq!(
            transport.read_value().await.expect("first frame"),
            json!({"id": 1})
        );
        assert_eq!(
            transport.read_value().await.expect("second frame"),
            json!({"method": "ready"})
        );
        peer_task.await.expect("peer task");
    }

    #[tokio::test]
    async fn rejects_oversized_frames_before_unbounded_growth() {
        let (client, mut peer) = tokio::io::duplex(256);
        let (reader, writer) = split(client);
        let mut transport = JsonLineTransport::new(BufReader::new(reader), writer, 8);

        let peer_task = tokio::spawn(async move {
            peer.write_all(b"123456789\n")
                .await
                .expect("oversized frame");
        });

        assert!(matches!(
            transport.read_value().await,
            Err(Error::LineTooLong { max_bytes: 8 })
        ));
        peer_task.await.expect("peer task");
    }

    #[tokio::test]
    async fn distinguishes_invalid_utf8_malformed_json_and_eof() {
        async fn read(bytes: &[u8]) -> Error {
            let capacity = bytes.len().max(1) + 16;
            let (client, mut peer) = tokio::io::duplex(capacity);
            let (reader, writer) = split(client);
            let mut transport = JsonLineTransport::new(BufReader::new(reader), writer, 64);
            let owned = bytes.to_vec();
            let peer_task = tokio::spawn(async move {
                peer.write_all(&owned).await.expect("fixture write");
            });
            let error = transport
                .read_value()
                .await
                .expect_err("fixture should fail");
            peer_task.await.expect("peer task");
            error
        }

        assert!(matches!(read(&[0xff, b'\n']).await, Error::InvalidUtf8));
        assert!(matches!(read(b"not-json\n").await, Error::MalformedJson));
        assert!(matches!(read(b"").await, Error::UnexpectedEof));
    }
}
