use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::TlsAcceptor;

use crate::tls::{ClientTls, PeerCertificate, peer_certificate};

pub trait Io: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

pub type BoxedIo = Box<dyn Io>;

pub struct Connected {
    pub io: BoxedIo,
    pub local: Option<SocketAddr>,
    pub remote: Option<SocketAddr>,
    pub alpn: Option<Vec<u8>>,
}

pub struct Accepted {
    pub io: BoxedIo,
    pub peer: Option<PeerCertificate>,
}

pub async fn connect(
    host: &str,
    port: u16,
    tls: Option<ClientTls>,
    timeout: Duration,
) -> Result<Connected, String> {
    let addresses: Vec<SocketAddr> = if quark_core::has_connect_rules() {
        let resolved: Vec<SocketAddr> =
            tokio::time::timeout(timeout, tokio::net::lookup_host((host, port)))
                .await
                .map_err(|_| format!("resolving {host} timed out"))?
                .map_err(|error| format!("cannot resolve {host}: {error}"))?
                .collect();
        let ips: Vec<std::net::IpAddr> = resolved.iter().map(SocketAddr::ip).collect();
        quark_core::check_connect(host, port, &ips)?;
        resolved
    } else {
        Vec::new()
    };
    let connecting = async {
        if addresses.is_empty() {
            TcpStream::connect((host, port)).await
        } else {
            TcpStream::connect(addresses.as_slice()).await
        }
    };
    let stream = tokio::time::timeout(timeout, connecting)
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
            alpn: None,
        });
    };

    let stream = tokio::time::timeout(timeout, tls.connector.connect(tls.server_name, stream))
        .await
        .map_err(|_| format!("the TLS handshake with {host}:{port} timed out"))?
        .map_err(|error| format!("the TLS handshake with {host}:{port} failed: {error}"))?;
    let alpn = stream.get_ref().1.alpn_protocol().map(<[u8]>::to_vec);
    Ok(Connected {
        io: Box::new(stream),
        local,
        remote,
        alpn,
    })
}

pub async fn accept(
    stream: TcpStream,
    tls: Option<TlsAcceptor>,
    timeout: Duration,
) -> Result<Accepted, String> {
    let _ = stream.set_nodelay(true);
    let Some(acceptor) = tls else {
        return Ok(Accepted {
            io: Box::new(stream),
            peer: None,
        });
    };
    let stream = tokio::time::timeout(timeout, acceptor.accept(stream))
        .await
        .map_err(|_| "the TLS handshake timed out".to_owned())?
        .map_err(|error| format!("the TLS handshake failed: {error}"))?;
    let peer = peer_certificate(stream.get_ref().1);
    Ok(Accepted {
        io: Box::new(stream),
        peer,
    })
}
