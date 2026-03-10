pub enum ProxyRequests {
    Connect,
    ConnectWithoutAuth,
    ConnectInvalidAuth,
    Get,
    Malformed,
}

impl ProxyRequests {
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            ProxyRequests::Connect => {
                b"CONNECT ident.me:443 HTTP/1.1\r\n\
                  Host: ident.me:443\r\n\
                  Proxy-Authorization: Basic cHJvY2VudDpvOTUzelk3bG5rWU1FbDVE\r\n\
                  \r\n"
            }
            ProxyRequests::ConnectWithoutAuth => {
                b"CONNECT example.com:443 HTTP/1.1\r\n\
                  Host: example.com:443\r\n\
                  \r\n"
            }
            ProxyRequests::ConnectInvalidAuth => {
                b"CONNECT example.com:443 HTTP/1.1\r\n\
                  Host: example.com:443\r\n\
                  Proxy-Authorization: Basic aW52YWxpZDppbnZhbGlk\r\n\
                  \r\n"
            }
            ProxyRequests::Get => {
                b"GET / HTTP/1.1\r\n\
                  Host: example.com\r\n\
                  \r\n"
            }
            ProxyRequests::Malformed => b"INVALID REQUEST\r\n",
        }
    }
}
