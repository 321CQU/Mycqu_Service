from typing import List

import micro_services_protobuf.mycqu_service.mycqu_service_pb2_grpc as ms_grpc
import micro_services_protobuf.mycqu_service.mycqu_request_response_pb2 as ms_rr
import micro_services_protobuf.mycqu_service.mycqu_model_pb2 as ms_m
from httpx import AsyncClient

from mycqu import BookInfo
from _321CQU.tools.protobufBridge import model_list2protobuf

from utils.ClientManager import LibraryClient
from utils.handleMycquError import handle_mycqu_error


class LibraryServicer(ms_grpc.LibraryFetcherServicer):
    def __init__(self):
        super().__init__()
        self.client_manager = LibraryClient()

    async def get_logined_client(self, info: ms_rr.BaseLoginInfo) -> AsyncClient:
        return await self.client_manager.get_logined_client(info.auth, info.password)

    @handle_mycqu_error
    async def FetchBorrowBook(self, request: ms_rr.FetchBorrowBookRequest, context):
        client = await self.get_logined_client(request.info)
        infos = await BookInfo.async_fetch(client, request.is_curr)

        result: List[ms_m.BookInfo] = []

        for info in infos:
            result.append(
                ms_m.BookInfo(
                    id=info.id,
                    title=info.title,
                    call_no=info.call_no,
                    library_name=info.library_name,
                    borrow_time=int(info.borrow_time.timestamp()),
                    should_return_time=info.should_return_time.strftime("%Y-%m-%d") if info.should_return_time is not None else None,
                    is_return=info.is_return,
                    return_time=info.return_time.strftime("%Y-%m-%d") if info.return_time is not None else None,
                    renew_count=info.renew_count,
                    can_renew=info.can_renew
                )
            )

        return ms_rr.FetchBorrowBookResponse(book_infos=result)

    @handle_mycqu_error
    async def RenewBook(self, request: ms_rr.RenewBookRequest, context):
        client = await self.get_logined_client(request.info)
        info = await BookInfo.async_renew(client, request.book_id)
        return ms_rr.RenewBookResponse(message=info)
