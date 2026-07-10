<!-- OPENDOCK:START id=files:PATTERN_GUIDE.md dock=opendock/design-ultrawork path=PATTERN_GUIDE.md -->
# Pattern Guide

레퍼런스에서 추출한 패턴을 실제 UI에 적용할 때의 기준입니다. 항상 `DESIGN.md`와 충돌하지 않는 범위에서 사용합니다.

## Navbar

- 단순 site: logo, primary nav 3-5개, primary CTA 1개.
- SaaS: product, solution, resources, pricing, CTA가 기본입니다.
- Commerce: category, search, cart, account가 우선입니다.
- Mobile: hamburger보다 bottom nav나 compact sheet가 더 나은지 먼저 판단합니다.
- Mega menu는 정보량이 실제로 많을 때만 사용합니다.

## CTA

- Primary CTA는 하나의 행동을 분명히 말합니다.
- Secondary CTA는 primary와 시각적 무게가 같으면 안 됩니다.
- CTA section에는 offer, reason to act, trust note가 있어야 합니다.
- CTA copy는 “시작하기”보다 작업 맥락에 맞는 동사를 우선합니다.

## Hero

- Product-first: 실제 제품/상태/화면을 보여줘야 할 때.
- Text-first: 메시지와 포지셔닝이 더 중요할 때.
- Visual-first: 브랜드/캠페인/포트폴리오에서 감각이 핵심일 때.
- Split layout은 가능하지만 card 속 hero를 기본값으로 쓰지 않습니다.

## Cards

- 반복 카드의 radius, shadow, image ratio, metadata order를 통일합니다.
- 카드 안에 또 다른 card를 넣지 않습니다.
- 상품/글/프로젝트 카드에는 비교에 필요한 정보를 같은 위치에 둡니다.
- hover는 정보 탐색을 돕는 수준이어야 하며 layout shift를 만들지 않습니다.

## Motion

- Motion은 state change, spatial relationship, hierarchy를 설명해야 합니다.
- decorative motion은 reduced-motion path가 있어야 합니다.
- Duration과 easing은 system-wide로 일관되어야 합니다.
- Text readability를 방해하는 continuous motion은 피합니다.

## Icons

- 하나의 icon family를 유지합니다.
- Stroke width, corner style, filled/outline style을 섞지 않습니다.
- Emoji를 UI icon으로 쓰지 않습니다.
- Icon-only button에는 accessible name이 필요합니다.

## Accessibility

- Color만으로 상태를 전달하지 않습니다.
- Focus visible이 hover와 같은 수준으로 설계되어야 합니다.
- Form control에는 label 또는 accessible name이 있어야 합니다.
- Error state에는 `aria-invalid`, `aria-describedby`, `role=alert`, `aria-live` 중 맥락에 맞는 처리가 필요합니다.

## Component Selection

- Form UI는 `Field`, `Label`, `Input` 또는 `Textarea`, `FieldDescription`, `FieldError`의 구조를 먼저 고려합니다.
- Option 선택은 목적에 따라 `Select`, `Combobox`, `Radio Group`, `Checkbox Group`, `Segmented Control` 중 고릅니다.
- 짧게 사라지는 screen-level feedback은 `Toast`, section에 붙는 feedback은 `Inline Alert`, 사용자가 반드시 응답해야 하는 흐름은 `Modal`을 씁니다.
- Navigation은 `Navbar`, `Sidebar`, `Breadcrumbs`, `Tabs`, `Tab Menu`, `Pagination` 중 정보 구조에 맞게 고릅니다.
- 상태 표시에는 `Badge`, `Status Badge`, `Chip`, `Progress`, `Spinner`, `Stepper`를 구분해 씁니다.
- Component를 고른 뒤 default, hover, focus, disabled, loading, empty, error, responsive, reduced-motion state 중 필요한 범위를 기록합니다.

## Token Discipline

- Typography는 display, heading, body, paragraph, ui, numeric, code 같은 역할로 계획합니다.
- Spacing은 layout, section, component scale로 나누고, 같은 scale의 간격끼리 묶습니다.
- Radius는 component personality입니다. card, field, button, chip이 서로 설명 가능한 radius 관계를 가져야 합니다.
- Shadow는 elevation, component state, text legibility를 분리해서 씁니다.
- Raw value는 예외입니다. 예외를 쓰면 run manifest에 이유를 기록합니다.
<!-- OPENDOCK:END id=files:PATTERN_GUIDE.md dock=opendock/design-ultrawork path=PATTERN_GUIDE.md -->
