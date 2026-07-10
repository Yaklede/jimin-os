use std::{net::SocketAddr, time::Duration};

use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
    time::timeout,
};

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeTarget {
    Live,
    Ready,
}

impl ProbeTarget {
    #[must_use]
    pub const fn path(self) -> &'static str {
        match self {
            Self::Live => "/health/live",
            Self::Ready => "/health/ready",
        }
    }
}

#[derive(Debug, Error)]
#[error("health probe failed")]
pub struct ProbeError;

/// Calls the selected local health endpoint and requires an HTTP 200 response.
///
/// # Errors
///
/// Returns [`ProbeError`] on timeout, connection failure, malformed response,
/// or any non-200 status.
pub async fn run_probe(target: ProbeTarget, address: SocketAddr) -> Result<(), ProbeError> {
    timeout(PROBE_TIMEOUT, probe(target, address))
        .await
        .map_err(|_| ProbeError)??;
    Ok(())
}

async fn probe(target: ProbeTarget, address: SocketAddr) -> Result<(), ProbeError> {
    let mut stream = TcpStream::connect(address).await.map_err(|_| ProbeError)?;
    let request = format!(
        "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        target.path()
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|_| ProbeError)?;

    let mut status_line = String::new();
    BufReader::new(stream)
        .read_line(&mut status_line)
        .await
        .map_err(|_| ProbeError)?;

    if status_line.len() > 128 || status_line.split_ascii_whitespace().nth(1) != Some("200") {
        return Err(ProbeError);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use tokio::{io::AsyncReadExt, net::TcpListener};

    use super::*;

    #[tokio::test]
    async fn ready_probe_requires_http_200() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("address should be available");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("probe should connect");
            let mut request = [0_u8; 256];
            let count = stream
                .read(&mut request)
                .await
                .expect("request should arrive");
            assert!(String::from_utf8_lossy(&request[..count]).starts_with("GET /health/ready"));
            stream
                .write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\n\r\n")
                .await
                .expect("response should be written");
        });

        assert!(run_probe(ProbeTarget::Ready, address).await.is_err());
        server.await.expect("server task should finish");
    }
}
