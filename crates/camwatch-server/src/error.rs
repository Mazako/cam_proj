use std::{fmt, net::SocketAddr};

#[derive(Debug, PartialEq, Eq)]
pub struct NonLoopbackBindAddress(pub SocketAddr);

impl fmt::Display for NonLoopbackBindAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "server bind address must be 127.0.0.1 or ::1, got {}",
            self.0
        )
    }
}

impl std::error::Error for NonLoopbackBindAddress {}
