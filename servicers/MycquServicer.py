import json
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
from utils.handleMycquError import handle_mycqu_error
from utils.tencentSCF import SCF


def _date2timestamp(_date: date) -> int:
    return int(datetime.fromisoformat(_date.isoformat()).timestamp())


class MycquServicer(ms_grpc.MycquFetcherServicer):
    def __init__(self):
        super().__init__()
        self.client_manager = MycquClient()

    async def get_logined_client(self, info: ms_rr.BaseLoginInfo) -> AsyncClient:
        return await self.client_manager.get_logined_client(info.auth, info.password)

    @handle_mycqu_error
    async def FetchUser(self, request: ms_rr.BaseLoginInfo, context):
        res = await SCF.invoke_mycqu({
            "username": request.auth,
            "password": request.password,
            "target": [
                "mycqu",
                "user_info"
            ],
            "params": {}
        })
        info = User.model_validate_json(res)
        return model2protobuf(info, ms_m.UserInfo)

    @handle_mycqu_error
    async def FetchEnrollCourseInfo(self, request: ms_rr.FetchEnrollCourseInfoRequest, context):
        client = await self.get_logined_client(request.base_login_info)
        info = await EnrollCourseInfo.async_fetch(client, request.is_major)
        res = {}
        for key, value in info.items():
            res[key] = {'info': [i.model_dump() for i in value]}
        return ParseDict({'result': res}, ms_rr.FetchEnrollCourseInfoResponse())

    @handle_mycqu_error
    async def FetchEnrollCourseItem(self, request: ms_rr.FetchEnrollCourseItemRequest, context):
        client = await self.get_logined_client(request.base_login_info)
        info = await EnrollCourseItem.async_fetch(client, request.id, request.is_major)
        return model_list2protobuf(info, 'enroll_course_items', ms_rr.FetchEnrollCourseItemResponse)

    @handle_mycqu_error
    async def FetchExam(self, request: ms_rr.FetchExamRequest, context):
        res = await SCF.invoke_mycqu({
            "username": request.base_login_info.auth,
            "password": request.base_login_info.password,
            "target": [
                "mycqu",
                "exam"
            ],
            "params": {}
        })
        info = [Exam.model_validate(i) for i in json.loads(res)]
        res = []
        for exam in info:
            temp = exam.model_dump()
            temp['date'] = exam.date.isoformat()
            temp['start_time'] = exam.start_time.isoformat()
            temp['end_time'] = exam.end_time.isoformat()
            res.append(temp)

        return ParseDict({'exams': res}, ms_rr.FetchExamResponse())

    @handle_mycqu_error
    async def FetchAllSession(self, request: ms_rr.BaseLoginInfo, context):
        client = await self.get_logined_client(request)
        info = await CQUSession.async_fetch(client)
        return model_list2protobuf(info, 'sessions', ms_rr.FetchAllSessionResponse)

    @handle_mycqu_error
    async def FetchCurrSessionInfo(self, request: ms_rr.BaseLoginInfo, context):
        res = await SCF.invoke_mycqu({
            "username": request.auth,
            "password": request.password,
            "target": [
                "mycqu",
                "curr_session_info"
            ],
            "params": {}
        })
        info = CQUSessionInfo.model_validate_json(res)
        res = info.model_dump(exclude_none=True)
        if info.begin_date is not None:
            res['begin_date'] = _date2timestamp(info.begin_date)
        if info.end_date is not None:
            res['end_date'] = _date2timestamp(info.end_date)
        return ParseDict(res, ms_m.CquSessionInfo())

    @handle_mycqu_error
    async def FetchAllSessionInfo(self, request: ms_rr.BaseLoginInfo, context):
        res = await SCF.invoke_mycqu({
            "username": request.auth,
            "password": request.password,
            "target": [
                "mycqu",
                "all_session_info"
            ],
            "params": {}
        })
        info = [CQUSessionInfo.model_validate(i) for i in json.loads(res)]
        res = []
        for session_info in info:
            temp = session_info.model_dump()
            if session_info.begin_date is not None:
                temp['begin_date'] = _date2timestamp(session_info.begin_date)
            if session_info.end_date is not None:
                temp['end_date'] = _date2timestamp(session_info.end_date)
            res.append(temp)
        return ParseDict({'session_infos': res}, ms_rr.FetchAllSessionInfoResponse())

    @handle_mycqu_error
    async def FetchCourseTimetable(self, request: ms_rr.FetchCourseTimetableRequest, context):
        cqu_session = CQUSession.model_validate(MessageToDict(
            request.session, including_default_value_fields=True, preserving_proto_field_name=True
        ))
        if cqu_session.id == 0:
            cqu_session.id = None
        res = await SCF.invoke_mycqu({
            "username": request.base_login_info.auth,
            "password": request.base_login_info.password,
            "target": [
                "mycqu",
                "course_timetable"
            ],
            "params": {
                "cqu_session": cqu_session.model_dump(mode='json')
            }
        })
        info = [CourseTimetable.model_validate(i) for i in json.loads(res)]
        return model_list2protobuf(info, 'course_timetables', ms_rr.FetchCourseTimetableResponse)

    @handle_mycqu_error
    async def FetchEnrollTimetable(self, request: ms_rr.FetchEnrollTimetableRequest, context):
        client = await self.get_logined_client(request.base_login_info)
        info = await CourseTimetable.async_fetch_enroll(client)
        return model_list2protobuf(info, 'course_timetables', ms_rr.FetchCourseTimetableResponse)

    @handle_mycqu_error
    async def FetchScore(self, request: ms_rr.FetchScoreRequest, context):
        res = await SCF.invoke_mycqu({
            "username": request.base_login_info.auth,
            "password": request.base_login_info.password,
            "target": [
                "mycqu",
                "score"
            ],
            "params": {
                "is_minor": request.is_minor
            }
        })
        scores: list[dict] = json.loads(res)
        info = [Score.model_validate(i) for i in scores]
        return model_list2protobuf(info, 'scores', ms_rr.FetchScoreResponse)

    @handle_mycqu_error
    async def FetchGpaRanking(self, request: ms_rr.BaseLoginInfo, context):
        res = await SCF.invoke_mycqu({
            "username": request.auth,
            "password": request.password,
            "target": [
                "mycqu",
                "gpa_ranking"
            ],
            "params": {}
        })
        info = GpaRanking.model_validate_json(res)
        return model2protobuf(info, ms_m.GpaRanking)
