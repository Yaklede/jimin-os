<!-- OPENDOCK:START id=files:WRITING.md dock=opendock/ux-writing-ultrawork path=WRITING.md -->
# WRITING.md

이 파일은 이 프로젝트의 문구 계약입니다. UX Writing Ultrawork는 일반 원칙보다 이 파일을 우선합니다.

## Brand Voice

- Professional but easy
- Clear before clever
- Warm, not childish
- Helpful, not technical

## Language Policy

Primary locales:
- ko
- en

Default Korean ending: 해요체
Default English style: plain, concise, sentence case

## Korean

### Prefer

- 사용자가 바로 이해할 수 있는 말
- 능동형 문장
- 긍정형 문장
- 해결 행동이 있는 에러 메시지
- 짧은 버튼 문구

### Avoid

- 개발자 내부 용어
- 명사+명사 구조 남발
- 수동형 남발
- 같은 화면 안에서 해요체와 합니다체 혼용
- 사용자가 할 수 있는 행동이 없는 에러

### Examples

| Avoid | Prefer |
| --- | --- |
| 인증 토큰이 만료되었습니다 | 다시 로그인해 주세요 |
| 유효하지 않은 payload입니다 | 입력한 내용을 다시 확인해 주세요 |
| endpoint 호출에 실패했습니다 | 잠시 후 다시 시도해 주세요 |
| 권한이 없습니다 | 이 작업을 할 수 있는 권한이 없어요 |

## English

### Prefer

- Plain language
- Action-first labels
- Sentence case
- One idea per sentence
- Error messages with a recovery action

### Avoid

- Internal implementation terms
- All caps labels
- Vague failure messages
- Blameful wording
- Long button labels

### Examples

| Avoid | Prefer |
| --- | --- |
| Invalid payload | Check the information and try again |
| Token expired | Sign in again |
| Endpoint request failed | Try again in a moment |
| Permission denied | You do not have access to this action |

## UI Copy Rules

### Buttons

- Use verbs when possible.
- Keep labels short.
- Match the user's next action.

### Errors

- Say what happened in plain language.
- Tell the user what to do next.
- Avoid exposing internal systems.

### Empty States

- Explain what is missing.
- Offer the next useful action.

### Loading

- Use calm, short language.
- Do not overpromise.

### Success

- Confirm the result.
- Avoid exaggerated praise.

## Naming Rules

- Names should match the product concept.
- Names should be easy to say and remember.
- Avoid internal project code names in public UI.
- Avoid names that sound like database tables, API resources, or admin tools.
<!-- OPENDOCK:END id=files:WRITING.md dock=opendock/ux-writing-ultrawork path=WRITING.md -->
