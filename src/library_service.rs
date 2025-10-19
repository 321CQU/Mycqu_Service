//! [proto::library_fetcher_server::LibraryFetcher] implementation.

use tonic::{Request, Response, Status, async_trait};

use crate::proto;

pub struct LibraryService {}

impl LibraryService {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl proto::library_fetcher_server::LibraryFetcher for LibraryService {
    async fn fetch_borrow_book(
        &self,
        _request: Request<proto::FetchBorrowBookRequest>,
    ) -> Result<Response<proto::FetchBorrowBookResponse>, Status> {
        Err(Status::unimplemented("Not yet implemented"))
    }
    async fn renew_book(
        &self,
        _request: Request<proto::RenewBookRequest>,
    ) -> Result<Response<proto::RenewBookResponse>, Status> {
        Err(Status::unimplemented("Not yet implemented"))
    }
}
