from typing import List, Tuple, Type

from mycqu.exception import *
from mycqu.exception import MycquException
from _321CQU.tools import grpc_method_error_handler

__all__ = ['handle_mycqu_error']

ERROR_INFO: List[Tuple[Type[MycquException], str]] = [
    (CQUWebsiteError, "学校网站异常，请稍后重试"),
    (NotAllowedService, "无权访问学校该服务"),
    (NeedCaptcha, "需要验证码，请前往教务网登陆后重试"),
    (InvaildCaptcha, "无效的验证码"),
    (IncorrectLoginCredentials, "用户名或密码错误"),
    (TicketGetError, "无法获取ticket"),
    (ParseError, "无法解析数据"),
    (UnknownAuthserverException, "登陆/认证过程中发生未知错误"),
    (NotLogined, "用户未登陆"),
    (MultiSessionConflict, "启用了单点登陆，请关闭后重试"),
    (MycquUnauthorized, "未获取认证或认证过期"),
    (InvalidRoom, "无效的教室名")
]

handle_mycqu_error = grpc_method_error_handler(ERROR_INFO)
