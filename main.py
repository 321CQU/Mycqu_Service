import asyncio
import logging
import grpc

from micro_services_protobuf.mycqu_service import mycqu_service_pb2_grpc as ms_grpc
from _321CQU.tools.gRPCManager import gRPCManager, ServiceEnum

from servicers.MycquServicer import MycquServicer
from servicers.CardServicer import CardServicer
from servicers.LibraryServicer import LibraryServicer


async def serve():
    port = gRPCManager().get_service_config(ServiceEnum.MycquService)[1]

    server = grpc.aio.server()
    ms_grpc.add_MycquFetcherServicer_to_server(MycquServicer(), server)
    ms_grpc.add_CardFetcherServicer_to_server(CardServicer(), server)
    ms_grpc.add_LibraryFetcherServicer_to_server(LibraryServicer(), server)
    server.add_insecure_port('[::]:' + port)
    await server.start()
    await server.wait_for_termination()


if __name__ == '__main__':
    print("启动 mycqu service 服务")
    logging.basicConfig(level=logging.INFO)
    asyncio.new_event_loop().run_until_complete(serve())
