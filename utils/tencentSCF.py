import asyncio
from typing import Callable, Awaitable, TypeVar, Generic, Any

from _321CQU.tools.ConfigHandler import _CONFIG_HANDLER
from _321CQU.tools.Singleton import Singleton
from tencentserverless import scf
from tencentserverless.scf import Client

Connection = TypeVar('Connection')

class ClientPool(Generic[Connection], metaclass=Singleton):
    def __init__(
            self,
            create_client: Callable[[], Awaitable[Connection]],
            max_size: int = 10,
    ):
        self.create_client = create_client
        self.max_size = max_size
        self.semaphore = asyncio.Semaphore(max_size)
        self.pool = asyncio.Queue(max_size)
        self._created = 0  # 当前已创建的客户端数量

    async def get(self) -> Connection:
        """
        获取一个可用的客户端。如果池已满，等待其他客户端被释放。
        """
        # 尝试直接从队列中获取可用的客户端
        try:
            client = self.pool.get_nowait()
            return client
        except asyncio.QueueEmpty:
            pass

        # 如果池未满，创建一个新的客户端
        async with self.semaphore:
            if self._created < self.max_size:
                client = await self.create_client()
                self._created += 1
                return client
            else:
                # 如果池已满，等待队列中的客户端被释放
                return await self.pool.get()

    async def put(self, client: Connection) -> None:
        """
        将客户端放回池中。如果池已满，(client 将被保留或丢弃)
        """
        if not self.pool.full():
            await self.pool.put(client)
        else:
            # 如果池已满，可以考虑释放资源或者等待
            # 这里简单地将客户端放回队列（假设队列满的情况比较少见）
            await self.pool.put(client)

    async def close(self):
        """
        关闭所有客户端并清理池
        """
        while not self.pool.empty():
            client = self.pool.get_nowait()
            await client.close()

class TencentSCF(metaclass=Singleton):
    def __init__(self):
        self.sercet_id = _CONFIG_HANDLER.get_config("TencentCloud", "secret_id")
        self.secret_key = _CONFIG_HANDLER.get_config("TencentCloud", "secret_key")
        self.region = _CONFIG_HANDLER.get_config("TencentCloud", "region")
        self.mycqu_function_name = _CONFIG_HANDLER.get_config("TencentCloud", "mycqu_function_name")
        self.client_pool = ClientPool(self.create_client)


    async def create_client(self) -> Client:
        return scf.Client(region=self.region, secret_id=self.sercet_id, secret_key=self.secret_key)

    async def get_client(self) -> Client:
        return await self.client_pool.get()

    async def invoke_mycqu(self, data: Any):
        client = await self.get_client()
        res = client.invoke(self.mycqu_function_name, data=data)
        await self.client_pool.put(client)
        return res

SCF = TencentSCF()
