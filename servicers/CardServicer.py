import time
from typing import Tuple, Dict

import micro_services_protobuf.mycqu_service.mycqu_service_pb2_grpc as ms_grpc
import micro_services_protobuf.mycqu_service.mycqu_request_response_pb2 as ms_rr
import micro_services_protobuf.mycqu_service.mycqu_model_pb2 as ms_m
from httpx import AsyncClient

from mycqu import Card, EnergyFees
from _321CQU.tools.protobufBridge import model2protobuf, model_list2protobuf

from utils.ClientManager import CardClient
from utils.handleMycquError import handle_mycqu_error


class CardServicer(ms_grpc.CardFetcherServicer):
    def __init__(self):
        super().__init__()
        self.client_manager = CardClient()
        self.cards: Dict[str, Tuple[Card, int]] = {}

    async def get_logined_client(self, info: ms_rr.BaseLoginInfo) -> AsyncClient:
        return await self.client_manager.get_logined_client(info.auth, info.password)

    @handle_mycqu_error
    async def get_card(self, auth: str, client: AsyncClient) -> Card:
        info = self.cards.get(auth)
        now = time.time()
        if info is not None and now - info[1] > 60 * 10:
            return info[0]
        else:
            card = await Card.async_fetch(client)
            self.cards[auth] = (card, int(now))
            return card

    @handle_mycqu_error
    async def FetchCard(self, request: ms_rr.BaseLoginInfo, context):
        client = await self.get_logined_client(request)
        info = await self.get_card(request.auth, client)
        return model2protobuf(info, ms_m.Card)

    @handle_mycqu_error
    async def FetchBills(self, request: ms_rr.BaseLoginInfo, context):
        client = await self.get_logined_client(request)
        info = await (await self.get_card(request.auth, client)).async_fetch_bills(client)
        return model_list2protobuf(info, 'bills', ms_rr.FetchBillResponse)

    @handle_mycqu_error
    async def FetchEnergyFee(self, request: ms_rr.FetchEnergyFeeRequest, context):
        client = await self.get_logined_client(request.base_login_info)
        info = await EnergyFees.async_fetch(client, request.is_hu_xi, request.room)
        return model2protobuf(info, ms_m.EnergyFees)
