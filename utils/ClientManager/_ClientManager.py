import time
from typing import Callable, Awaitable, Dict, Tuple, List
import asyncio
import random

from httpx import AsyncClient

from utils.configManager import ConfigReader


__all__ = ['ClientManager']

USER_AGENT = [
    'Mozilla/4.76 [en_jp] (X11; U; SunOS 5.8 sun4u)',
    'Mozilla/5.0 (X11; U; Linux i686; en-US; rv:1.9.0.8) Gecko Fedora/1.9.0.8-1.fc10 Kazehakase/0.5.6',
    'Mozilla/5.0 (X11; Linux i686; U;) Gecko/20070322 Kazehakase/0.4.5',
    'Mozilla/5.0 (Windows NT 6.1; WOW64) AppleWebKit/537.1 (KHTML, like Gecko) Chrome/21.0.1180.71 Safari/537.1 LBBROWSER',
    'Mozilla/4.0 (compatible; MSIE 6.0; Windows NT 5.1; SV1; .NET CLR 1.1.4322; .NET CLR 2.0.50727)',
    'Mozilla/5.0 (Windows NT 6.1; WOW64) AppleWebKit/535.11 (KHTML, like Gecko) Chrome/17.0.963.56 Safari/535.11',
    'Mozilla/5.0 (Windows NT 6.1; WOW64) AppleWebKit/537.11 (KHTML, like Gecko) Chrome/23.0.1271.64 Safari/537.11',
    'Mozilla/4.0 (compatible; MSIE 6.0; Windows NT 5.1; SV1; QQDownload 732; .NET4.0C; .NET4.0E)',
    'Mozilla/5.0 (Windows NT 6.1; WOW64) AppleWebKit/535.11 (KHTML, like Gecko) Chrome/17.0.963.56 Safari/535.11',
    'Mozilla/4.0 (compatible; MSIE 7.0; Windows NT 5.1; Trident/4.0; SV1; QQDownload 732; .NET4.0C; .NET4.0E; 360SE)',
    'Mozilla/4.0 (compatible; MSIE 7.0b; Windows NT 5.2; .NET CLR 1.1.4322; .NET CLR 2.0.50727; InfoPath.2; .NET CLR 3.0.04506.30)',
    'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_7_3) AppleWebKit/535.20 (KHTML, like Gecko) Chrome/19.0.1036.7 Safari/535.20',
    'Mozilla/5.0 (X11; U; Linux i686; en-US; rv:1.9.0.8) Gecko Fedora/1.9.0.8-1.fc10 Kazehakase/0.5.6',
    'Mozilla/5.0 (X11; U; Linux x86_64; zh-CN; rv:1.9.2.10) Gecko/20100922 Ubuntu/10.10 (maverick) Firefox/3.6.10',
    'Mozilla/5.0 (Windows NT 6.1; WOW64) AppleWebKit/537.1 (KHTML, like Gecko) Chrome/21.0.1180.71 Safari/537.1 LBBROWSER',
    'Mozilla/5.0 (Windows NT 6.1; WOW64) AppleWebKit/537.1 (KHTML, like Gecko) Chrome/21.0.1180.89 Safari/537.1',
    'Mozilla/4.0 (compatible; MSIE 7.0; Windows NT 6.0; Acoo Browser; SLCC1; .NET CLR 2.0.50727; Media Center PC 5.0; .NET CLR 3.0.04506)',
    'Mozilla/5.0 (X11; U; Linux i686; en-US; rv:1.8.0.12) Gecko/20070731 Ubuntu/dapper-security Firefox/1.5.0.12',
    'Mozilla/4.0 (compatible; MSIE 6.0; Windows NT 5.1; SV1; QQDownload 732; .NET4.0C; .NET4.0E; LBBROWSER)',
    'Mozilla/5.0 (Windows NT 6.1; WOW64) AppleWebKit/537.1 (KHTML, like Gecko) Chrome/21.0.1180.89 Safari/537.1',
    'Mozilla/4.8 [en] (X11; U; SunOS; 5.7 sun4u)'
]


class ClientManager:
    """
    用于管理httpx所有Client的类，基于description和id实现复用，十分钟后自动释放Client
    """
    def __init__(self):
        self.clients: Dict[str, Dict[str, Tuple[AsyncClient, int]]] = {}
        reader = ConfigReader()
        self.client_lock = asyncio.Lock()
        self.overtime_check_time = int(reader.get_config('ClientManager', 'overtime_check_time'))
        self.client_overtime = int(reader.get_config('ClientManager', 'client_overtime'))
        self.clean_task = asyncio.create_task(self.remove_overtime_client())

    async def acquire(self, description: str, id: str, login: Callable[[AsyncClient], Awaitable]) -> AsyncClient:
        async with self.client_lock:
            if description in self.clients.keys() and id in self.clients[description]:
                return self.clients[description][id][0]

        return await self.launch_new_client(description, id, login)

    async def launch_new_client(self, description: str, id: str, login: Callable[[AsyncClient], Awaitable]) -> AsyncClient:
        client = AsyncClient(headers={'User-Agent': random.choice(USER_AGENT)}, timeout=20, verify=False)
        await login(client)
        async with self.client_lock:
            if description not in self.clients.keys():
                self.clients[description] = {}
            self.clients[description][id] = (client, int(time.time()))
        return client

    async def remove_overtime_client(self):
        await asyncio.sleep(self.overtime_check_time)

        now = int(time.time())

        should_remove_path: List[Tuple[str, str]] = []
        should_remove_client: List[AsyncClient] = []

        async with self.client_lock:
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

        self.clean_task = asyncio.create_task(self.remove_overtime_client())
