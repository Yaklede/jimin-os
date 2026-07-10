<!-- OPENDOCK:START id=files:COLOR_PLAYBOOK.md dock=opendock/design-ultrawork path=COLOR_PLAYBOOK.md -->
# Color Playbook

색은 마지막 장식이 아니라 화면의 읽기 순서와 신뢰감을 만드는 구조입니다. UI를 만들기 전에 `DESIGN.md`의 색상 계약을 확인하고, 필요한 경우 이 문서의 팔레트 루프로 보완합니다.

## 참고할 팔레트 소스

- Coolors: palette generator, palette explore, image picker, contrast checker, real design preview를 참고합니다.
- Color Hunt: hand-picked palette, mood/category 탐색을 참고합니다.
- Adobe Color: project goal, audience, color theory, 3-5 core colors plus neutrals 기준을 참고합니다.

## Palette Planning Loop

바로 색을 고르지 않습니다. 아래 순서대로 먼저 기록합니다.

1. Project mood: 제품/브랜드가 차갑고 정밀한지, 따뜻하고 편한지, 강하고 대담한지 정합니다.
2. Palette source: Coolors, Color Hunt, Adobe Color, existing brand, image extraction, custom 중 무엇을 참고했는지 적습니다.
3. Core colors: 3-5개의 핵심 색만 고릅니다. 많은 색을 동시에 주역으로 쓰지 않습니다.
4. Neutrals: canvas, surface, text, border에 쓸 중립색을 분리합니다.
5. Role map: primary action, secondary accent, success/warning/error, focus, muted text, surface, border 역할을 정합니다.
6. Contrast plan: body text, CTA text, disabled state, focus ring이 읽히는지 확인합니다.
7. Coherence check: one accent, one radius, one shadow language와 충돌하지 않는지 확인합니다.

## 피해야 할 색감

- Beige, cream, tan, olive, brown, orange 계열만으로 화면 전체를 채우는 muddy palette
- 보라/파랑 gradient만 반복하는 one-note AI palette
- accent가 2개 이상 경쟁해서 CTA 우선순위가 흐려지는 구성
- 브랜드 근거 없이 random pastel, neon, rainbow를 섞는 구성
- 본문 텍스트와 배경 대비가 약한 muted-on-muted 조합
- pure black을 기본 배경 또는 텍스트로 쓰는 구성
- warning/error/success 색을 장식 색과 혼동하는 구성

## 권장 역할 수

- Canvas: 전체 배경
- Surface: card, panel, menu
- Text: 본문과 제목
- Muted text: 보조 설명
- Border: 분리선과 card outline
- Primary accent: 주 CTA와 핵심 상태
- Secondary accent: 작은 강조 또는 illustration support
- Focus: keyboard focus ring
- Semantic: success, warning, danger

## Manifest 기록 예시

```md
Palette Source: Coolors explore + existing brand
Palette Mood: calm professional, not beige-heavy
Palette Role Map: canvas #f7f8f5, surface #ffffff, text #172033, primary #2563eb, secondary #12a594, focus #f59e0b, border #d9e2e7
Contrast Plan: primary CTA uses white text on #2563eb, body text uses #172033 on #f7f8f5, focus ring uses #f59e0b.
Color Risks: avoid beige-only warmth, avoid extra accent colors, avoid low-contrast muted text.
```

## 적용 원칙

- 팔레트 사이트에서 색을 그대로 베끼는 것이 목적이 아닙니다. 조합 방식과 역할 분리를 참고합니다.
- `DESIGN.md`가 이미 브랜드 색을 정했다면 새 색을 추가하지 말고 역할 매핑만 명확히 합니다.
- 새 색을 추가해야 한다면 `DESIGN.md`에 semantic token과 사용 역할을 함께 추가합니다.
- 화면마다 다른 팔레트를 새로 만들지 않습니다. 한 workspace 안에서는 같은 역할명이 같은 느낌을 유지해야 합니다.
<!-- OPENDOCK:END id=files:COLOR_PLAYBOOK.md dock=opendock/design-ultrawork path=COLOR_PLAYBOOK.md -->
