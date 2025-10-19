from functools import partial

from httpx import AsyncClient
from mycqu import async_login, async_access_mycqu

from utils.ClientManager._ClientManager import ClientManager

from _321CQU.tools.Singleton import Singleton


__all__ = ['MycquClient']


async def login_and_access_mycqu(client: AsyncClient, auth: str, password: str):
    await async_login(client, auth, password, kick_others=True)
    await async_access_mycqu(client)


class MycquClient(ClientManager, metaclass=Singleton):
    async def get_logined_client(self, auth: str, password: str) -> AsyncClient:
        return await self.acquire('mycqu', auth, partial(login_and_access_mycqu, auth=auth, password=password))
