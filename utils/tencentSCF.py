import json
from functools import wraps
from typing import Any

from _321CQU.tools.ConfigHandler import _CONFIG_HANDLER
from _321CQU.tools.Singleton import Singleton
from grpc import StatusCode
from grpc.aio import ServicerContext
from tencentserverless import scf
from tencentserverless.scf import Client


class SCFException(Exception):
    def __init__(self, msg: str, *args):
        self.msg = msg
        super().__init__(*args)

class TencentSCF(metaclass=Singleton):
    def __init__(self):
        self.sercet_id = _CONFIG_HANDLER.get_config("TencentCloud", "secret_id")
        self.secret_key = _CONFIG_HANDLER.get_config("TencentCloud", "secret_key")
        self.region = _CONFIG_HANDLER.get_config("TencentCloud", "region")
        self.mycqu_function_name = _CONFIG_HANDLER.get_config("TencentCloud", "mycqu_function_name")


    def create_client(self) -> Client:
        return scf.Client(region=self.region, secret_id=self.sercet_id, secret_key=self.secret_key)

    def invoke_mycqu(self, data: Any) -> object:
        client = self.create_client()
        res = client.invoke(self.mycqu_function_name, data=data)
        res = json.loads(res)
        if res['status'] != 1:
            raise SCFException(res['error'])
        return res['result']

def handle_scf_error(func):
    @wraps(func)
    async def wrapper(self, request, context: ServicerContext):
        try:
            return await func(self, request, context)
        except Exception as e:
            if isinstance(e, SCFException):
                await context.abort(
                    code=StatusCode.INVALID_ARGUMENT,
                    details=e.msg
                )
            raise e
    return wrapper

SCF = TencentSCF()
