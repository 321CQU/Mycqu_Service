import time
from typing import Callable, Awaitable, Dict, Tuple, List
import asyncio

from httpx import AsyncClient

from utils.configManager import ConfigReader


__all__ = ['ClientManager']


class ClientManager:
    """
    用于管理httpx所有Client的类，基于description和id实现复用，十分钟后自动释放Client
    """
    def __init__(self):
        self.clients: Dict[str, Dict[str, Tuple[AsyncClient, int]]] = {}
        reader = ConfigReader()
        self.overtime_check_time = int(reader.get_config('ClientManager', 'overtime_check_time'))
        self.client_overtime = int(reader.get_config('ClientManager', 'client_overtime'))
        asyncio.create_task(self.remove_overtime_client())

    async def acquire(self, description: str, id: str, login: Callable[[AsyncClient], Awaitable]) -> AsyncClient:
        if description in self.clients.keys() and id in self.clients[description]:
            return self.clients[description][id][0]
        else:
            return await self.launch_new_client(description, id, login)

    async def launch_new_client(self, description: str, id: str, login: Callable[[AsyncClient], Awaitable]) -> AsyncClient:
        client = AsyncClient(timeout=20)
        await login(client)
        if description not in self.clients.keys():
            self.clients[description] = {}
        self.clients[description][id] = (client, int(time.time()))
        return client

    async def remove_overtime_client(self):
        await asyncio.sleep(self.overtime_check_time)

        now = int(time.time())

        should_remove_path: List[Tuple[str, str]] = []
        should_remove_client: List[AsyncClient] = []

        for description, pair in self.clients.items():
            for id, value in pair.items():
                if now - value[1] > self.client_overtime:
                    should_remove_path.append((description, id))
                    should_remove_client.append(value[0])

        async with asyncio.TaskGroup() as tg:
            for client in should_remove_client:
                tg.create_task(client.aclose())

        for description, id in should_remove_path:
            self.clients[description].pop(id)

        asyncio.create_task(self.remove_overtime_client())
