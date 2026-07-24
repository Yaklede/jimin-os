-- Local development-only data for visual and interaction review.
-- All records use deterministic IDs so this script can be run repeatedly.
-- Run with:
--   docker exec -i jimin-os-local-postgres-1 \
--     psql -v ON_ERROR_STOP=1 -U jimin_api -d jimin_os \
--     < scripts/seed-ui-review.sql

BEGIN;

DO $$
DECLARE
    review_user_id UUID;
    company_workspace_id UUID;
    personal_workspace_id UUID;
    inflow_source_id UUID;
    company_project_id UUID := '019f9000-0001-7000-8000-000000000001';
    personal_project_id UUID := '019f9000-0002-7000-8000-000000000002';
    local_today TIMESTAMPTZ :=
        date_trunc('day', now() AT TIME ZONE 'Asia/Seoul')
        AT TIME ZONE 'Asia/Seoul';
BEGIN
    SELECT id
    INTO STRICT review_user_id
    FROM users
    ORDER BY created_at
    LIMIT 1;

    SELECT id
    INTO STRICT company_workspace_id
    FROM workspaces
    WHERE user_id = review_user_id AND scope = 'company';

    SELECT id
    INTO STRICT personal_workspace_id
    FROM workspaces
    WHERE user_id = review_user_id AND scope = 'personal';

    SELECT id
    INTO inflow_source_id
    FROM project_google_chat_sources
    WHERE user_id = review_user_id AND enabled
    ORDER BY created_at
    LIMIT 1;

    INSERT INTO projects (
        id, user_id, workspace_id, title, objective, status, risk_level,
        next_action, due_at, created_at, updated_at, version
    )
    VALUES
        (
            company_project_id,
            review_user_id,
            company_workspace_id,
            '결제 운영 자동화',
            'Google Chat으로 들어오는 요청을 정리해 담당자 배정, 기한 관리, 결과 공유까지 한 흐름으로 처리한다.',
            'active',
            2,
            '유입 메시지 분류 기준과 담당자별 처리 규칙을 확정한다.',
            local_today + interval '21 days',
            now(),
            now(),
            1
        ),
        (
            personal_project_id,
            review_user_id,
            personal_workspace_id,
            '개인 성장 운영',
            '시간과 자산을 주간 단위로 돌아보고 다음 행동을 결정하는 개인 운영 루틴을 만든다.',
            'active',
            1,
            '이번 주 회고와 다음 주 집중 목표를 작성한다.',
            local_today + interval '45 days',
            now(),
            now(),
            1
        )
    ON CONFLICT (id) DO UPDATE SET
        title = EXCLUDED.title,
        objective = EXCLUDED.objective,
        status = EXCLUDED.status,
        risk_level = EXCLUDED.risk_level,
        next_action = EXCLUDED.next_action,
        due_at = EXCLUDED.due_at,
        updated_at = now();

    INSERT INTO tasks (
        id, user_id, title, notes, status, priority, due_at, completed_at,
        created_at, updated_at, version, project_id, assignee_name,
        parent_task_id
    )
    VALUES
        (
            '019f9000-0100-7000-8000-000000000100',
            review_user_id,
            '결제 알림 처리 흐름 개선',
            '유입 업무를 분류하고 담당자 배정과 Google Chat 공유까지 이어지는 전체 흐름을 완성한다.',
            'open',
            3,
            local_today + interval '10 days',
            NULL,
            now() - interval '3 days',
            now(),
            1,
            company_project_id,
            '조지민',
            NULL
        ),
        (
            '019f9000-0101-7000-8000-000000000101',
            review_user_id,
            '유입 메시지 분류 기준 확정',
            '업무 요청, 후속 질문, 상태 공유, 잡담을 구분하는 판단 기준을 문서화한다.',
            'open',
            3,
            local_today + interval '17 hours',
            NULL,
            now() - interval '2 days',
            now(),
            1,
            company_project_id,
            '조지민',
            '019f9000-0100-7000-8000-000000000100'
        ),
        (
            '019f9000-0102-7000-8000-000000000102',
            review_user_id,
            '담당자 멘션 전송 QA',
            '담당자 이름과 Google Chat 사용자 ID 매핑을 확인하고 실제 멘션 문구를 검증한다.',
            'open',
            2,
            local_today + interval '1 day 16 hours',
            NULL,
            now() - interval '1 day',
            now(),
            1,
            company_project_id,
            '김경주',
            '019f9000-0100-7000-8000-000000000100'
        ),
        (
            '019f9000-0103-7000-8000-000000000103',
            review_user_id,
            '정산 리포트 누락 원인 확인',
            '누락된 거래 건을 재현하고 집계 쿼리와 다운로드 결과를 비교한다.',
            'open',
            3,
            local_today - interval '1 day' + interval '18 hours',
            NULL,
            now() - interval '4 days',
            now(),
            1,
            company_project_id,
            '주홍석',
            NULL
        ),
        (
            '019f9000-0104-7000-8000-000000000104',
            review_user_id,
            '다음 운영 회의 안건 정리',
            '반복 요청과 미결정 항목을 묶어 다음 회의의 의사결정 안건으로 정리한다.',
            'open',
            1,
            NULL,
            NULL,
            now() - interval '1 day',
            now(),
            1,
            company_project_id,
            '조지민',
            NULL
        ),
        (
            '019f9000-0105-7000-8000-000000000105',
            review_user_id,
            '구글챗 연결 상태 점검',
            '회사 계정, 스페이스, 동기화 시각과 오류 여부를 확인했다.',
            'completed',
            1,
            local_today + interval '9 hours',
            local_today + interval '9 hours 20 minutes',
            now() - interval '2 days',
            now(),
            1,
            company_project_id,
            '김경주',
            NULL
        ),
        (
            '019f9000-0106-7000-8000-000000000106',
            review_user_id,
            '저녁 운동 40분',
            '하체 부담이 적은 유산소 20분과 근력 운동 20분을 진행한다.',
            'open',
            2,
            local_today + interval '20 hours',
            NULL,
            now(),
            now(),
            1,
            personal_project_id,
            NULL,
            NULL
        ),
        (
            '019f9000-0107-7000-8000-000000000107',
            review_user_id,
            '7월 지출 내역 분류',
            '카드·계좌 사용 내역을 고정비, 생활비, 투자 항목으로 나눈다.',
            'open',
            1,
            local_today + interval '1 day 21 hours',
            NULL,
            now(),
            now(),
            1,
            personal_project_id,
            NULL,
            NULL
        ),
        (
            '019f9000-0108-7000-8000-000000000108',
            review_user_id,
            '구독 서비스 자동 결제 항목을 점검하고 사용하지 않는 서비스의 해지 후보 정리',
            '최근 3개월 동안 사용하지 않은 정기 결제와 다음 결제일을 함께 확인한다.',
            'open',
            1,
            local_today + interval '3 days 19 hours',
            NULL,
            now(),
            now(),
            1,
            personal_project_id,
            NULL,
            NULL
        ),
        (
            '019f9000-0109-7000-8000-000000000109',
            review_user_id,
            '주간 회고 초안 작성',
            '완료한 일, 지연된 일, 다음 주에 줄일 일을 각 세 가지씩 정리했다.',
            'completed',
            1,
            local_today - interval '2 days' + interval '21 hours',
            local_today - interval '2 days' + interval '21 hours 30 minutes',
            now() - interval '4 days',
            now(),
            1,
            personal_project_id,
            NULL,
            NULL
        )
    ON CONFLICT (id) DO UPDATE SET
        title = EXCLUDED.title,
        notes = EXCLUDED.notes,
        status = EXCLUDED.status,
        priority = EXCLUDED.priority,
        due_at = EXCLUDED.due_at,
        completed_at = EXCLUDED.completed_at,
        project_id = EXCLUDED.project_id,
        assignee_name = EXCLUDED.assignee_name,
        parent_task_id = EXCLUDED.parent_task_id,
        updated_at = now();

    INSERT INTO schedule_entries (
        id, user_id, title, notes, starts_at, ends_at, time_zone, source,
        status, created_at, updated_at, version
    )
    VALUES
        (
            '019f9000-0201-7000-8000-000000000201',
            review_user_id,
            '주간 우선순위 정리',
            '오늘 처리할 일 세 가지와 미룰 일을 결정한다.',
            local_today + interval '11 hours',
            local_today + interval '11 hours 30 minutes',
            'Asia/Seoul',
            'manual',
            'confirmed',
            now(),
            now(),
            1
        ),
        (
            '019f9000-0202-7000-8000-000000000202',
            review_user_id,
            '비스킷링크 변경사항 리뷰',
            '담당자별 진행 상태와 막힌 항목을 확인한다.',
            local_today + interval '15 hours',
            local_today + interval '15 hours 45 minutes',
            'Asia/Seoul',
            'manual',
            'confirmed',
            now(),
            now(),
            1
        ),
        (
            '019f9000-0203-7000-8000-000000000203',
            review_user_id,
            '업무 시작 브리핑',
            '내일 일정, 기한 임박 업무, 판단 대기 항목을 확인한다.',
            local_today + interval '1 day 9 hours',
            local_today + interval '1 day 9 hours 30 minutes',
            'Asia/Seoul',
            'manual',
            'confirmed',
            now(),
            now(),
            1
        ),
        (
            '019f9000-0204-7000-8000-000000000204',
            review_user_id,
            '고객사 연동 점검 회의',
            '환불 상태 동기화와 운영 서버 IP 변경 요청을 함께 점검한다.',
            local_today + interval '1 day 14 hours',
            local_today + interval '1 day 15 hours',
            'Asia/Seoul',
            'manual',
            'confirmed',
            now(),
            now(),
            1
        ),
        (
            '019f9000-0205-7000-8000-000000000205',
            review_user_id,
            '완료된 일정 예시',
            '지난 일정 목록과 상세 이동을 확인하기 위한 개발 데이터다.',
            local_today - interval '1 day' + interval '16 hours',
            local_today - interval '1 day' + interval '17 hours',
            'Asia/Seoul',
            'manual',
            'confirmed',
            now() - interval '2 days',
            now(),
            1
        )
    ON CONFLICT (id) DO UPDATE SET
        title = EXCLUDED.title,
        notes = EXCLUDED.notes,
        starts_at = EXCLUDED.starts_at,
        ends_at = EXCLUDED.ends_at,
        time_zone = EXCLUDED.time_zone,
        source = EXCLUDED.source,
        status = EXCLUDED.status,
        updated_at = now();

    INSERT INTO goals (
        id, user_id, workspace_id, project_id, title, desired_outcome, status,
        target_at, created_at, updated_at, version
    )
    VALUES
        (
            '019f9000-0301-7000-8000-000000000301',
            review_user_id,
            company_workspace_id,
            company_project_id,
            '업무 요청 처리 시간 30% 단축',
            '반복 유입의 분류·배정·공유 과정을 자동화해 요청 확인부터 담당자 전달까지 걸리는 시간을 줄인다.',
            'active',
            local_today + interval '60 days',
            now(),
            now(),
            1
        ),
        (
            '019f9000-0302-7000-8000-000000000302',
            review_user_id,
            personal_workspace_id,
            personal_project_id,
            '월간 투자 여력 확보',
            '불필요한 고정비와 충동 지출을 줄여 매월 추가 투자 가능 금액을 만든다.',
            'active',
            local_today + interval '90 days',
            now(),
            now(),
            1
        )
    ON CONFLICT (id) DO UPDATE SET
        title = EXCLUDED.title,
        desired_outcome = EXCLUDED.desired_outcome,
        status = EXCLUDED.status,
        target_at = EXCLUDED.target_at,
        updated_at = now();

    INSERT INTO intelligence_signals (
        id, user_id, workspace_id, project_id, goal_id, kind, severity, title,
        summary, source_type, source_entity_id, fingerprint, status,
        observed_at, valid_until, resolved_at, created_at, updated_at, version
    )
    VALUES
        (
            '019f9000-0401-7000-8000-000000000401',
            review_user_id,
            company_workspace_id,
            company_project_id,
            '019f9000-0301-7000-8000-000000000301',
            'task_deadline',
            3,
            '기한이 지난 중요 업무가 있어요',
            '정산 리포트 누락 원인 확인 업무가 기한을 넘겼고 후속 공유가 아직 필요합니다.',
            'task',
            '019f9000-0103-7000-8000-000000000103',
            'ui-seed-overdue-task-20260724',
            'active',
            now() - interval '30 minutes',
            now() + interval '2 days',
            NULL,
            now(),
            now(),
            1
        ),
        (
            '019f9000-0402-7000-8000-000000000402',
            review_user_id,
            company_workspace_id,
            company_project_id,
            '019f9000-0301-7000-8000-000000000301',
            'schedule_conflict',
            2,
            '오후 회의 전에 담당자를 확정해야 해요',
            '15시 리뷰 전에 미배정 유입 업무의 담당자와 처리 기한을 정하면 회의 시간을 줄일 수 있습니다.',
            'system',
            NULL,
            'ui-seed-review-prep-20260724',
            'active',
            now() - interval '20 minutes',
            now() + interval '8 hours',
            NULL,
            now(),
            now(),
            1
        )
    ON CONFLICT (id) DO UPDATE SET
        title = EXCLUDED.title,
        summary = EXCLUDED.summary,
        severity = EXCLUDED.severity,
        status = 'active',
        observed_at = EXCLUDED.observed_at,
        valid_until = EXCLUDED.valid_until,
        resolved_at = NULL,
        updated_at = now();

    INSERT INTO recommendations (
        id, user_id, workspace_id, project_id, goal_id, signal_id, title,
        rationale, expected_effect, risk_summary, confidence, urgency, impact,
        risk_level, effort_minutes, suggested_action_kind, suggested_entity_id,
        status, valid_until, revisit_at, created_at, updated_at, version
    )
    VALUES
        (
            '019f9000-0501-7000-8000-000000000501',
            review_user_id,
            company_workspace_id,
            company_project_id,
            '019f9000-0301-7000-8000-000000000301',
            '019f9000-0401-7000-8000-000000000401',
            '지연된 정산 업무부터 확인하세요',
            '기한이 지난 업무를 그대로 두면 오후 리뷰에서 다시 확인해야 하고 관계자 공유도 늦어집니다.',
            '원인 확인과 담당자 후속 조치를 오늘 안에 시작할 수 있습니다.',
            '긴급한 신규 요청이 있으면 처리 순서를 다시 조정해야 합니다.',
            96,
            3,
            3,
            1,
            20,
            'update_task',
            '019f9000-0103-7000-8000-000000000103',
            'pending',
            now() + interval '8 hours',
            NULL,
            now(),
            now(),
            1
        ),
        (
            '019f9000-0502-7000-8000-000000000502',
            review_user_id,
            company_workspace_id,
            company_project_id,
            '019f9000-0301-7000-8000-000000000301',
            '019f9000-0402-7000-8000-000000000402',
            '13시를 집중 업무 시간으로 확보해 보세요',
            '오후 회의 전 비어 있는 시간에 유입 메시지 분류 기준을 정리하면 회의에서 바로 확정할 수 있습니다.',
            '회의 시간을 줄이고 오늘 마감 업무의 완료 가능성을 높입니다.',
            NULL,
            88,
            2,
            2,
            0,
            60,
            'create_schedule',
            NULL,
            'pending',
            now() + interval '6 hours',
            NULL,
            now(),
            now(),
            1
        )
    ON CONFLICT (id) DO UPDATE SET
        title = EXCLUDED.title,
        rationale = EXCLUDED.rationale,
        expected_effect = EXCLUDED.expected_effect,
        risk_summary = EXCLUDED.risk_summary,
        confidence = EXCLUDED.confidence,
        urgency = EXCLUDED.urgency,
        impact = EXCLUDED.impact,
        risk_level = EXCLUDED.risk_level,
        effort_minutes = EXCLUDED.effort_minutes,
        status = 'pending',
        valid_until = EXCLUDED.valid_until,
        revisit_at = NULL,
        updated_at = now();

    INSERT INTO meetings (
        id, user_id, workspace_id, project_id, title, transcript, started_at,
        duration_seconds, status, summary, topics, risks, follow_up,
        analyzed_at, created_at, updated_at, version
    )
    VALUES
        (
            '019f9000-0601-7000-8000-000000000601',
            review_user_id,
            company_workspace_id,
            company_project_id,
            '비스킷링크 운영 개선 회의',
            '조지민: 유입 메시지를 그대로 일감으로 만들면 제목과 설명이 너무 길어집니다. 김경주: 요청 단위로 묶고 담당자와 마감일을 먼저 제안해야 합니다. 주홍석: 기한이 지난 업무는 홈에서 바로 보여야 합니다.',
            local_today - interval '1 day' + interval '14 hours',
            2700,
            'review_ready',
            '유입 대화를 업무 단위로 정리한 뒤 담당자와 기한을 제안하고, 승인된 항목만 할 일로 승격하기로 했습니다.',
            ARRAY['유입 업무 분류', '담당자 배정', '기한 알림'],
            ARRAY['기존 대화 재수집 시 중복 업무가 생길 수 있음'],
            '분류 기준을 문서화한 뒤 신규 스페이스 데이터로 승격 흐름을 다시 검증합니다.',
            now() - interval '1 day',
            now() - interval '1 day',
            now(),
            1
        ),
        (
            '019f9000-0602-7000-8000-000000000602',
            review_user_id,
            personal_workspace_id,
            personal_project_id,
            '7월 개인 운영 회고',
            '조지민: 기록은 했지만 다음 행동을 정하지 않은 항목이 많았습니다. 다음 주에는 저녁 운동과 지출 분류를 먼저 루틴으로 만들겠습니다.',
            local_today - interval '3 days' + interval '20 hours',
            1800,
            'applied',
            '다음 주에는 저녁 운동과 지출 분류를 우선 루틴으로 두고 완료 여부를 주간 회고에서 확인하기로 했습니다.',
            ARRAY['건강 루틴', '지출 점검'],
            ARRAY[]::TEXT[],
            '일요일 저녁에 한 주 결과를 다시 확인합니다.',
            now() - interval '3 days',
            now() - interval '3 days',
            now(),
            1
        )
    ON CONFLICT (id) DO UPDATE SET
        title = EXCLUDED.title,
        transcript = EXCLUDED.transcript,
        started_at = EXCLUDED.started_at,
        duration_seconds = EXCLUDED.duration_seconds,
        status = EXCLUDED.status,
        summary = EXCLUDED.summary,
        topics = EXCLUDED.topics,
        risks = EXCLUDED.risks,
        follow_up = EXCLUDED.follow_up,
        analyzed_at = EXCLUDED.analyzed_at,
        updated_at = now();

    INSERT INTO meeting_decisions (
        id, meeting_id, content, rationale, source_excerpt,
        source_timestamp_seconds, created_at
    )
    VALUES
        (
            '019f9000-0611-7000-8000-000000000611',
            '019f9000-0601-7000-8000-000000000601',
            '원문 메시지가 아니라 정리된 업무 초안을 먼저 보여준다.',
            '불필요한 댓글과 반복 문구가 그대로 일감에 포함되는 문제를 줄이기 위해서다.',
            '요청 단위로 묶고 담당자와 마감일을 먼저 제안해야 합니다.',
            740,
            now()
        ),
        (
            '019f9000-0612-7000-8000-000000000612',
            '019f9000-0601-7000-8000-000000000601',
            '승격 전에 담당자와 마감일을 한 화면에서 확인한다.',
            '승격 후 다시 편집하고 웹훅을 보내는 반복 작업을 없애기 위해서다.',
            '담당자와 마감일을 먼저 제안해야 합니다.',
            810,
            now()
        )
    ON CONFLICT (id) DO UPDATE SET
        content = EXCLUDED.content,
        rationale = EXCLUDED.rationale,
        source_excerpt = EXCLUDED.source_excerpt,
        source_timestamp_seconds = EXCLUDED.source_timestamp_seconds;

    INSERT INTO meeting_action_items (
        id, meeting_id, kind, project_id, title, notes, priority, due_at,
        starts_at, ends_at, time_zone, source_excerpt, confidence, status,
        target_entity_id, applied_at, rejected_at, created_at, updated_at,
        version
    )
    VALUES
        (
            '019f9000-0621-7000-8000-000000000621',
            '019f9000-0601-7000-8000-000000000601',
            'task',
            company_project_id,
            '유입 메시지 분류 기준 문서화',
            '업무 요청, 질문, 상태 공유, 잡담의 판별 예시를 각각 세 개씩 정리합니다.',
            3,
            local_today + interval '2 days',
            NULL,
            NULL,
            NULL,
            '분류 기준을 문서화한 뒤 신규 스페이스 데이터로 검증합니다.',
            94,
            'suggested',
            '019f9000-0701-7000-8000-000000000701',
            NULL,
            NULL,
            now(),
            now(),
            1
        ),
        (
            '019f9000-0622-7000-8000-000000000622',
            '019f9000-0601-7000-8000-000000000601',
            'schedule',
            company_project_id,
            '유입 업무 승격 흐름 재검증',
            NULL,
            2,
            NULL,
            local_today + interval '2 days 15 hours',
            local_today + interval '2 days 16 hours',
            'Asia/Seoul',
            '신규 스페이스 데이터로 승격 흐름을 다시 검증합니다.',
            91,
            'suggested',
            '019f9000-0702-7000-8000-000000000702',
            NULL,
            NULL,
            now(),
            now(),
            1
        )
    ON CONFLICT (id) DO UPDATE SET
        title = EXCLUDED.title,
        notes = EXCLUDED.notes,
        priority = EXCLUDED.priority,
        due_at = EXCLUDED.due_at,
        starts_at = EXCLUDED.starts_at,
        ends_at = EXCLUDED.ends_at,
        time_zone = EXCLUDED.time_zone,
        source_excerpt = EXCLUDED.source_excerpt,
        confidence = EXCLUDED.confidence,
        status = 'suggested',
        applied_at = NULL,
        rejected_at = NULL,
        updated_at = now();

    IF inflow_source_id IS NOT NULL THEN
        INSERT INTO project_inflow_items (
            id, user_id, project_id, source_id, provider_message_name,
            provider_thread_name, sender_name, content_text, received_at,
            status, promoted_task_id, acknowledged_at, created_at, updated_at,
            version, sender_provider_name
        )
        VALUES
            (
                '019f9000-0801-7000-8000-000000000801',
                review_user_id,
                (SELECT project_id FROM project_google_chat_sources WHERE id = inflow_source_id),
                inflow_source_id,
                'spaces/dev/messages/ui-seed-01',
                'spaces/dev/threads/ui-seed-01',
                '김경주',
                '정산내역 CSV 다운로드 시 승인번호 열이 필요합니다. 기존 파일 형식은 유지하고 금요일까지 가능 여부를 알려 주세요.',
                now() - interval '5 minutes',
                'pending',
                NULL,
                NULL,
                now(),
                now(),
                1,
                'users/112959013708458147544'
            ),
            (
                '019f9000-0802-7000-8000-000000000802',
                review_user_id,
                (SELECT project_id FROM project_google_chat_sources WHERE id = inflow_source_id),
                inflow_source_id,
                'spaces/dev/messages/ui-seed-02',
                'spaces/dev/threads/ui-seed-02',
                '주홍석',
                '카드 결제를 취소했는데 환불 상태가 계속 처리 중으로 보입니다. 재현 확인 후 오늘 안에 원인 공유 부탁드립니다.',
                now() - interval '12 minutes',
                'pending',
                NULL,
                NULL,
                now(),
                now(),
                1,
                'users/107243631365817505408'
            ),
            (
                '019f9000-0803-7000-8000-000000000803',
                review_user_id,
                (SELECT project_id FROM project_google_chat_sources WHERE id = inflow_source_id),
                inflow_source_id,
                'spaces/dev/messages/ui-seed-03',
                'spaces/dev/threads/ui-seed-03',
                '송인준',
                '신규 가맹점 등록에서 법인번호를 입력하면 저장 버튼이 비활성화됩니다. 영향 범위와 수정 일정을 확인해 주세요.',
                now() - interval '18 minutes',
                'pending',
                NULL,
                NULL,
                now(),
                now(),
                1,
                'users/106728169696918757516'
            ),
            (
                '019f9000-0804-7000-8000-000000000804',
                review_user_id,
                (SELECT project_id FROM project_google_chat_sources WHERE id = inflow_source_id),
                inflow_source_id,
                'spaces/dev/messages/ui-seed-04',
                'spaces/dev/threads/ui-seed-04',
                '이의현',
                '운영 서버 IP 허용 목록 변경 요청이 들어왔습니다. 필요한 IP 정보를 확인하고 내일 오전까지 반영 가능 여부를 답변해 주세요.',
                now() - interval '25 minutes',
                'pending',
                NULL,
                NULL,
                now(),
                now(),
                1,
                'users/107987990660367540952'
            ),
            (
                '019f9000-0805-7000-8000-000000000805',
                review_user_id,
                (SELECT project_id FROM project_google_chat_sources WHERE id = inflow_source_id),
                inflow_source_id,
                'spaces/dev/messages/ui-seed-05',
                'spaces/dev/threads/ui-seed-05',
                '조지민',
                '#3901 거래내역 생성 시 정산방식 표기 요청입니다. 기존 데이터와 신규 데이터에 동일하게 노출되는지 검토가 필요합니다.',
                now() - interval '31 minutes',
                'pending',
                NULL,
                NULL,
                now(),
                now(),
                1,
                'users/113145855577166216187'
            )
        ON CONFLICT (id) DO UPDATE SET
            sender_name = EXCLUDED.sender_name,
            content_text = EXCLUDED.content_text,
            received_at = EXCLUDED.received_at,
            status = 'pending',
            promoted_task_id = NULL,
            acknowledged_at = NULL,
            sender_provider_name = EXCLUDED.sender_provider_name,
            updated_at = now();

        INSERT INTO project_inflow_analyses (
            id, user_id, project_id, source_id, conversation_key,
            representative_item_id, state, classification, confidence, summary,
            suggested_task_title, suggested_action_items,
            suggested_completion_criteria, suggested_assignee_name,
            suggested_due_at, suggested_priority, linked_task_id,
            analysis_model_id, analysis_version, source_revision,
            analyzed_revision, claim_owner, claim_expires_at, attempt_count,
            error_code, analyzed_at, created_at, updated_at, version
        )
        VALUES
            (
                '019f9000-0811-7000-8000-000000000811',
                review_user_id,
                (SELECT project_id FROM project_google_chat_sources WHERE id = inflow_source_id),
                inflow_source_id,
                'thread:spaces/dev/threads/ui-seed-01',
                '019f9000-0801-7000-8000-000000000801',
                'ready',
                'new_task',
                94,
                '정산내역 CSV에 승인번호를 추가하되 기존 파일 형식을 유지하고 금요일까지 가능 여부를 공유해야 합니다.',
                '정산내역 CSV 승인번호 열 추가',
                ARRAY[
                    '기존 CSV 컬럼 순서와 포맷을 확인한다',
                    '승인번호 열을 추가하고 샘플 파일을 검증한다',
                    '금요일까지 처리 가능 여부를 관계자에게 공유한다'
                ],
                '기존 형식을 유지한 CSV에서 승인번호가 정확히 노출되고 처리 일정이 공유됩니다.',
                '김경주',
                local_today + interval '1 day 18 hours',
                2,
                NULL,
                'dev-seed',
                '1',
                1,
                1,
                NULL,
                NULL,
                0,
                NULL,
                now(),
                now(),
                now(),
                1
            ),
            (
                '019f9000-0812-7000-8000-000000000812',
                review_user_id,
                (SELECT project_id FROM project_google_chat_sources WHERE id = inflow_source_id),
                inflow_source_id,
                'thread:spaces/dev/threads/ui-seed-02',
                '019f9000-0802-7000-8000-000000000802',
                'ready',
                'new_task',
                97,
                '카드 결제 취소 후 환불 상태가 갱신되지 않는 문제를 재현하고 당일 원인을 공유해야 합니다.',
                '결제 취소 후 환불 상태 미갱신 오류 확인',
                ARRAY[
                    '결제 취소 시나리오를 재현한다',
                    '환불 상태 동기화 로그를 확인한다',
                    '원인과 수정 계획을 공유한다'
                ],
                '재현 결과와 원인, 수정 계획이 관계자에게 공유됩니다.',
                '주홍석',
                local_today + interval '17 hours',
                3,
                NULL,
                'dev-seed',
                '1',
                1,
                1,
                NULL,
                NULL,
                0,
                NULL,
                now(),
                now(),
                now(),
                1
            ),
            (
                '019f9000-0813-7000-8000-000000000813',
                review_user_id,
                (SELECT project_id FROM project_google_chat_sources WHERE id = inflow_source_id),
                inflow_source_id,
                'thread:spaces/dev/threads/ui-seed-03',
                '019f9000-0803-7000-8000-000000000803',
                'ready',
                'new_task',
                92,
                '신규 가맹점 등록에서 법인번호 입력 후 저장 버튼이 비활성화되는 문제의 영향 범위와 수정 일정을 확인해야 합니다.',
                '가맹점 법인번호 입력 시 저장 버튼 비활성화 수정',
                ARRAY[
                    '법인번호 입력 검증 조건을 확인한다',
                    '개인·법인 가맹점 영향 범위를 구분한다',
                    '수정 가능 일정을 공유한다'
                ],
                '법인번호가 정상 입력되면 저장 버튼이 활성화되고 영향 범위와 일정이 정리됩니다.',
                '송인준',
                local_today + interval '2 days 18 hours',
                2,
                NULL,
                'dev-seed',
                '1',
                1,
                1,
                NULL,
                NULL,
                0,
                NULL,
                now(),
                now(),
                now(),
                1
            ),
            (
                '019f9000-0814-7000-8000-000000000814',
                review_user_id,
                (SELECT project_id FROM project_google_chat_sources WHERE id = inflow_source_id),
                inflow_source_id,
                'thread:spaces/dev/threads/ui-seed-04',
                '019f9000-0804-7000-8000-000000000804',
                'ready',
                'new_task',
                90,
                '운영 서버 IP 허용 목록 변경에 필요한 정보를 확인하고 내일 오전까지 반영 가능 여부를 답변해야 합니다.',
                '운영 서버 IP 허용 목록 변경 검토',
                ARRAY[
                    '변경 대상 IP와 포트를 확인한다',
                    '방화벽 반영 범위와 영향도를 검토한다',
                    '내일 오전까지 가능 여부를 답변한다'
                ],
                '필요 정보와 반영 가능 시점이 확정되어 관계자에게 전달됩니다.',
                '이의현',
                local_today + interval '1 day 11 hours',
                2,
                NULL,
                'dev-seed',
                '1',
                1,
                1,
                NULL,
                NULL,
                0,
                NULL,
                now(),
                now(),
                now(),
                1
            ),
            (
                '019f9000-0815-7000-8000-000000000815',
                review_user_id,
                (SELECT project_id FROM project_google_chat_sources WHERE id = inflow_source_id),
                inflow_source_id,
                'thread:spaces/dev/threads/ui-seed-05',
                '019f9000-0805-7000-8000-000000000805',
                'ready',
                'new_task',
                95,
                '거래내역 생성 시 정산방식이 기존·신규 데이터에 동일하게 표시되도록 요구 범위와 데이터 영향을 검토해야 합니다.',
                '#3901 거래내역 정산방식 표기 범위 확정',
                ARRAY[
                    '현재 정산방식 저장 위치를 확인한다',
                    '기존 데이터의 표시 가능 여부를 검토한다',
                    '신규 데이터와 동일한 노출 기준을 확정한다'
                ],
                '기존·신규 데이터의 정산방식 노출 기준과 구현 범위가 확정됩니다.',
                '조지민',
                local_today + interval '3 days 18 hours',
                2,
                NULL,
                'dev-seed',
                '1',
                1,
                1,
                NULL,
                NULL,
                0,
                NULL,
                now(),
                now(),
                now(),
                1
            )
        ON CONFLICT (id) DO UPDATE SET
            state = 'ready',
            classification = EXCLUDED.classification,
            confidence = EXCLUDED.confidence,
            summary = EXCLUDED.summary,
            suggested_task_title = EXCLUDED.suggested_task_title,
            suggested_action_items = EXCLUDED.suggested_action_items,
            suggested_completion_criteria =
                EXCLUDED.suggested_completion_criteria,
            suggested_assignee_name = EXCLUDED.suggested_assignee_name,
            suggested_due_at = EXCLUDED.suggested_due_at,
            suggested_priority = EXCLUDED.suggested_priority,
            linked_task_id = NULL,
            source_revision = 1,
            analyzed_revision = 1,
            claim_owner = NULL,
            claim_expires_at = NULL,
            error_code = NULL,
            analyzed_at = now(),
            updated_at = now();
    END IF;
END
$$;

COMMIT;
