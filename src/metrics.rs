use std::{
    future::Future,
    net::SocketAddr,
    pin::Pin,
    sync::LazyLock,
    task::{Context, Poll},
    time::Instant,
};

use prometheus::{
    Encoder, HistogramVec, IntCounterVec, TextEncoder, register_histogram_vec,
    register_int_counter_vec,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use tonic::{body::Body, codegen::http};
use tower::{Layer, Service};

static GRPC_SERVER_DURATION: LazyLock<HistogramVec> = LazyLock::new(|| {
    register_histogram_vec!(
        "grpc_server_duration_seconds",
        "gRPC server request duration in seconds.",
        &["service", "method", "status_code"]
    )
    .expect("register grpc server duration metric")
});

static GRPC_SERVER_REQUESTS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "grpc_server_requests_total",
        "Total gRPC server requests.",
        &["service", "method", "status_code"]
    )
    .expect("register grpc server request metric")
});

#[derive(Clone)]
pub struct MetricsLayer;

#[derive(Clone)]
pub struct MetricsService<S> {
    inner: S,
}

impl<S> Layer<S> for MetricsLayer {
    type Service = MetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MetricsService { inner }
    }
}

impl<S, ResponseBody> Service<http::Request<Body>> for MetricsService<S>
where
    S: Service<http::Request<Body>, Response = http::Response<ResponseBody>> + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = http::Response<ResponseBody>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let path = request.uri().path().to_string();
        let (service, method) = split_rpc_path(&path);
        let started_at = Instant::now();
        let future = self.inner.call(request);

        Box::pin(async move {
            let response = future.await;
            let status_code = response
                .as_ref()
                .ok()
                .and_then(grpc_status)
                .unwrap_or_else(|| "unknown".to_string());

            GRPC_SERVER_DURATION
                .with_label_values(&[&service, &method, &status_code])
                .observe(started_at.elapsed().as_secs_f64());
            GRPC_SERVER_REQUESTS
                .with_label_values(&[&service, &method, &status_code])
                .inc();

            response
        })
    }
}

fn split_rpc_path(path: &str) -> (String, String) {
    path.trim_matches('/')
        .rsplit_once('/')
        .map(|(service, method)| (service.to_string(), method.to_string()))
        .unwrap_or_else(|| ("unknown".to_string(), path.trim_matches('/').to_string()))
}

fn grpc_status<ResponseBody>(response: &http::Response<ResponseBody>) -> Option<String> {
    response
        .headers()
        .get("grpc-status")
        .and_then(|status| status.to_str().ok())
        .map(ToString::to_string)
        .or_else(|| {
            if response.status().is_success() {
                Some("0".to_string())
            } else {
                Some(response.status().as_u16().to_string())
            }
        })
}

pub async fn serve_metrics(addr: SocketAddr) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("metrics server listening on {}", addr);

    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let mut buffer = [0_u8; 1024];
            let path = match socket.read(&mut buffer).await {
                Ok(0) | Err(_) => return,
                Ok(size) => request_path(&buffer[..size]),
            };

            let (status, content_type, body) = if path.as_deref() == Some("/metrics") {
                let encoder = TextEncoder::new();
                let metric_families = prometheus::gather();
                let mut body = Vec::new();
                if encoder.encode(&metric_families, &mut body).is_err() {
                    (
                        500,
                        "text/plain; charset=utf-8".to_string(),
                        b"encode error".to_vec(),
                    )
                } else {
                    (200, encoder.format_type().to_string(), body)
                }
            } else {
                (
                    404,
                    "text/plain; charset=utf-8".to_string(),
                    b"Not Found".to_vec(),
                )
            };

            let response = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                status,
                reason_phrase(status),
                content_type,
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.write_all(&body).await;
            let _ = socket.shutdown().await;
        });
    }
}

fn request_path(buffer: &[u8]) -> Option<String> {
    let line = std::str::from_utf8(buffer).ok()?.lines().next()?;
    let mut parts = line.split_whitespace();
    let _method = parts.next()?;
    parts.next().map(ToString::to_string)
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Internal Server Error",
    }
}
