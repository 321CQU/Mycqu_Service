use std::{
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

use crate::{
    card_service::CardService,
    library_service::LibraryService,
    mycqu_service::MycquServicer,
    utils::{CachedCoalescer, PROXIED_CLIENT_PROVIDER, PROXY_CLIENT_GET_ERROR},
};

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
    fn into_status(self) -> Status {
        match self {
            ApiError::NotLogin => Status::unauthenticated("登录失败，请检查用户名或密码"),
            ApiError::NotAccess => Status::unauthenticated("获取教务网访问权限失败，请稍后重试"),
            ApiError::Request { .. } => {
                Status::internal("教务网请求发送失败，请稍后重试，长时间出现请联系管理员员")
            }
            ApiError::ModelParse { .. } => {
                Status::internal("教务网响应解析失败，请稍后重试，长时间出现请联系管理员")
            }
            ApiError::Website { msg } => Status::unavailable(format!("教务网异常：{msg}")),
            ApiError::Inner { .. } => Status::internal("内部异常，请联系管理员"),
            ApiError::Whatever { .. } => Status::internal("内部异常，请联系管理员"),
            ApiError::Session { .. } => Status::internal("内部异常，请联系管理员"),
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::]:53211".parse()?;

    Server::builder()
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
