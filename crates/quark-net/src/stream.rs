use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::TlsAcceptor;

use crate::tls::ClientTls;

pub trait Io: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

pub type BoxedIo = Box<dyn Io>;

pub struct Connected {
    pub io: BoxedIo,
    pub local: Option<SocketAddr>,
    pub remote: Option<SocketAddr>,
}

pub async fn connect(
    host: &str,
    port: u16,
    tls: Option<ClientTls>,
    timeout: Duration,
) -> Result<Connected, String> {
    let stream = tokio::time::timeout(timeout, TcpStream::connect((host, port)))
        .await
        .map_err(|_| format!("connecting to {host}:{port} timed out"))?
        .map_err(|error| format!("cannot connect to {host}:{port}: {error}"))?;
    let _ = stream.set_nodelay(true);
    let local = stream.local_addr().ok();
    let remote = stream.peer_addr().ok();

    let Some(tls) = tls else {
        return Ok(Connected {
            io: Box::new(stream),
            local,
            remote,
        });
    };

    let stream = tokio::time::timeout(timeout, tls.connector.connect(tls.server_name, stream))
        .await
        .map_err(|_| format!("the TLS handshake with {host}:{port} timed out"))?
        .map_err(|error| format!("the TLS handshake with {host}:{port} failed: {error}"))?;
    Ok(Connected {
        io: Box::new(stream),
        local,
        remote,
    })
}

pub async fn accept(
    stream: TcpStream,
    tls: Option<TlsAcceptor>,
    timeout: Duration,
) -> Result<BoxedIo, String> {
    let _ = stream.set_nodelay(true);
    match tls {
        None => Ok(Box::new(stream)),
        Some(acceptor) => tokio::time::timeout(timeout, acceptor.accept(stream))
            .await
            .map_err(|_| "the TLS handshake timed out".to_owned())?
            .map(|stream| Box::new(stream) as BoxedIo)
            .map_err(|error| format!("the TLS handshake failed: {error}")),
    }
}
