use crate::auth::parse_proxy_auth_token;
use crate::context::Context;
use crate::http_utils::response::ProxyResponse;
use crate::registry::{LimitError, Limits};
use crate::tunnel::connect_target;
use anyhow::{bail, Result};
use httparse::{Request, EMPTY_HEADER};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, error, warn};

#[allow(clippy::too_many_lines)]
pub async fn handle_connection(mut source: TcpStream, ctx: Context) -> Result<()> {
    let mut buff = [0u8; 1024];

    let size = match source.read(&mut buff).await {
        Ok(0) => return Ok(()),
        Ok(n) => n,
        Err(e) => {
            error!(error = format!("{}", e));
            bail!(e);
        }
    };

    let mut headers = [EMPTY_HEADER; 16];
    let mut request = Request::new(&mut headers);
    let status = request.parse(&buff[..size])?;
    if status.is_partial() {
        source
            .write_all(ProxyResponse::BadRequest.as_bytes())
            .await?;
        return Ok(());
    }

    debug!(request = format!("{:?}", request));
    let Some(request_method) = request.method else {
        bail!("Missing request method after complete parse");
    };
    let Some(request_path) = request.path else {
        bail!("Missing request path after complete parse");
    };

    if request_method != "CONNECT" {
        source
            .write_all(ProxyResponse::MethodNotAllowed.as_bytes())
            .await?;
        return Ok(());
    }
    let auth_header = request
        .headers
        .iter()
        .find(|header| header.name == "Proxy-Authorization");

    match auth_header {
        None => {
            source
                .write_all(ProxyResponse::ProxyAuthRequired.as_bytes())
                .await?;
        }
        Some(proxy_auth_header) => {
            let (user, password) = parse_proxy_auth_token(proxy_auth_header.value)?;

            let db_user = ctx.backend.fetch_user(&user).await?;
            if db_user.is_none() {
                source
                    .write_all(ProxyResponse::Unauthorized.as_bytes())
                    .await?;
                return Ok(());
            }
            let db_user = db_user.unwrap();
            if !db_user.is_authenticated(&password) {
                source
                    .write_all(ProxyResponse::Unauthorized.as_bytes())
                    .await?;
                return Ok(());
            }

            let mut registry = ctx.registry.lock().await;
            registry.create_user(&user, Limits::from(db_user));
            registry.inc_concurrency(&user);

            match registry.check_limits(&user) {
                Ok(()) => {
                    drop(registry);

                    let result: Result<(u64, u64)> = async {
                        let mut target = TcpStream::connect(request_path).await?;
                        connect_target(
                            &mut source,
                            &mut target,
                            Duration::from_secs(ctx.config.connection_timeout),
                        )
                        .await
                    }
                    .await;

                    let mut registry = ctx.registry.lock().await;
                    match result {
                        Ok((ingress, egress)) => {
                            registry.add_ingress_traffic(&user, u128::from(ingress));
                            registry.add_egress_traffic(&user, u128::from(egress));
                        }
                        Err(e) => {
                            warn!(error = %e, "Connection failed");
                        }
                    }
                    registry.dec_concurrency(&user);
                }
                Err(err) => {
                    registry.dec_concurrency(&user);
                    drop(registry);

                    warn!(message = format!("{:?}", err));
                    match err {
                        LimitError::ConcurrencyLimitExceed(_) => {
                            source
                                .write_all(ProxyResponse::TooManyRequests.as_bytes())
                                .await?;
                        }
                        LimitError::TrafficLimitExceed(_) => {
                            source
                                .write_all(ProxyResponse::QuotaExceeded.as_bytes())
                                .await?;
                        }
                        LimitError::UserNotFound => {
                            bail!("check_limits called for unknown user");
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
