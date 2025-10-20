//! [proto::card_fetcher_server] implementation

use std::{
    ops::{Deref, DerefMut},
    sync::Arc,
    time::Duration,
};

use rsmycqu::session::Session;
use tokio::sync::RwLock;
use tonic::{Response, Status, async_trait};
use tracing::instrument;

use crate::{
    IntoStatus, MISSING_LOGIN_INFO_STATUS, Service, proto,
    utils::{CachedCoalescer, PROXIED_CLIENT_PROVIDER, PROXY_CLIENT_GET_ERROR},
};

pub struct CardService {
    request_coalescer: CachedCoalescer<String, Result<Arc<RwLock<Session>>, Status>>,
    card_coalescer: CachedCoalescer<String, Result<rsmycqu::card::Card, Status>>,
}

impl Service for CardService {
    async fn access(session: &mut Session) -> Result<(), Status> {
        rsmycqu::card::access_card(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session,
        )
        .await
        .map_err(IntoStatus::into_status)
    }

    fn request_coalescer(&self) -> &CachedCoalescer<String, Result<Arc<RwLock<Session>>, Status>> {
        &self.request_coalescer
    }
}

impl CardService {
    pub fn new() -> Self {
        Self {
            request_coalescer: CachedCoalescer::new(Duration::from_secs(30)),
            card_coalescer: CachedCoalescer::new(Duration::from_secs(30)),
        }
    }

    #[instrument(skip(self))]
    async fn fetch_card_with_cached(
        &self,
        auth: String,
        session: Arc<RwLock<Session>>,
    ) -> Result<rsmycqu::card::Card, Status> {
        self.card_coalescer
            .execute(
                auth,
                || async move {
                    rsmycqu::card::Card::fetch_self(
                        PROXIED_CLIENT_PROVIDER
                            .get_random_client()
                            .await
                            .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                            .deref(),
                        session.read().await.deref(),
                    )
                    .await
                    .map_err(IntoStatus::into_status)
                },
                Duration::from_secs(30),
            )
            .await
    }
}

#[async_trait]
impl proto::card_fetcher_server::CardFetcher for CardService {
    #[instrument(skip(self))]
    async fn fetch_card(
        &self,
        request: tonic::Request<proto::BaseLoginInfo>,
    ) -> Result<Response<proto::Card>, Status> {
        let base_login_info = request.into_inner();
        let auth = base_login_info.auth.clone();
        let session = self.get_authorized_session(base_login_info).await?;

        let card = self.fetch_card_with_cached(auth, session).await?;

        Ok(Response::new(card.into()))
    }

    #[instrument(skip(self))]
    async fn fetch_bills(
        &self,
        request: tonic::Request<proto::BaseLoginInfo>,
    ) -> Result<Response<proto::FetchBillResponse>, Status> {
        let base_login_info = request.into_inner();
        let auth = base_login_info.auth.clone();
        let session = self.get_authorized_session(base_login_info).await?;

        let now = chrono::Local::now();
        let start_date = (now - chrono::Duration::days(30))
            .format("%Y-%m-%d")
            .to_string();
        let end_date = now.format("%Y-%m-%d").to_string();

        let card = self.fetch_card_with_cached(auth, session.clone()).await?;
        let bills = card
            .fetch_bill(
                PROXIED_CLIENT_PROVIDER
                    .get_random_client()
                    .await
                    .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                    .deref(),
                session.read().await.deref(),
                &start_date,
                &end_date,
                1,
                100,
            )
            .await
            .map_err(IntoStatus::into_status)?
            .into_iter()
            .map(TryInto::try_into)
            .filter_map(Result::ok)
            .collect();

        Ok(Response::new(proto::FetchBillResponse { bills }))
    }

    #[instrument(skip(self))]
    async fn fetch_energy_fee(
        &self,
        request: tonic::Request<proto::FetchEnergyFeeRequest>,
    ) -> Result<Response<proto::EnergyFees>, Status> {
        let proto::FetchEnergyFeeRequest {
            base_login_info,
            is_hu_xi,
            room,
        } = request.into_inner();

        let base_login_info = base_login_info.ok_or_else(|| MISSING_LOGIN_INFO_STATUS.clone())?;
        let session = self.get_authorized_session(base_login_info).await?;

        let energy_fees = rsmycqu::card::EnergyFees::fetch_self(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session.write().await.deref_mut(),
            &room,
            is_hu_xi,
        )
        .await
        .map_err(IntoStatus::into_status)?;

        Ok(Response::new(energy_fees.into()))
    }
}

impl From<rsmycqu::card::Card> for proto::Card {
    fn from(card: rsmycqu::card::Card) -> Self {
        proto::Card {
            id: card.id.to_string(),
            amount: card.amount as f32 / 100.0,
        }
    }
}

impl TryFrom<rsmycqu::card::Bill> for proto::Bill {
    type Error = chrono::ParseError;

    fn try_from(bill: rsmycqu::card::Bill) -> Result<Self, Self::Error> {
        Ok(proto::Bill {
            name: bill.name,
            date: proto::parser_date_time_str(bill.date.as_str())?.timestamp() as u32,
            place: bill.place,
            tran_amount: bill.tran_amount as f32 / 100.0,
            acc_amount: bill.acc_amount as f32 / 100.0,
        })
    }
}

impl From<rsmycqu::card::EnergyFees> for proto::EnergyFees {
    fn from(fees: rsmycqu::card::EnergyFees) -> Self {
        let (subsidies, electricity_subsidy, water_subsidy) = match fees.subsidies {
            rsmycqu::card::Subsidy::Huxi { electricity, water } => {
                (None, electricity.parse().ok(), water.parse().ok())
            }
            rsmycqu::card::Subsidy::Old { subsidies } => (subsidies.parse().ok(), None, None),
        };

        proto::EnergyFees {
            balance: fees.balance.parse().unwrap_or_default(),
            electricity_subsidy,
            water_subsidy,
            subsidies,
        }
    }
}
