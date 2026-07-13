<!-- OPENDOCK:START id=files:INTERACTION_PLAYBOOK.md dock=opendock/interactive-ui-ultrawork path=INTERACTION_PLAYBOOK.md -->
# Interaction Playbook

## 1. Interaction 계약

구현 전에 다음을 한 문장씩 고정합니다.

- **Trigger**: 사용자가 무엇을 하면 시작되는가.
- **Feedback**: 100ms 안에 무엇이 변해 입력이 접수됐음을 알리는가.
- **Primary Completion**: 성공 또는 종료를 확인하는 관찰 가능한 조건은 무엇인가.
- **Recovery Path**: 실패, 취소, 재시도 후 안정 상태로 돌아가는 경로는 무엇인가.
- **Focus Contract**: 시작 전, 동작 중, 완료 또는 recovery 후 focus는 어디에 있는가.

이 세 계약은 run manifest의 첫 `##` section보다 앞에 있는 동명의 top-level field에 구체적으로 기록합니다.

Interaction은 시각 효과가 아니라 상태 전이입니다. Idle에서 시작해 hover/focus/pressed, loading, success/error, disabled, reduced-motion 분기를 먼저 모델링합니다.

## 2. 구현 계층 선택

### CSS 우선

Hover/focus/pressed feedback, opacity/transform 전환, disclosure의 단순 open/close처럼 DOM 수명과 순서 제어가 단순할 때 사용합니다. `transition-all` 대신 `transition-property`를 제한하고 `prefers-reduced-motion: reduce`에서 duration을 제거하거나 즉시 상태로 전환합니다.

### WAAPI

Element 단위 keyframe, imperative cancel/reverse, 짧은 sequence가 필요하지만 별도 상태 라이브러리가 필요하지 않을 때 사용합니다. `Animation` reference를 보관하고 cancel 시 정리합니다. Runtime reduced-motion 조건에서는 animation을 만들지 않거나 finish state를 즉시 적용합니다.

### Motion

React에서 mount/unmount orchestration, layout animation, shared layout, gesture와 상태가 결합된 복합 interaction에만 명시적으로 선택합니다. 프로젝트에 이미 설치된 Motion을 우선하며 dock 또는 agent가 자동으로 설치하지 않습니다. 단순 hover/opacity 전환에 Motion을 추가하지 않습니다.

### 특수 timeline/SVG

다중 scene timeline, scroll-linked choreography, path morphing처럼 CSS/WAAPI/Motion으로 복잡도가 더 커지는 경우에만 선택합니다. Run manifest에 대안, bundle/cleanup 비용, reduced-motion fallback, 사용자 승인을 기록합니다. 라이브러리 자동 설치는 금지합니다.

## 3. 입력 Parity

- Native `button`, `a`, `input`, `details`를 우선합니다.
- Custom control은 role, accessible name, focusability, Enter/Space 또는 표준 key contract를 모두 제공합니다.
- Mouse-only event보다 Pointer Events를 사용하고 touch에서 hover가 없음을 전제로 합니다.
- Hover는 보조 신호입니다. 동일한 정보와 action이 focus, press, persistent UI 중 하나로 제공되어야 합니다.
- Modal, menu, popover는 open focus, keyboard navigation, Escape, close 후 focus restoration을 검증합니다.
- Drag/reorder에는 keyboard 대체 경로와 screen reader용 상태 전달을 제공합니다.

## 4. 상태와 비동기 Action

- **Loading**: 진행 중임을 label, progress, skeleton 중 적합한 방식으로 전달합니다.
- **Error**: 실패 원인과 recovery action을 사용자 맥락 가까이에 둡니다.
- **Disabled**: 중복 실행을 막되 이유와 다음 행동을 잃지 않게 합니다.
- **Empty**: 데이터가 없는 상태와 로딩 실패를 구분합니다.
- **Success**: 완료 feedback을 제공하고 focus 또는 다음 action을 예측 가능하게 유지합니다.

Async action은 loading/error/disabled를 함께 설계합니다. Visual color만으로 상태를 구분하지 않습니다.

## 5. Motion 기준

- Transform과 opacity를 우선하고 layout-thrashing property animation을 피합니다.
- Duration과 easing은 interaction 목적별로 제한된 token을 사용합니다.
- 연속 입력에서 animation이 누적되지 않도록 cancel, replace, debounce 정책을 정합니다.
- Reduced motion에서는 핵심 상태 변화는 유지하고 이동·반복·parallax를 제거합니다.
- Loading animation은 `prefers-reduced-motion`에서도 상태를 인지할 수 있는 정적 label을 제공합니다.

## 6. Cleanup

- `setTimeout`은 `clearTimeout`, `setInterval`은 `clearInterval`과 짝을 이룹니다.
- `requestAnimationFrame`은 `cancelAnimationFrame`으로 종료합니다.
- `addEventListener`는 동일 handler와 option으로 제거하거나 `AbortController` signal을 사용합니다.
- WAAPI animation, observer, subscription, media query listener도 unmount/cancel에서 해제합니다.
- React effect cleanup과 route 전환 후 state update 가능성을 검증합니다.

## 7. Responsive와 Overflow

- `100vw`는 scrollbar 폭을 포함할 수 있으므로 page content 폭에 기본 사용하지 않습니다.
- Fixed/min width는 작은 viewport에서 wrap, shrink, scroll container 중 의도한 전략을 명시합니다.
- Drawer, popover, tooltip은 viewport edge collision과 virtual keyboard를 검증합니다.
- 320px 폭, 200% zoom, 긴 번역 문자열, dynamic loading label에서 horizontal overflow를 확인합니다.

## 8. Validation Evidence

최소 evidence는 다음과 같습니다.

- Keyboard-only flow와 visible focus
- Touch 또는 Pointer Events 경로
- Reduced-motion branch
- Loading, error, disabled 또는 구체적인 non-applicable 이유
- Unmount/cancel cleanup
- Mobile viewport horizontal overflow
- 실행한 자동 test/lint/build 명령과 결과

Evidence는 계획이 아니라 관찰 결과입니다. `검증 예정`, `N/A`, `문제 없음`만 적지 말고 viewport, 입력, command, 관찰된 결과를 기록합니다.
<!-- OPENDOCK:END id=files:INTERACTION_PLAYBOOK.md dock=opendock/interactive-ui-ultrawork path=INTERACTION_PLAYBOOK.md -->
