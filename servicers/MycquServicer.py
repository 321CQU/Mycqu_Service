from datetime import datetime, date

import micro_services_protobuf.mycqu_service.mycqu_service_pb2_grpc as ms_grpc
import micro_services_protobuf.mycqu_service.mycqu_request_response_pb2 as ms_rr
import micro_services_protobuf.mycqu_service.mycqu_model_pb2 as ms_m
from httpx import AsyncClient
from google.protobuf.json_format import ParseDict, MessageToDict

from mycqu import User, EnrollCourseInfo, EnrollCourseItem, Exam, CQUSession, CQUSessionInfo, CourseTimetable, \
    Score, GpaRanking
from _321CQU.tools.protobufBridge import model2protobuf, model_list2protobuf

from utils.ClientManager import MycquClient


def _date2timestamp(_date: date) -> int:
    return int(datetime.fromisoformat(_date.isoformat()).timestamp())


class MycquServicer(ms_grpc.MycquFetcherServicer):
    def __init__(self):
        super().__init__()
        self.client_manager = MycquClient()

    async def get_logined_client(self, info: ms_rr.BaseLoginInfo) -> AsyncClient:
        return await self.client_manager.get_logined_client(info.auth, info.password)

    async def FetchUser(self, request: ms_rr.BaseLoginInfo, context):
        client = await self.get_logined_client(request)
        info = await User.async_fetch_self(client)
        return model2protobuf(info, ms_m.UserInfo)

    async def FetchEnrollCourseInfo(self, request: ms_rr.FetchEnrollCourseInfoRequest, context):
        client = await self.get_logined_client(request.base_login_info)
        info = await EnrollCourseInfo.async_fetch(client, request.is_major)
        res = {}
        for key, value in info.items():
            res[key] = {'info': [i.dict() for i in value]}
        return ParseDict({'result': res}, ms_rr.FetchEnrollCourseInfoResponse())

    async def FetchEnrollCourseItem(self, request: ms_rr.FetchEnrollCourseItemRequest, context):
        client = await self.get_logined_client(request.base_login_info)
        info = await EnrollCourseItem.async_fetch(client, request.id, request.is_major)
        return model_list2protobuf(info, 'enroll_course_items', ms_rr.FetchEnrollCourseItemResponse)

    async def FetchExam(self, request: ms_rr.FetchExamRequest, context):
        client = await self.get_logined_client(request.base_login_info)
        info = await Exam.async_fetch(client, request.stu_id)
        res = []
        for exam in info:
            temp = exam.dict()
            temp['date'] = exam.date.isoformat()
            temp['start_time'] = exam.start_time.isoformat()
            temp['end_time'] = exam.end_time.isoformat()
            res.append(temp)

        return ParseDict({'exams': res}, ms_rr.FetchExamResponse())

    async def FetchAllSession(self, request: ms_rr.BaseLoginInfo, context):
        client = await self.get_logined_client(request)
        info = await CQUSession.async_fetch(client)
        return model_list2protobuf(info, 'sessions', ms_rr.FetchAllSessionResponse)

    async def FetchCurrSessionInfo(self, request: ms_rr.BaseLoginInfo, context):
        client = await self.get_logined_client(request)
        info = await CQUSessionInfo.async_fetch(client)
        res = info.dict(exclude_none=True)
        if info.begin_date is not None:
            res['begin_date'] = _date2timestamp(info.begin_date)
        if info.end_date is not None:
            res['end_date'] = _date2timestamp(info.end_date)
        return ParseDict(res, ms_m.CquSessionInfo())

    async def FetchAllSessionInfo(self, request: ms_rr.BaseLoginInfo, context):
        client = await self.get_logined_client(request)
        info = await CQUSessionInfo.async_fetch_all(client)
        res = []
        for session_info in info:
            temp = session_info.dict()
            if session_info.begin_date is not None:
                temp['begin_date'] = _date2timestamp(session_info.begin_date)
            if session_info.end_date is not None:
                temp['end_date'] = _date2timestamp(session_info.end_date)
            res.append(temp)
        return ParseDict({'session_infos': res}, ms_rr.FetchAllSessionInfoResponse())

    async def FetchCourseTimetable(self, request: ms_rr.FetchCourseTimetableRequest, context):
        client = await self.get_logined_client(request.base_login_info)
        info = await CourseTimetable.async_fetch(
            client, request.code,
            CQUSession.parse_obj(MessageToDict(
                request.session, including_default_value_fields=True, preserving_proto_field_name=True
            )))
        return model_list2protobuf(info, 'course_timetables', ms_rr.FetchCourseTimetableResponse)

    async def FetchEnrollTimetable(self, request: ms_rr.FetchEnrollTimetableRequest, context):
        client = await self.get_logined_client(request.base_login_info)
        info = await CourseTimetable.async_fetch_enroll(client, request.code)
        return model_list2protobuf(info, 'course_timetables', ms_rr.FetchCourseTimetableResponse)

    async def FetchScore(self, request: ms_rr.FetchScoreRequest, context):
        client = await self.get_logined_client(request.base_login_info)
        info = await Score.async_fetch(client, request.is_minor)
        return model_list2protobuf(info, 'scores', ms_rr.FetchScoreResponse)

    async def FetchGpaRanking(self, request: ms_rr.BaseLoginInfo, context):
        client = await self.get_logined_client(request)
        info = await GpaRanking.async_fetch(client)
        return model2protobuf(info, ms_m.GpaRanking)
