import asyncio
import logging
import grpc

from micro_services_protobuf.mycqu_service import mycqu_service_pb2_grpc as ms_grpc

from utils.configManager import ConfigReader
from servicers.MycquServicer import MycquServicer
from servicers.CardServicer import CardServicer


async def serve():
    reader = ConfigReader()
    port = reader.get_config('gRPCServiceConfig', 'MycquServicePort')

    server = grpc.aio.server()
    ms_grpc.add_MycquFetcherServicer_to_server(MycquServicer(), server)
    ms_grpc.add_CardFetcherServicer_to_server(CardServicer(), server)
    server.add_insecure_port('[::]:' + port)
    await server.start()
    await server.wait_for_termination()


if __name__ == '__main__':
    logging.basicConfig(level=logging.INFO)
    asyncio.new_event_loop().run_until_complete(serve())
