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

const KNOWN_RPCS: &[(&str, &str)] = &[
    ("mycqu_service.MycquFetcher", "FetchUser"),
    ("mycqu_service.MycquFetcher", "FetchEnrollCourseInfo"),
    ("mycqu_service.MycquFetcher", "FetchEnrollCourseItem"),
    ("mycqu_service.MycquFetcher", "FetchExam"),
    ("mycqu_service.MycquFetcher", "FetchAllSession"),
    ("mycqu_service.MycquFetcher", "FetchCurrSessionInfo"),
    ("mycqu_service.MycquFetcher", "FetchAllSessionInfo"),
    ("mycqu_service.MycquFetcher", "FetchCourseTimetable"),
    ("mycqu_service.MycquFetcher", "FetchEnrollTimetable"),
    ("mycqu_service.MycquFetcher", "FetchScore"),
    ("mycqu_service.MycquFetcher", "FetchGpaRanking"),
    ("mycqu_service.CardFetcher", "FetchCard"),
    ("mycqu_service.CardFetcher", "FetchBills"),
    ("mycqu_service.CardFetcher", "FetchEnergyFee"),
    ("mycqu_service.LibraryFetcher", "FetchBorrowBook"),
    ("mycqu_service.LibraryFetcher", "RenewBook"),
];

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
        let (service, method) = label_rpc_path(&path);
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

fn label_rpc_path(path: &str) -> (&'static str, &'static str) {
    let Some((service, method)) = path.trim_matches('/').rsplit_once('/') else {
        return ("unknown", "unknown");
    };

    KNOWN_RPCS
        .iter()
        .copied()
        .find(|known| *known == (service, method))
        .unwrap_or(("unknown", "unknown"))
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

#[cfg(test)]
mod tests {
    use std::{
        convert::Infallible,
        future::{Ready, ready},
    };

    use super::*;

    #[derive(Clone)]
    struct FixedResponseService {
        http_status: http::StatusCode,
        grpc_status: Option<&'static str>,
    }

    impl Service<http::Request<Body>> for FixedResponseService {
        type Response = http::Response<()>;
        type Error = Infallible;
        type Future = Ready<Result<Self::Response, Self::Error>>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, _request: http::Request<Body>) -> Self::Future {
            let mut builder = http::Response::builder().status(self.http_status);
            if let Some(status) = self.grpc_status {
                builder = builder.header("grpc-status", status);
            }
            ready(Ok(builder.body(()).expect("build test response")))
        }
    }

    #[test]
    fn labels_known_rpc_paths() {
        assert_eq!(
            label_rpc_path("/mycqu_service.MycquFetcher/FetchScore"),
            ("mycqu_service.MycquFetcher", "FetchScore")
        );
        assert_eq!(
            label_rpc_path("/mycqu_service.CardFetcher/FetchBills"),
            ("mycqu_service.CardFetcher", "FetchBills")
        );
        assert_eq!(
            label_rpc_path("/mycqu_service.LibraryFetcher/RenewBook"),
            ("mycqu_service.LibraryFetcher", "RenewBook")
        );
    }

    #[test]
    fn buckets_unknown_rpc_paths_to_fixed_labels() {
        assert_eq!(
            label_rpc_path("/random.Service/Method"),
            ("unknown", "unknown")
        );
        assert_eq!(
            label_rpc_path("/mycqu_service.MycquFetcher/NewMethod"),
            ("unknown", "unknown")
        );
        assert_eq!(label_rpc_path("malformed"), ("unknown", "unknown"));
    }

    #[test]
    fn extracts_grpc_status_from_response_headers() {
        let response = http::Response::builder()
            .status(http::StatusCode::OK)
            .header("grpc-status", "7")
            .body(())
            .expect("build test response");

        assert_eq!(grpc_status(&response), Some("7".to_string()));
    }

    #[test]
    fn falls_back_to_http_status_when_grpc_status_is_absent() {
        let ok_response = http::Response::builder()
            .status(http::StatusCode::OK)
            .body(())
            .expect("build test response");
        let unavailable_response = http::Response::builder()
            .status(http::StatusCode::SERVICE_UNAVAILABLE)
            .body(())
            .expect("build test response");

        assert_eq!(grpc_status(&ok_response), Some("0".to_string()));
        assert_eq!(grpc_status(&unavailable_response), Some("503".to_string()));
    }

    #[test]
    fn parses_http_request_path_for_metrics_endpoint() {
        let request = b"GET /metrics HTTP/1.1\r\nHost: localhost\r\n\r\n";

        assert_eq!(request_path(request), Some("/metrics".to_string()));
    }

    #[tokio::test]
    async fn metrics_layer_records_known_rpc_request() {
        let service = FixedResponseService {
            http_status: http::StatusCode::OK,
            grpc_status: Some("0"),
        };
        let mut service = MetricsLayer.layer(service);
        let labels = &["mycqu_service.MycquFetcher", "FetchScore", "0"];
        let before = GRPC_SERVER_REQUESTS.with_label_values(labels).get();
        let request = http::Request::builder()
            .uri("/mycqu_service.MycquFetcher/FetchScore")
            .body(Body::empty())
            .expect("build test request");

        let response = service.call(request).await.expect("service call succeeds");

        assert_eq!(response.status(), http::StatusCode::OK);
        assert_eq!(
            GRPC_SERVER_REQUESTS.with_label_values(labels).get(),
            before + 1
        );
    }

    #[tokio::test]
    async fn metrics_layer_buckets_unknown_rpc_request() {
        let service = FixedResponseService {
            http_status: http::StatusCode::NOT_FOUND,
            grpc_status: None,
        };
        let mut service = MetricsLayer.layer(service);
        let labels = &["unknown", "unknown", "404"];
        let before = GRPC_SERVER_REQUESTS.with_label_values(labels).get();
        let request = http::Request::builder()
            .uri("/not.AService/Nope")
            .body(Body::empty())
            .expect("build test request");

        let response = service.call(request).await.expect("service call succeeds");

        assert_eq!(response.status(), http::StatusCode::NOT_FOUND);
        assert_eq!(
            GRPC_SERVER_REQUESTS.with_label_values(labels).get(),
            before + 1
        );
    }
}
