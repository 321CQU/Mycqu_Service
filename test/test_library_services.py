import unittest

import grpc
from micro_services_protobuf.mycqu_service import mycqu_request_response_pb2 as ms_rr
from micro_services_protobuf.mycqu_service import mycqu_model_pb2 as ms_m
from micro_services_protobuf.mycqu_service import mycqu_service_pb2_grpc as ms_grpc

from utils.configManager import ConfigReader


class MyTestCase(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self) -> None:
        self.channel = grpc.aio.insecure_channel('localhost:53211')
        self.stub = ms_grpc.LibraryFetcherStub(self.channel)

        config = ConfigReader()

        self.login_info = ms_rr.BaseLoginInfo(auth=config.get_config('test', 'auth'),
                                              password=config.get_config('test', 'password'))
        self.sid = config.get_config('test', 'sid')

    async def asyncTearDown(self) -> None:
        await self.channel.close()

    async def test_fetch_borrow_book(self):
        res1 = await self.stub.FetchBorrowBook(ms_rr.FetchBorrowBookRequest(info=self.login_info, is_curr=True))
        res2 = await self.stub.FetchBorrowBook(ms_rr.FetchBorrowBookRequest(info=self.login_info, is_curr=False))
        print(res1, res2)
        self.assertIsInstance(res1, ms_rr.FetchBorrowBookResponse)
        self.assertIsInstance(res2, ms_rr.FetchBorrowBookResponse)

    async def test_renew_book(self):
        res = await self.stub.RenewBook(ms_rr.RenewBookRequest(info=self.login_info, book_id='123456'))
        print(res)
        self.assertIsInstance(res, ms_rr.RenewBookResponse)


if __name__ == '__main__':
    unittest.main()
