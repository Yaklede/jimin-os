<!-- OPENDOCK:START id=files:CREATE_UI_PLAYBOOK.md dock=opendock/design-ultrawork path=CREATE_UI_PLAYBOOK.md -->
# Create UI Playbook

Create UI는 코드 복사 대상이 아니라 UI 판단 기준입니다. `DESIGN.md`가 최우선이고, 이 문서는 컴포넌트 선택, token 사용, 상태/접근성 점검을 더 촘촘하게 만드는 보조 기준입니다.

## 적용 순서

1. `DESIGN.md`에서 브랜드와 금지사항을 확인합니다.
2. 화면 유형과 section 구조는 `LAYOUT_PLAYBOOK.md`에서 정합니다.
3. 색상 역할은 `COLOR_PLAYBOOK.md`에서 정합니다.
4. 이 문서에서 component inventory, typography token, spacing token, radius token, shadow token을 고릅니다.
5. 선택 결과를 `.opendock/runs/design/<run-id>/manifest.md`에 기록합니다.

## Foundation 기준

### Typography

- Raw `font-size`, `leading`, `tracking`, `font-weight`를 따로 조합하지 말고 역할 기반 token을 먼저 고릅니다.
- 권장 role: `display`, `heading`, `body`, `paragraph`, `ui`, `numeric`, `code`.
- 큰 display/heading은 반응형으로 줄어들 수 있지만, control label, numeric, code는 크기가 흔들리지 않게 고정합니다.
- `text-[48px] leading-[1.3] tracking-[-1.6px] font-medium` 같은 조합은 token이 없는 예외일 때만 허용합니다.
- Type token에는 color token을 따로 붙입니다. 예: title은 strongest, body는 body, hint는 placeholder처럼 역할로 나눕니다.

### Spacing

- 간격은 `layout`, `section`, `component` 세 단계로 나눕니다.
- `layout`: page region, section 간격, page gutter.
- `section`: section 내부의 heading/body/card stack.
- `component`: button, chip, field, tab, menu item 내부 padding과 gap.
- `gap-7`, `p-[19px]`처럼 일회성 수치를 늘리지 말고 어떤 scale에 속하는지 먼저 정합니다.

### Radius

- Radius는 component personality입니다. 한 화면에서 여러 radius personality를 섞지 않습니다.
- `none`, `sm`, `md`, `lg`, `xl`, `2xl`, `3xl`, `full` 중 역할을 정합니다.
- Card, modal, popover, field, button, chip의 radius 관계가 설명 가능해야 합니다.
- Pill은 의도가 있을 때만 씁니다. 모든 CTA를 습관적으로 pill로 만들지 않습니다.

### Colors

- Raw primitive color보다 semantic role을 먼저 씁니다.
- 기본 role: canvas, surface, text, muted text, border, primary, secondary, focus, success, warning, danger.
- 상태 색은 장식 색과 섞지 않습니다. Error는 error, warning은 warning, success는 success로 읽혀야 합니다.
- Light/dark mode는 token이 처리해야 하며, 수동 `dark:bg-*` 반복은 예외로 봅니다.

### Shadows

- Shadow는 elevation, component state, text legibility를 분리합니다.
- `shadow-neutral-*`: card, popover, dropdown, modal 같은 surface elevation.
- `shadow-component-*`: focus ring, inset border, hover outline 같은 control state.
- `text-shadow-*`: 사진/색상 위 text legibility가 필요할 때만.
- 한 화면에서는 one shadow language를 유지하고, raw multi-layer shadow를 직접 쓰는 패턴은 피합니다.

## Component Decision Inventory

UI 작업 전에 필요한 component를 이 목록에서 고르고, 선택 이유를 manifest에 기록합니다.

### Action

- `Button`: 명확한 command, submit, primary/secondary action.
- `Button Group`: 같은 수준의 action이 한 덩어리로 묶일 때.
- `Close Button`: dialog, toast, sheet, popover dismiss.
- `FAB Button`: 모바일 또는 canvas 중심 화면의 떠 있는 핵심 action. 남용 금지.
- `Text Link`: navigation 또는 inline action. Button처럼 보이게 만들지 않습니다.
- `Social Login Button`, `App Store Badge`: 외부 provider/store action일 때만.

### Form

- `Field`: label, description, error, disabled, invalid, loading 상태를 묶는 기본 단위.
- `Label`: placeholder를 label로 대체하지 않습니다.
- `Input`: short text, email, search, phone, URL, date 등 한 줄 입력.
- `Textarea`: multi-line 입력.
- `Input Group`: input과 button/select/icon affordance가 한 줄에 결합될 때.
- `Input OTP`: 자리수가 정해진 verification code.
- `Input Tag`: tag/chip 입력.
- `Input Stepper`, `Slider`: numeric adjustment. 단순 숫자 입력을 `type="number"`만으로 방치하지 않습니다.
- `Select`, `Combobox`: option 선택. 검색/필터가 필요하면 Combobox를 고려합니다.
- `Checkbox`, `Checkbox Group`: 복수 선택 또는 동의.
- `Radio`, `Radio Group`: 상호 배타적 선택.
- `Switch`, `Switch Group`: 즉시 켜고 끄는 설정. 제출형 선택에는 checkbox가 더 맞을 수 있습니다.
- `Password Strength`, `File Upload`, `Dropzone`, `File Format`: 입력 보조와 validation feedback이 필요한 specialized form.

### Navigation

- `Navbar`: site/app 상단 구조. nav item 수, CTA, account/search/cart 여부를 명확히 합니다.
- `Breadcrumbs`: hierarchy가 깊은 page에서 현재 위치를 설명합니다.
- `Pagination`: 대량 목록의 page 이동.
- `Tabs`, `Tab Menu`, `Segmented Control`: 같은 맥락 안의 view 전환. 서로 다른 destination navigation과 혼동하지 않습니다.
- `Sidebar`: 작업형 app, dashboard, admin에서 반복 navigation이 필요할 때.
- `Command`: command palette, shortcut-driven navigation.

### Overlay

- `Modal`: 사용자가 응답해야 다음으로 갈 수 있는 blocking task.
- `Popover`: 가벼운 supplemental panel.
- `Dropdown Menu`, `Context Menu`: action menu. Form option 선택에는 Select/Combobox를 우선합니다.
- `Tooltip`, `Info Tooltip`: 짧은 보조 설명. 필수 정보를 tooltip에 숨기지 않습니다.

### Feedback

- `Inline Alert`: 특정 section에 붙은 상태, 경고, 성공, 오류.
- `Alert Banner`: page 또는 product-wide announcement.
- `Toast`: 짧게 사라지는 screen-level status. 실패 해결이 필요하면 Inline Alert나 Modal을 고려합니다.
- `Progress`, `Spinner`, `Stepper`: 작업 진행과 단계 상태.
- `Status Badge`, `Badge`, `Chip`: 상태/분류/선택된 filter를 작게 표시합니다.
- `Rating`: 평가 입력 또는 표시가 핵심일 때.

### Display / Utility

- `Avatar`, `Avatar Group`: 사람, 팀, 계정 표현.
- `Accordion`: 긴 내용을 접어 탐색할 때. 핵심 정보 숨김에는 부적합합니다.
- `Aspect Ratio`: image/video/card media ratio 고정.
- `Scroll Area`: 내부 scroll이 실제로 필요한 작은 영역에만.
- `Separator`: section 또는 menu grouping을 보조합니다.

## State Coverage

선택한 component마다 관련 state를 확인합니다.

- Default
- Hover
- Focus / focus-visible
- Active / pressed
- Disabled
- Loading / pending
- Empty
- Error / invalid
- Success / completed
- Responsive / mobile
- Reduced motion

모든 component가 모든 state를 필요로 하지는 않습니다. 다만 누락한 state는 “해당 없음”으로 설명 가능해야 합니다.

## Accessibility Rules

- Form control은 label, `aria-label`, `aria-labelledby` 중 하나로 accessible name을 가집니다.
- Error는 `aria-invalid`, `aria-describedby`, `role="alert"`, `aria-live` 중 맥락에 맞게 연결합니다.
- Toast는 짧은 상태 알림이고, 중요한 경고는 role과 dismiss/undo 동작을 재검토합니다.
- Icon-only button에는 accessible name이 필요합니다.
- Hover affordance에는 keyboard focus affordance가 대응되어야 합니다.
- 최소 touch target은 44px를 목표로 합니다.

## Manifest 기록 예시

```md
Component Inventory: Button, Field, Input, Inline Alert, Toast, Tabs
Typography Token Plan: heading for section titles, paragraph for body copy, ui for controls, numeric for metrics
Spacing Token Plan: layout-md for page regions, section-sm for card stacks, component-sm for button/input internals
Radius Token Plan: component-lg for cards and fields, component-full only for small chips
Shadow Token Plan: neutral-sm for cards, component-focus for controls, no decorative raw shadows
State Coverage: Button hover/focus/disabled/loading, Field invalid/error, Toast dismissible, Tabs selected/focus
```

## 금지 패턴

- 공개 문서의 component 코드를 그대로 복사하지 않습니다.
- Pro/private code, screenshot, exact text, brand asset을 결과물에 포함하지 않습니다.
- Component를 “예뻐 보이는 모양”으로만 선택하지 않습니다. 역할과 state를 먼저 설명합니다.
- Raw color, arbitrary text size, random radius, custom shadow를 여러 곳에 흩뿌리지 않습니다.
<!-- OPENDOCK:END id=files:CREATE_UI_PLAYBOOK.md dock=opendock/design-ultrawork path=CREATE_UI_PLAYBOOK.md -->
