use std::{
    net::SocketAddr,
    ops::Deref,
    sync::{Arc, LazyLock},
    time::Duration,
};

use rsmycqu::{
    errors::{ApiError, RsMyCQUError},
    session::Session,
    sso::LoginResult,
};
use tokio::sync::RwLock;
use tonic::{Status, transport::Server};
use tower_http::trace::TraceLayer;
use tracing::{Level, error, instrument};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    EnvFilter, Layer, filter, fmt, layer::SubscriberExt, util::SubscriberInitExt,
};

use crate::{
    card_service::CardService,
    library_service::LibraryService,
    mycqu_service::MycquServicer,
    utils::{CachedCoalescer, PROXIED_CLIENT_PROVIDER, PROXY_CLIENT_GET_ERROR},
};

mod metrics;

mod mycqu_service;

mod card_service;

mod library_service;

pub(crate) mod utils;

mod proto {
    use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime, ParseResult, TimeZone};

    tonic::include_proto!("mycqu_service");

    pub fn parser_date_time_str(date_time_str: &str) -> ParseResult<DateTime<FixedOffset>> {
        static CN_FIXED_OFFSET: FixedOffset = FixedOffset::east_opt(8 * 3600).unwrap();

        NaiveDateTime::parse_from_str(date_time_str, "%Y-%m-%d %H:%M:%S")
            .or_else(|_| {
                NaiveDate::parse_from_str(date_time_str, "%Y-%m-%d")
                    .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
            })
            .map(|ndt| CN_FIXED_OFFSET.from_local_datetime(&ndt).single().unwrap())
    }

    impl From<rsmycqu::models::Period> for Period {
        fn from(value: rsmycqu::models::Period) -> Self {
            Period {
                start: value.start.into(),
                end: value.end.into(),
            }
        }
    }
}

trait IntoStatus {
    fn into_status(self) -> Status;
}

impl<E: RsMyCQUError> IntoStatus for ApiError<E> {
    #[instrument]
    fn into_status(self) -> Status {
        match self {
            ApiError::NotLogin => Status::unauthenticated("登录失败，请检查用户名或密码"),
            ApiError::NotAccess => Status::unauthenticated("获取教务网访问权限失败，请稍后重试"),
            ApiError::Request { .. } => {
                Status::internal("教务网请求发送失败，请稍后重试，长时间出现请联系管理员员")
            }
            ApiError::ModelParse { msg, raw_response } => {
                error!(%msg, %raw_response, "教务网响应解析失败");
                Status::internal("教务网响应解析失败，请稍后重试，长时间出现请联系管理员")
            }
            ApiError::Website { msg } => Status::unavailable(format!("教务网异常：{msg}")),
            ApiError::Inner { source } => {
                error!(%source);
                Status::internal("内部异常，请联系管理员")
            }
            ApiError::Whatever { source, message } => {
                error!(?source, %message);
                Status::internal("内部异常，请联系管理员")
            }
            ApiError::Session { source } => {
                error!(%source);
                Status::internal("内部异常，请联系管理员")
            }
        }
    }
}

trait Service {
    fn access(session: &mut Session) -> impl Future<Output = Result<(), Status>> + Send;

    fn request_coalescer(&self) -> &CachedCoalescer<String, Result<Arc<RwLock<Session>>, Status>>;

    fn login_sso(
        base_login_info: proto::BaseLoginInfo,
    ) -> impl Future<Output = Result<Session, Status>> + Send {
        async move {
            let mut session = Session::new();

            rsmycqu::sso::login(
                PROXIED_CLIENT_PROVIDER
                    .get_random_client()
                    .await
                    .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                    .deref(),
                &mut session,
                &base_login_info.auth,
                &base_login_info.password,
                true,
            )
            .await
            .map_err(IntoStatus::into_status)
            .and_then(|login_result| match login_result {
                LoginResult::Success => Ok(()),
                LoginResult::IncorrectLoginCredentials => {
                    Err(Status::unauthenticated("登录失败，请检查用户名或密码"))
                }
            })?;

            Ok(session)
        }
    }

    fn login_and_access(
        base_login_info: proto::BaseLoginInfo,
    ) -> impl Future<Output = Result<Arc<RwLock<Session>>, Status>> + Send {
        async move {
            let mut session = Self::login_sso(base_login_info).await?;
            Self::access(&mut session).await?;
            Ok(Arc::new(RwLock::new(session)))
        }
    }

    fn get_authorized_session(
        &self,
        login_info: proto::BaseLoginInfo,
    ) -> impl Future<Output = Result<Arc<RwLock<Session>>, Status>> + Send
    where
        Self: Sync,
    {
        async move {
            let auth = login_info.auth.clone();
            let res = self
                .request_coalescer()
                .execute(
                    auth.clone(),
                    || async move { Self::login_and_access(login_info).await },
                    Duration::from_secs(300),
                )
                .await;

            if res.is_err() {
                self.request_coalescer().clean_cache(&auth);
            }

            res
        }
    }
}

#[allow(clippy::declare_interior_mutable_const)]
static MISSING_LOGIN_INFO_STATUS: LazyLock<Status> =
    LazyLock::new(|| Status::invalid_argument("缺失登录信息"));

const LOG_DIR: &str = "logs";
const APP_LOG_PREFIX: &str = "app.log";
const ERROR_LOG_PREFIX: &str = "error.log";
const DEFAULT_LOG_RETENTION_DAYS: usize = 30;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- 文件轮转和分级日志设置 ---
    let log_retention_days = log_retention_days();
    let app_file_appender = daily_log_appender(APP_LOG_PREFIX, log_retention_days)?;
    let (non_blocking_app_writer, _app_guard) = tracing_appender::non_blocking(app_file_appender);

    let error_file_appender = daily_log_appender(ERROR_LOG_PREFIX, log_retention_days)?;
    let (non_blocking_error_writer, _error_guard) =
        tracing_appender::non_blocking(error_file_appender);

    let app_layer = fmt::layer()
        .with_writer(non_blocking_app_writer)
        .with_filter(filter::filter_fn(|meta| *meta.level() != Level::ERROR));

    let error_layer = fmt::layer()
        .with_writer(non_blocking_error_writer)
        .json()
        .flatten_event(true)
        .with_span_list(true)
        .with_filter(filter::LevelFilter::ERROR);

    let env_filter = EnvFilter::from_default_env().add_directive(Level::INFO.into());

    tracing_subscriber::registry()
        .with(env_filter)
        .with(app_layer)
        .with(error_layer)
        .init();

    // --- gRPC 服务启动 ---
    let addr = "[::]:53211".parse()?;
    let metrics_addr = std::env::var("METRICS_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:9321".to_string())
        .parse::<SocketAddr>()?;

    tracing::info!("gRPC server listening on {}", addr);
    tokio::spawn(async move {
        if let Err(error) = metrics::serve_metrics(metrics_addr).await {
            tracing::error!(%error, "metrics server stopped");
        }
    });

    Server::builder()
        .layer(metrics::MetricsLayer)
        .layer(TraceLayer::new_for_grpc())
        .add_service(proto::mycqu_fetcher_server::MycquFetcherServer::new(
            MycquServicer::new(),
        ))
        .add_service(proto::card_fetcher_server::CardFetcherServer::new(
            CardService::new(),
        ))
        .add_service(proto::library_fetcher_server::LibraryFetcherServer::new(
            LibraryService::new(),
        ))
        .serve(addr)
        .await?;

    Ok(())
}

fn daily_log_appender(
    prefix: &'static str,
    retention_days: usize,
) -> Result<RollingFileAppender, tracing_appender::rolling::InitError> {
    RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(prefix)
        .max_log_files(max_log_files_for_retention_days(retention_days))
        .build(LOG_DIR)
}

fn log_retention_days() -> usize {
    std::env::var("LOG_RETENTION_DAYS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_LOG_RETENTION_DAYS)
}

fn max_log_files_for_retention_days(retention_days: usize) -> usize {
    retention_days.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_current_log_file_in_retention_count() {
        assert_eq!(max_log_files_for_retention_days(0), 1);
        assert_eq!(max_log_files_for_retention_days(30), 31);
        assert_eq!(max_log_files_for_retention_days(usize::MAX), usize::MAX);
    }
}
