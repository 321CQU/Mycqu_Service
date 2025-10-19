from functools import partial

from httpx import AsyncClient
from mycqu import async_login, async_access_card

from utils.ClientManager._ClientManager import ClientManager

from _321CQU.tools.Singleton import singleton


__all__ = ['CardClient']


async def login_and_access_card(client: AsyncClient, auth: str, password: str):
    await async_login(client, auth, password, kick_others=True)
    await async_access_card(client)


@singleton
class CardClient(ClientManager):
    async def get_logined_client(self, auth: str, password: str) -> AsyncClient:
        return await self.acquire('card', auth, partial(login_and_access_card, auth=auth, password=password))
