from typing import List, Tuple, Type

from mycqu.exception import *
from mycqu.exception import MycquException
from _321CQU.tools import grpc_method_error_handler

__all__ = ['handle_mycqu_error']

ERROR_INFO: List[Tuple[Type[MycquException], str, bool]] = [
    (CQUWebsiteError, "学校网站异常，请稍后重试", False),
    (NotAllowedService, "无权访问学校该服务", False),
    (NeedCaptcha, "需要验证码，请前往教务网登陆后重试", True),
    (InvaildCaptcha, "无效的验证码", False),
    (IncorrectLoginCredentials, "用户名或密码错误", True),
    (TicketGetError, "无法获取ticket", False),
    (ParseError, "无法解析数据", False),
    (UnknownAuthserverException, "登陆/认证过程中发生未知错误", False),
    (NotLogined, "用户未登陆", False),
    (MultiSessionConflict, "启用了单点登陆，请关闭后重试", False),
    (MycquUnauthorized, "未获取认证或认证过期", False),
    (InvalidRoom, "无效的教室名", False)
]

handle_mycqu_error = grpc_method_error_handler(ERROR_INFO)
