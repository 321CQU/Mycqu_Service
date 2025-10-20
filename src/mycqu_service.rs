//! [proto::mycqu_fetcher_server] implementation

use std::{collections::HashMap, ops::Deref, sync::Arc, time::Duration};

use rsmycqu::session::Session;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status, async_trait};
use tracing::instrument;

use crate::{
    IntoStatus, MISSING_LOGIN_INFO_STATUS, Service, proto,
    proto::mycqu_fetcher_server::MycquFetcher,
    utils::{CachedCoalescer, PROXIED_CLIENT_PROVIDER, PROXY_CLIENT_GET_ERROR},
};

pub struct MycquServicer {
    request_coalescer: CachedCoalescer<String, Result<Arc<RwLock<Session>>, Status>>,
}

impl Service for MycquServicer {
    async fn access(session: &mut Session) -> Result<(), Status> {
        rsmycqu::mycqu::access_mycqu(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session,
        )
        .await
        .map_err(IntoStatus::into_status)
    }

    fn request_coalescer(&self) -> &CachedCoalescer<String, Result<Arc<RwLock<Session>>, Status>> {
        &self.request_coalescer
    }
}

impl MycquServicer {
    pub fn new() -> Self {
        let request_coalescer = CachedCoalescer::new(Duration::from_secs(30));
        Self { request_coalescer }
    }
}

#[async_trait]
impl MycquFetcher for MycquServicer {
    #[instrument(skip(self))]
    async fn fetch_user(
        &self,
        request: Request<proto::BaseLoginInfo>,
    ) -> Result<Response<proto::UserInfo>, Status> {
        let session = self.get_authorized_session(request.into_inner()).await?;
        let user = rsmycqu::mycqu::User::fetch_self(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session.read().await.deref(),
        )
        .await
        .map_err(IntoStatus::into_status)?;

        Ok(Response::new(user.into()))
    }

    #[instrument(skip(self))]
    async fn fetch_enroll_course_info(
        &self,
        request: Request<proto::FetchEnrollCourseInfoRequest>,
    ) -> Result<Response<proto::FetchEnrollCourseInfoResponse>, Status> {
        let proto::FetchEnrollCourseInfoRequest {
            base_login_info,
            is_major,
        } = request.into_inner();

        let base_login_info = base_login_info.ok_or_else(|| MISSING_LOGIN_INFO_STATUS.clone())?;
        let session = self.get_authorized_session(base_login_info).await?;

        let result = rsmycqu::mycqu::enroll::EnrollCourseInfo::fetch_all(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session.read().await.deref(),
            is_major,
        )
        .await
        .map_err(IntoStatus::into_status)?
        .into_iter()
        .map(|(enroll_type, infos)| {
            let info = infos.into_iter().map(Into::into).collect::<Vec<_>>();

            (
                enroll_type,
                proto::fetch_enroll_course_info_response::EnrollCourseInfos { info },
            )
        })
        .collect::<HashMap<_, _>>();

        Ok(Response::new(proto::FetchEnrollCourseInfoResponse {
            result,
        }))
    }

    #[instrument(skip(self))]
    async fn fetch_enroll_course_item(
        &self,
        request: Request<proto::FetchEnrollCourseItemRequest>,
    ) -> Result<Response<proto::FetchEnrollCourseItemResponse>, Status> {
        let proto::FetchEnrollCourseItemRequest {
            base_login_info,
            id,
            is_major,
        } = request.into_inner();

        let base_login_info = base_login_info.ok_or_else(|| MISSING_LOGIN_INFO_STATUS.clone())?;
        let session = self.get_authorized_session(base_login_info).await?;

        let enroll_course_items = rsmycqu::mycqu::enroll::EnrollCourseItem::fetch_all(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session.read().await.deref(),
            id.as_str(),
            is_major,
        )
        .await
        .map_err(IntoStatus::into_status)?
        .into_iter()
        .map(Into::into)
        .collect();

        Ok(Response::new(proto::FetchEnrollCourseItemResponse {
            enroll_course_items,
        }))
    }

    #[instrument(skip(self))]
    async fn fetch_exam(
        &self,
        request: Request<proto::FetchExamRequest>,
    ) -> Result<Response<proto::FetchExamResponse>, Status> {
        let proto::FetchExamRequest {
            base_login_info,
            stu_id,
        } = request.into_inner();

        let base_login_info = base_login_info.ok_or_else(|| MISSING_LOGIN_INFO_STATUS.clone())?;
        let session = self.get_authorized_session(base_login_info).await?;

        let exams = rsmycqu::mycqu::exam::Exam::fetch_all(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session.read().await.deref(),
            stu_id,
        )
        .await
        .map_err(IntoStatus::into_status)?
        .into_iter()
        .map(Into::into)
        .collect();

        Ok(Response::new(proto::FetchExamResponse { exams }))
    }

    #[instrument(skip(self))]
    async fn fetch_all_session(
        &self,
        request: Request<proto::BaseLoginInfo>,
    ) -> Result<Response<proto::FetchAllSessionResponse>, Status> {
        let session = self.get_authorized_session(request.into_inner()).await?;
        let res = rsmycqu::mycqu::course::CQUSession::fetch_all(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session.read().await.deref(),
        )
        .await
        .map_err(IntoStatus::into_status)?
        .into_iter()
        .map(Into::into)
        .collect();

        Ok(Response::new(proto::FetchAllSessionResponse {
            sessions: res,
        }))
    }

    #[instrument(skip(self))]
    async fn fetch_curr_session_info(
        &self,
        request: Request<proto::BaseLoginInfo>,
    ) -> Result<Response<proto::CquSessionInfo>, Status> {
        let session = Self::get_authorized_session(self, request.into_inner()).await?;

        let id = rsmycqu::mycqu::course::CQUSessionInfo::fetch_curr(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session.read().await.deref(),
        )
        .await
        .map_err(IntoStatus::into_status)?
        .session
        .id
        .ok_or_else(|| Status::invalid_argument("教务网响应异常，请联系管理员"))?;

        let detail = rsmycqu::mycqu::course::CQUSessionInfo::fetch_detail(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session.read().await.deref(),
            id as u32,
        )
        .await
        .map_err(IntoStatus::into_status)?;

        Ok(Response::new(detail.into()))
    }

    #[instrument(skip(self))]
    async fn fetch_all_session_info(
        &self,
        request: Request<proto::BaseLoginInfo>,
    ) -> Result<Response<proto::FetchAllSessionInfoResponse>, Status> {
        let session = Self::get_authorized_session(self, request.into_inner()).await?;

        let session_infos = rsmycqu::mycqu::course::CQUSessionInfo::fetch_all(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session.read().await.deref(),
        )
        .await
        .map_err(IntoStatus::into_status)?
        .into_iter()
        .map(Into::into)
        .collect();
        Ok(Response::new(proto::FetchAllSessionInfoResponse {
            session_infos,
        }))
    }

    #[instrument(skip(self))]
    async fn fetch_course_timetable(
        &self,
        request: Request<proto::FetchCourseTimetableRequest>,
    ) -> Result<Response<proto::FetchCourseTimetableResponse>, Status> {
        let proto::FetchCourseTimetableRequest {
            base_login_info,
            code,
            session: cqu_session,
        } = request.into_inner();

        let base_login_info = base_login_info.ok_or_else(|| MISSING_LOGIN_INFO_STATUS.clone())?;
        let session = self.get_authorized_session(base_login_info).await?;

        let course_timetables = rsmycqu::mycqu::course::CourseTimetable::fetch_curr(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session.read().await.deref(),
            code.as_str(),
            cqu_session
                .ok_or_else(|| Status::invalid_argument("缺少查询学期信息"))?
                .id as u16,
        )
        .await
        .map_err(IntoStatus::into_status)?
        .into_iter()
        .map(Into::into)
        .collect();

        Ok(Response::new(proto::FetchCourseTimetableResponse {
            course_timetables,
        }))
    }

    #[instrument(skip(self))]
    async fn fetch_enroll_timetable(
        &self,
        request: Request<proto::FetchEnrollTimetableRequest>,
    ) -> Result<Response<proto::FetchCourseTimetableResponse>, Status> {
        let proto::FetchEnrollTimetableRequest {
            base_login_info,
            code,
        } = request.into_inner();

        let base_login_info = base_login_info.ok_or_else(|| MISSING_LOGIN_INFO_STATUS.clone())?;
        let session = self.get_authorized_session(base_login_info).await?;

        let course_timetables = rsmycqu::mycqu::course::CourseTimetable::fetch_enroll(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session.read().await.deref(),
            code,
        )
        .await
        .map_err(IntoStatus::into_status)?
        .into_iter()
        .map(Into::into)
        .collect();

        Ok(Response::new(proto::FetchCourseTimetableResponse {
            course_timetables,
        }))
    }

    #[instrument(skip(self))]
    async fn fetch_score(
        &self,
        request: Request<proto::FetchScoreRequest>,
    ) -> Result<Response<proto::FetchScoreResponse>, Status> {
        let proto::FetchScoreRequest {
            base_login_info,
            is_minor,
        } = request.into_inner();

        let base_login_info = base_login_info.ok_or_else(|| MISSING_LOGIN_INFO_STATUS.clone())?;
        let session = self.get_authorized_session(base_login_info).await?;

        let scores = rsmycqu::mycqu::score::Score::fetch_self(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session.read().await.deref(),
            is_minor,
        )
        .await
        .map_err(IntoStatus::into_status)?
        .into_iter()
        .map(Into::into)
        .collect();

        Ok(Response::new(proto::FetchScoreResponse { scores }))
    }

    #[instrument(skip(self))]
    async fn fetch_gpa_ranking(
        &self,
        request: Request<proto::BaseLoginInfo>,
    ) -> Result<Response<proto::GpaRanking>, Status> {
        let session = Self::get_authorized_session(self, request.into_inner()).await?;

        let res = rsmycqu::mycqu::score::GPARanking::fetch_self(
            PROXIED_CLIENT_PROVIDER
                .get_random_client()
                .await
                .ok_or_else(|| PROXY_CLIENT_GET_ERROR.clone())?
                .deref(),
            session.read().await.deref(),
        )
        .await
        .map_err(IntoStatus::into_status)?;

        Ok(Response::new(res.into()))
    }
}

impl From<rsmycqu::mycqu::User> for proto::UserInfo {
    fn from(value: rsmycqu::mycqu::User) -> Self {
        proto::UserInfo {
            id: value.id,
            code: value.code,
            role: value.role,
            email: value.email,
            name: value.name,
            phone_number: value.phone_number,
        }
    }
}

impl From<rsmycqu::mycqu::course::CourseDayTime> for proto::CourseDayTime {
    fn from(value: rsmycqu::mycqu::course::CourseDayTime) -> Self {
        proto::CourseDayTime {
            weekday: value.weekday.into(),
            period: Some(value.period.into()),
        }
    }
}

impl From<rsmycqu::mycqu::enroll::EnrollCourseInfo> for proto::EnrollCourseInfo {
    fn from(value: rsmycqu::mycqu::enroll::EnrollCourseInfo) -> Self {
        proto::EnrollCourseInfo {
            id: value.id,
            course: Some(value.course.into()),
            category: value.course_category,
            r#type: value.course_type,
            enroll_sign: value.enroll_sign,
            course_nature: value.course_nature,
            campus: value.campus,
        }
    }
}

impl From<rsmycqu::mycqu::enroll::EnrollCourseTimetable> for proto::EnrollCourseTimetable {
    fn from(value: rsmycqu::mycqu::enroll::EnrollCourseTimetable) -> Self {
        proto::EnrollCourseTimetable {
            weeks: value.weeks.into_iter().map(Into::into).collect(),
            time: value.time.map(Into::into),
            pos: value.pos,
        }
    }
}

impl From<rsmycqu::mycqu::enroll::EnrollCourseItem> for proto::EnrollCourseItem {
    fn from(value: rsmycqu::mycqu::enroll::EnrollCourseItem) -> Self {
        proto::EnrollCourseItem {
            id: value.id,
            session_id: value.session_id,
            checked: value.checked,
            course_id: value.course_id,
            course: Some(value.course.into()),
            r#type: value.course_type,
            selected_num: value.selected_num.map(Into::into),
            capacity: value.capacity.map(Into::into),
            children: value
                .children
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            campus: value.campus,
            parent_id: value.parent_id,
            timetables: value.timetables.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<rsmycqu::mycqu::course::CQUSession> for proto::CquSession {
    fn from(value: rsmycqu::mycqu::course::CQUSession) -> Self {
        proto::CquSession {
            id: value.id.unwrap_or_default() as u32,
            year: value.year as u32,
            is_autumn: value.is_autumn,
        }
    }
}

impl From<rsmycqu::mycqu::course::Course> for proto::Course {
    fn from(value: rsmycqu::mycqu::course::Course) -> Self {
        proto::Course {
            name: value.name,
            code: value.code,
            course_num: value.course_num,
            dept: value.dept,
            credit: value.credit.map(|credit| credit as f32),
            instructor: value.instructor,
            session: value.session.map(|session| session.into()),
        }
    }
}

impl From<rsmycqu::mycqu::course::CQUSessionInfo> for proto::CquSessionInfo {
    fn from(value: rsmycqu::mycqu::course::CQUSessionInfo) -> Self {
        let begin_date = value
            .begin_date_str
            .and_then(|date_str| proto::parser_date_time_str(date_str.as_str()).ok())
            .map(|date| date.timestamp() as u32);
        let end_date = value
            .end_date_str
            .and_then(|date_str| proto::parser_date_time_str(date_str.as_str()).ok())
            .map(|date| date.timestamp() as u32);

        proto::CquSessionInfo {
            session: Some(value.session.into()),
            begin_date,
            end_date,
        }
    }
}

impl From<rsmycqu::mycqu::course::CourseTimetable> for proto::CourseTimetable {
    fn from(value: rsmycqu::mycqu::course::CourseTimetable) -> Self {
        proto::CourseTimetable {
            course: Some(value.course.into()),
            stu_num: value.stu_num.map(Into::into),
            classroom: value.classroom,
            weeks: value.weeks.into_iter().map(Into::into).collect(),
            day_time: value.day_time.map(Into::into),
            whole_week: value.whole_week,
            classroom_name: value.classroom_name,
            expr_projects: value.expr_projects,
        }
    }
}

impl From<rsmycqu::mycqu::exam::Invigilator> for proto::Invigilator {
    fn from(value: rsmycqu::mycqu::exam::Invigilator) -> Self {
        proto::Invigilator {
            name: value.name,
            dept: value.dept,
        }
    }
}

impl From<rsmycqu::mycqu::exam::Exam> for proto::Exam {
    fn from(value: rsmycqu::mycqu::exam::Exam) -> Self {
        proto::Exam {
            course: Some(value.course.into()),
            batch: value.batch,
            batch_id: value.batch_id as u32,
            building: value.building,
            floor: value.floor.map(Into::into),
            room: value.room,
            stu_num: value.stu_num as u32,
            date: value.date_str,
            start_time: value.start_time_str,
            end_time: value.end_time_str,
            week: value.week as u32,
            weekday: value.weekday as u32,
            stu_id: value.stu_id,
            seat_num: value.seat_num as u32,
            chief_invi: value
                .chief_invigilator
                .into_iter()
                .map(Into::into)
                .collect(),
            asst_invi: value
                .asst_invigilator
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<rsmycqu::mycqu::score::Score> for proto::Score {
    fn from(value: rsmycqu::mycqu::score::Score) -> Self {
        proto::Score {
            session: Some(value.session.into()),
            course: Some(value.course.into()),
            score: value.score,
            study_nature: value.study_nature,
            course_nature: value.course_nature,
        }
    }
}

impl From<rsmycqu::mycqu::score::GPARanking> for proto::GpaRanking {
    fn from(value: rsmycqu::mycqu::score::GPARanking) -> Self {
        proto::GpaRanking {
            gpa: value.gpa,
            weighted_avg: value.weighted_avg,
            minor_gpa: value.minor_gpa,
            minor_weighted_avg: value.minor_weighted_avg,
            major_ranking: value.major_ranking.map(Into::into),
            grade_ranking: value.grade_ranking.map(Into::into),
            class_ranking: value.class_ranking.map(Into::into),
        }
    }
}
