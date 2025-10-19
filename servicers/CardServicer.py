import json
import time
from typing import Tuple, Dict, List

import micro_services_protobuf.mycqu_service.mycqu_service_pb2_grpc as ms_grpc
import micro_services_protobuf.mycqu_service.mycqu_request_response_pb2 as ms_rr
import micro_services_protobuf.mycqu_service.mycqu_model_pb2 as ms_m
from httpx import AsyncClient

from mycqu import Card, EnergyFees, Bill
from _321CQU.tools.protobufBridge import model2protobuf

from utils.handleMycquError import handle_mycqu_error
from utils.tencentSCF import handle_scf_error, SCF


class CardServicer(ms_grpc.CardFetcherServicer):
    def __init__(self):
        super().__init__()

    @handle_scf_error
    async def FetchCard(self, request: ms_rr.BaseLoginInfo, context):
        res = SCF.invoke_mycqu({
            "username": request.auth,
            "password": request.password,
            "target": [
                "card",
                "card"
            ],
            "params": {}
        })
        info = Card.model_validate(res)
        return model2protobuf(info, ms_m.Card)

    @handle_scf_error
    async def FetchBills(self, request: ms_rr.BaseLoginInfo, context):
        res = SCF.invoke_mycqu(
            {
                "username": request.auth,
                "password": request.password,
                "target": [
                    "card",
                    "bills"
                ],
                "params": {}
            }
        )

        info = [Bill.model_validate(i) for i in res]

        result: List[ms_m.Bill] = []
        for bill in info:
            result.append(
                ms_m.Bill(
                    name=bill.name,
                    date=int(bill.date.timestamp()),
                    place=bill.place,
                    tran_amount=bill.tran_amount,
                    acc_amount=bill.acc_amount
                )
            )

        return ms_rr.FetchBillResponse(bills=result)

    @handle_scf_error
    async def FetchEnergyFee(self, request: ms_rr.FetchEnergyFeeRequest, context):
        res = SCF.invoke_mycqu(
            {
                "username": request.base_login_info.auth,
                "password": request.base_login_info.password,
                "target": [
                    "card",
                    "energy_fees"
                ],
                "params": {
                    "is_huxi": request.is_hu_xi,
                    "room": request.room
                }
            }
        )
        info = EnergyFees.model_validate(res)
        return model2protobuf(info, ms_m.EnergyFees)
