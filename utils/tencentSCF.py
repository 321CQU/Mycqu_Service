from typing import Any

from _321CQU.tools.ConfigHandler import _CONFIG_HANDLER
from _321CQU.tools.Singleton import Singleton
from tencentserverless import scf
from tencentserverless.scf import Client


class TencentSCF(metaclass=Singleton):
    def __init__(self):
        self.sercet_id = _CONFIG_HANDLER.get_config("TencentCloud", "secret_id")
        self.secret_key = _CONFIG_HANDLER.get_config("TencentCloud", "secret_key")
        self.region = _CONFIG_HANDLER.get_config("TencentCloud", "region")
        self.mycqu_function_name = _CONFIG_HANDLER.get_config("TencentCloud", "mycqu_function_name")


    def create_client(self) -> Client:
        return scf.Client(region=self.region, secret_id=self.sercet_id, secret_key=self.secret_key)

    def invoke_mycqu(self, data: Any):
        client = self.create_client()
        res = client.invoke(self.mycqu_function_name, data=data)
        return res

SCF = TencentSCF()
