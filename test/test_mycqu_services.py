import unittest

import grpc
from micro_services_protobuf.mycqu_service import mycqu_request_response_pb2 as ms_rr
from micro_services_protobuf.mycqu_service import mycqu_model_pb2 as ms_m
from micro_services_protobuf.mycqu_service import mycqu_service_pb2_grpc as ms_grpc

from utils.configManager import ConfigReader


class MyTestCase(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self) -> None:
        self.channel = grpc.aio.insecure_channel('localhost:53211')
        self.stub = ms_grpc.MycquFetcherStub(self.channel)

        config = ConfigReader()

        self.login_info = ms_rr.BaseLoginInfo(auth=config.get_config('test', 'auth'),
                                              password=config.get_config('test', 'password'))
        self.sid = config.get_config('test', 'sid')

    async def asyncTearDown(self) -> None:
        await self.channel.close()

    async def test_fetch_user(self):
        res = await self.stub.FetchUser(self.login_info)
        print(res)
        self.assertIsInstance(res, ms_m.UserInfo)

    async def test_fetch_enroll_course_info(self):
        res = await self.stub.FetchEnrollCourseInfo(ms_rr.FetchEnrollCourseInfoRequest(base_login_info=self.login_info,
                                                                                       is_major=True))
        print(res)
        self.assertIsInstance(res, ms_rr.FetchEnrollCourseInfoResponse)

    async def test_fetch_enroll_course_item(self):
        res = await self.stub.FetchEnrollCourseItem(ms_rr.FetchEnrollCourseItemRequest(base_login_info=self.login_info,
                                                                                       id='10000004360', is_major=True))
        print(res)
        self.assertIsInstance(res, ms_rr.FetchEnrollCourseItemResponse)

    async def test_fetch_exam(self):
        res = await self.stub.FetchExam(ms_rr.FetchExamRequest(base_login_info=self.login_info, stu_id=self.sid))
        print(res)
        self.assertIsInstance(res, ms_rr.FetchExamResponse)

    async def test_fetch_all_session(self):
        res = await self.stub.FetchAllSession(self.login_info)
        print(res)
        self.assertIsInstance(res, ms_rr.FetchAllSessionResponse)

    async def test_fetch_curr_session_info(self):
        res = await self.stub.FetchCurrSessionInfo(self.login_info)
        print(res)
        self.assertIsInstance(res, ms_m.CquSessionInfo)

    async def test_fetch_all_session_info(self):
        res = await self.stub.FetchAllSessionInfo(self.login_info)
        print(res)
        self.assertIsInstance(res, ms_rr.FetchAllSessionInfoResponse)

    async def test_fetch_course_timetable(self):
        res = await self.stub.FetchCourseTimetable(
            ms_rr.FetchCourseTimetableRequest(
                base_login_info=self.login_info,
                code=self.sid,
                session=ms_m.CquSession(year=2022, is_autumn=True)
            )
        )
        print(res)
        self.assertIsInstance(res, ms_rr.FetchCourseTimetableResponse)

    async def test_fetch_enroll_timetable(self):
        res = await self.stub.FetchEnrollTimetable(
            ms_rr.FetchEnrollTimetableRequest(
                base_login_info=self.login_info,
                code=self.sid,
            )
        )
        print(res)
        self.assertIsInstance(res, ms_rr.FetchCourseTimetableResponse)

    async def test_fetch_score(self):
        res = await self.stub.FetchScore(ms_rr.FetchScoreRequest(base_login_info=self.login_info, is_minor=False))
        print(res)
        self.assertIsInstance(res, ms_rr.FetchScoreResponse)

    async def test_fetch_gpa_ranking(self):
        res = await self.stub.FetchGpaRanking(self.login_info)
        print(res)
        self.assertIsInstance(res, ms_m.GpaRanking)


if __name__ == '__main__':
    unittest.main()
