pub enum ProxyResponse {
    ConnectionEstablished,
    Unauthorized,
    ProxyAuthRequired,
    MethodNotAllowed,
    TooManyRequests,
    QuotaExceeded,
    BadRequest,
}

impl ProxyResponse {
    pub const fn as_bytes(&self) -> &'static [u8] {
        match self {
            Self::ConnectionEstablished => b"HTTP/1.1 200 Connection Established\r\n\r\n",
            Self::Unauthorized => b"HTTP/1.1 401 Unauthorized\r\n\r\n",
            Self::ProxyAuthRequired => {
                b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n"
            }
            Self::MethodNotAllowed => b"HTTP/1.1 405 Method Not Allowed\r\n\r\n",
            Self::TooManyRequests =>  b"HTTP/1.1 429 Too Many Requests\r\n\r\n",
            Self::QuotaExceeded =>  b"HTTP/1.1 403 Forbidden\r\n\r\n",
            Self::BadRequest => b"HTTP/1.1 400 Bad Request\r\n\r\n",
        }
    }
}
