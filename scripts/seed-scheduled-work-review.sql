-- Only run against the isolated scheduled_dev database; no external destinations.
DO $$
DECLARE owner_id uuid; personal_id uuid; project_id uuid := '019f0000-0000-7000-8000-000000000061';
BEGIN
IF current_database() <> 'scheduled_dev' THEN RAISE EXCEPTION 'Only scheduled_dev is allowed'; END IF;
SELECT id INTO STRICT owner_id FROM users ORDER BY created_at DESC LIMIT 1;
SELECT id INTO STRICT personal_id FROM workspaces WHERE user_id=owner_id AND scope='personal';
INSERT INTO projects(id,user_id,workspace_id,title,objective)
VALUES(project_id,owner_id,personal_id,'예약 업무 검증','운영 데이터와 별도로 반복 업무 흐름을 확인해요.') ON CONFLICT DO NOTHING;
INSERT INTO tasks(id,user_id,project_id,title,notes,assignee_name,due_at)
VALUES
('019f0000-0000-7000-8000-000000000062',owner_id,project_id,'오늘 마감 안내 검토','현재 담당자와 마감일이 미리보기에 반영되는지 확인해요.','홍길동',date_trunc('day',NOW() AT TIME ZONE 'Asia/Seoul') AT TIME ZONE 'Asia/Seoul'+INTERVAL '23 hours 59 minutes'),
('019f0000-0000-7000-8000-000000000063',owner_id,project_id,'내일 전달할 검토 결과 준비','참고 문서를 확인하고 변경된 내용을 정리해요.','이담당',date_trunc('day',NOW() AT TIME ZONE 'Asia/Seoul') AT TIME ZONE 'Asia/Seoul'+INTERVAL '1 day 18 hours'),
('019f0000-0000-7000-8000-000000000064',owner_id,project_id,'기한이 지난 작업 확인','후속 확인 대상이 누락되지 않는지 검증해요.','홍길동',NOW()-INTERVAL '1 day') ON CONFLICT DO NOTHING;
INSERT INTO task_assignment_public_details(task_id,user_id,summary,action_items,reference_links) VALUES('019f0000-0000-7000-8000-000000000062',owner_id,'현재 담당자와 마감일이 올바른지 확인해요.',ARRAY['초안 만들기','미리보기 확인','이번만 건너뛰기 확인'],ARRAY['https://example.com/reference']) ON CONFLICT DO NOTHING;
INSERT INTO schedule_entries(id,user_id,title,starts_at,ends_at,time_zone)
VALUES('019f0000-0000-7000-8000-000000000065',owner_id,'내일 오전 일정 확인',date_trunc('day',NOW() AT TIME ZONE 'Asia/Seoul') AT TIME ZONE 'Asia/Seoul'+INTERVAL '1 day 9 hours',date_trunc('day',NOW() AT TIME ZONE 'Asia/Seoul') AT TIME ZONE 'Asia/Seoul'+INTERVAL '1 day 10 hours','Asia/Seoul') ON CONFLICT DO NOTHING;
END $$;
