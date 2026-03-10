use crate::http_utils::response::ProxyResponse;
use crate::Server;
use anyhow::Result;
use httparse::{EMPTY_HEADER, Response};
use std::sync::atomic::{AtomicU16, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{sleep, Duration};

use super::common::request::ProxyRequests;

static PORT_COUNTER: AtomicU16 = AtomicU16::new(9100);

struct TestServer {
    handle: tokio::task::JoinHandle<()>,
    addr: String,
}

impl TestServer {
    async fn start() -> Self {
        let port = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
        let addr = format!("127.0.0.1:{port}");
        let addr_clone = addr.clone();

        let handle = tokio::spawn(async move {
            Server::run_on_addr(Some(addr_clone)).await.ok();
        });

        sleep(Duration::from_millis(100)).await;

        TestServer { handle, addr }
    }

    fn addr(&self) -> &str {
        &self.addr
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

async fn read_response(socket: &mut TcpStream) -> Result<Vec<u8>> {
    let mut buff = vec![0u8; 1024];
    let size = socket.read(&mut buff).await?;
    buff.truncate(size);
    Ok(buff)
}

#[tokio::test]
async fn test_proxy_auth_required() -> Result<()> {
    let server = TestServer::start().await;
    let mut socket = TcpStream::connect(server.addr()).await?;

    socket
        .write_all(ProxyRequests::ConnectWithoutAuth.as_bytes())
        .await?;

    let response = read_response(&mut socket).await?;
    let expected = ProxyResponse::ProxyAuthRequired.as_bytes();

    assert_eq!(response, expected);
    Ok(())
}

#[tokio::test]
async fn test_unauthorized() -> Result<()> {
    let server = TestServer::start().await;
    let mut socket = TcpStream::connect(server.addr()).await?;

    socket
        .write_all(ProxyRequests::ConnectInvalidAuth.as_bytes())
        .await?;

    let response = read_response(&mut socket).await?;
    let expected = ProxyResponse::Unauthorized.as_bytes();

    assert_eq!(response, expected);
    Ok(())
}

#[tokio::test]
async fn test_method_not_allowed() -> Result<()> {
    let server = TestServer::start().await;
    let mut socket = TcpStream::connect(server.addr()).await?;

    socket.write_all(ProxyRequests::Get.as_bytes()).await?;

    let response = read_response(&mut socket).await?;
    let expected = ProxyResponse::MethodNotAllowed.as_bytes();

    assert_eq!(response, expected);
    Ok(())
}

#[tokio::test]
async fn test_successful_connect() -> Result<()> {
    let server = TestServer::start().await;
    let mut socket = TcpStream::connect(server.addr()).await?;

    socket.write_all(ProxyRequests::Connect.as_bytes()).await?;

    let response_bytes = read_response(&mut socket).await?;

    let mut headers = [EMPTY_HEADER; 16];
    let mut response = Response::new(&mut headers);
    response.parse(&response_bytes)?;
    assert_eq!(response.code.unwrap(), 200);

    Ok(())
}

#[tokio::test]
async fn test_malformed_request() -> Result<()> {
    let server = TestServer::start().await;
    let mut socket = TcpStream::connect(server.addr()).await?;

    socket
        .write_all(ProxyRequests::Malformed.as_bytes())
        .await?;

    let result = read_response(&mut socket).await;

    assert!(result.is_ok());

    Ok(())
}

#[tokio::test]
async fn test_server_cleanup() -> Result<()> {
    let addr: String;

    {
        let server = TestServer::start().await;
        addr = server.addr().to_string();
        let socket = TcpStream::connect(server.addr()).await;
        assert!(socket.is_ok());
    }

    sleep(Duration::from_millis(50)).await;

    let listener = tokio::net::TcpListener::bind(&addr).await;
    assert!(
        listener.is_ok(),
        "Port should be freed after server cleanup"
    );

    Ok(())
}

struct MockTargetServer {
    addr: String,
    handle: tokio::task::JoinHandle<()>,
}

impl MockTargetServer {
    async fn start_echo() -> Self {
        let port = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
        let addr = format!("127.0.0.1:{port}");
        let addr_clone = addr.clone();

        let handle = tokio::spawn(async move {
            let listener = TcpListener::bind(&addr_clone).await.unwrap();
            loop {
                if let Ok((mut socket, _)) = listener.accept().await {
                    tokio::spawn(async move {
                        let mut buf = [0u8; 1024];
                        loop {
                            match socket.read(&mut buf).await {
                                Ok(0) => break,
                                Ok(n) => {
                                    let _ = socket.write_all(&buf[..n]).await;
                                }
                                Err(_) => break,
                            }
                        }
                    });
                }
            }
        });

        sleep(Duration::from_millis(50)).await;
        MockTargetServer { addr, handle }
    }

    async fn start_sender(bytes_to_send: usize) -> Self {
        let port = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
        let addr = format!("127.0.0.1:{port}");
        let addr_clone = addr.clone();

        let handle = tokio::spawn(async move {
            let listener = TcpListener::bind(&addr_clone).await.unwrap();
            if let Ok((mut socket, _)) = listener.accept().await {
                let data = vec![0x41u8; bytes_to_send];
                let _ = socket.write_all(&data).await;
                let _ = socket.shutdown().await;
            }
        });

        sleep(Duration::from_millis(50)).await;
        MockTargetServer { addr, handle }
    }

    fn addr(&self) -> &str {
        &self.addr
    }
}

impl Drop for MockTargetServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

fn connect_request_to(target: &str, auth: &str) -> Vec<u8> {
    format!(
        "CONNECT {target} HTTP/1.1\r\n\
         Host: {target}\r\n\
         Proxy-Authorization: Basic {auth}\r\n\
         \r\n"
    )
    .into_bytes()
}

#[tokio::test]
async fn test_traffic_limit_exceeded() -> Result<()> {
    let server = TestServer::start().await;
    let target = MockTargetServer::start_sender(15_000).await;

    let auth = "YWRtaW46MTIzNDU=";
    let request = connect_request_to(target.addr(), auth);

    {
        let mut socket = TcpStream::connect(server.addr()).await?;
        socket.write_all(&request).await?;

        let mut response = vec![0u8; 4096];
        let n = socket.read(&mut response).await?;
        response.truncate(n);

        assert!(
            response.starts_with(b"HTTP/1.1 200"),
            "First connection should succeed"
        );

        let mut data = vec![0u8; 20_000];
        let _ = socket.read(&mut data).await;
    }

    sleep(Duration::from_millis(100)).await;

    let target2 = MockTargetServer::start_echo().await;
    let request2 = connect_request_to(target2.addr(), auth);

    {
        let mut socket = TcpStream::connect(server.addr()).await?;
        socket.write_all(&request2).await?;

        let response = read_response(&mut socket).await?;
        let expected = ProxyResponse::QuotaExceeded.as_bytes();

        assert_eq!(
            response, expected,
            "Second connection should be rejected with 403"
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_concurrency_limit_exceeded() -> Result<()> {
    let server = TestServer::start().await;

    let target1 = MockTargetServer::start_echo().await;
    let target2 = MockTargetServer::start_echo().await;
    let target3 = MockTargetServer::start_echo().await;

    let auth = "YWRtaW46MTIzNDU=";

    let mut socket1 = TcpStream::connect(server.addr()).await?;
    socket1
        .write_all(&connect_request_to(target1.addr(), auth))
        .await?;
    let mut resp1 = vec![0u8; 1024];
    let n = socket1.read(&mut resp1).await?;
    resp1.truncate(n);
    assert!(
        resp1.starts_with(b"HTTP/1.1 200"),
        "First connection should succeed"
    );

    let mut socket2 = TcpStream::connect(server.addr()).await?;
    socket2
        .write_all(&connect_request_to(target2.addr(), auth))
        .await?;
    let mut resp2 = vec![0u8; 1024];
    let n = socket2.read(&mut resp2).await?;
    resp2.truncate(n);
    assert!(
        resp2.starts_with(b"HTTP/1.1 200"),
        "Second connection should succeed"
    );

    let mut socket3 = TcpStream::connect(server.addr()).await?;
    socket3
        .write_all(&connect_request_to(target3.addr(), auth))
        .await?;
    let response = read_response(&mut socket3).await?;
    let expected = ProxyResponse::TooManyRequests.as_bytes();

    assert_eq!(
        response, expected,
        "Third connection should be rejected with 429"
    );

    Ok(())
}
