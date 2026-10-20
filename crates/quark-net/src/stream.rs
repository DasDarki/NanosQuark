use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::rustls::pki_types::ServerName;

use crate::tls;

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
    tls: Option<&str>,
    timeout: Duration,
) -> Result<Connected, String> {
    let stream = tokio::time::timeout(timeout, TcpStream::connect((host, port)))
        .await
        .map_err(|_| format!("connecting to {host}:{port} timed out"))?
        .map_err(|error| format!("cannot connect to {host}:{port}: {error}"))?;
    let _ = stream.set_nodelay(true);
    let local = stream.local_addr().ok();
    let remote = stream.peer_addr().ok();

    let Some(server_name) = tls else {
        return Ok(Connected {
            io: Box::new(stream),
            local,
            remote,
        });
    };

    let name = ServerName::try_from(server_name.to_owned())
        .map_err(|_| format!("'{server_name}' is not a valid TLS server name"))?;
    let connector = tls::connector()?;
    let stream = tokio::time::timeout(timeout, connector.connect(name, stream))
        .await
        .map_err(|_| format!("the TLS handshake with {host}:{port} timed out"))?
        .map_err(|error| format!("the TLS handshake with {host}:{port} failed: {error}"))?;
    Ok(Connected {
        io: Box::new(stream),
        local,
        remote,
    })
}
