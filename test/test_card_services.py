import unittest

import grpc
from micro_services_protobuf.mycqu_service import mycqu_request_response_pb2 as ms_rr
from micro_services_protobuf.mycqu_service import mycqu_model_pb2 as ms_m
from micro_services_protobuf.mycqu_service import mycqu_service_pb2_grpc as ms_grpc

from utils.configManager import ConfigReader


class MyTestCase(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self) -> None:
        self.channel = grpc.aio.insecure_channel('localhost:53211')
        self.stub = ms_grpc.CardFetcherStub(self.channel)

        config = ConfigReader()

        self.login_info = ms_rr.BaseLoginInfo(auth=config.get_config('test', 'auth'),
                                              password=config.get_config('test', 'password'))
        self.sid = config.get_config('test', 'sid')

    async def asyncTearDown(self) -> None:
        await self.channel.close()

    async def test_get_card(self):
        res = await self.stub.FetchCard(self.login_info)
        print(res)
        self.assertIsInstance(res, ms_m.Card)

    async def test_get_bill(self):
        res = await self.stub.FetchBills(self.login_info)
        print(res)
        self.assertIsInstance(res, ms_rr.FetchBillResponse)

    async def test_get_energy_fees(self):
        res = await self.stub.FetchEnergyFee(ms_rr.FetchEnergyFeeRequest(base_login_info=self.login_info,
                                                                         is_hu_xi=True,
                                                                         room='B5321'))
        print(res)
        self.assertIsInstance(res, ms_m.EnergyFees)


if __name__ == '__main__':
    unittest.main()
