# Jimin OS Design Contract

Jimin OS의 제품 화면은 개인 데이터와 연결 서비스를 바탕으로 사용자의 일을
정리하고 실행을 돕는 AI 비서다. 장식보다 현재 요청의 명료함, 실제 진행 상태,
높은 정보 밀도, 차분한 상호작용을 우선한다.

## Product direction

- App type: personal AI assistant, productivity, operator, developer tool
- Surfaces: private web client, macOS companion shell, and responsive mobile client
- Mood: airy, calm, personal, precise
- Visual reference categories: 개인 비서 OS의 맥락 우선 구조, 명령 팔레트의 직접성, 실제 데이터의 작업 밀도
- Reference assets, 문구, 브랜드 요소를 복사하지 않는다.
- 첫 화면은 서버가 실제로 반환한 일정·할 일·연결 상태만으로 하루의 맥락을
  정리한다. 아직 연결하지 않은 서비스는 그 사실과 연결 행동만 보여 준다.

## Typography

- Primary family: Pretendard Variable
- Fallback: Pretendard, Apple SD Gothic Neo, system-ui, sans-serif
- Type sizes: 12px, 14px, 16px, 20px, 24px, 32px
- Line heights: 1.25, 1.4, 1.5, 1.6
- Weights: 400, 500, 600. The weight ceiling is 600.
- Body text is 16px on input and primary reading surfaces, 14px for dense supporting rows.
- Headings use sentence case and weight before size for hierarchy.
- Numeric and build metadata use tabular numerals.
- Body copy does not use negative letter-spacing. Labels use 0.04em only when short and functional.

## Color roles

- Canvas: #f5f6fb
- Surface: #ffffff
- Surface subtle: #f0f1f8
- Text strong: #1e2030
- Text primary: #37394a
- Text muted: #6d7080
- Text faint: #9295a3
- Border: #e1e2ec
- Border strong: #c9cbe0
- Accent: #6859d8
- Accent hover: #5547c4
- Accent tint: #efedff
- Focus: #5e50cd
- Success: #278a53
- Warning: #a5702c
- Warning tint: #faf1e5
- Destructive: #b25058
- Destructive hover: #963f47
- Destructive tint: #fbedee
- Disabled surface: #e9eaf1
- Disabled text: #858895
- Overlay text: #f7f7ff

## System appearance

- The app follows the operating system's light or dark appearance. It does not
  expose an in-app theme override in this phase.
- Native Android status and navigation bars use the matching canvas color with
  legible system icons. Mobile content must respect the top and bottom system
  insets.

### Dark color roles

- Canvas: #171720
- Surface: #22232e
- Surface subtle: #2a2b38
- Text strong: #f4f3ff
- Text primary: #deddea
- Text muted: #aaaab9
- Text faint: #858695
- Border: #3b3c4d
- Border strong: #555669
- Accent: #a99dff
- Accent hover: #b8afff
- Accent tint: #302d55
- Focus: #c4bcff
- Success: #68c98e
- Warning: #e2bb7d
- Warning tint: #453826
- Destructive: #e69ba1
- Destructive hover: #efafb4
- Destructive tint: #4b2b32
- Disabled surface: #32333f
- Disabled text: #9899a7
- Overlay text: #171720

The dark scheme preserves the same role hierarchy and violet accent family. It
does not introduce a second decorative accent, pure black canvas, or a
light-only status treatment.

The canvas uses a cool lavender-tinted neutral. White is reserved for bounded surfaces. The accent appears only on the primary action, focus treatment, selected navigation, and the single current-state marker. Normal detail rows remain neutral; warning and destructive colors encode real exceptions only.

## Contrast plan

- Body and heading text target WCAG AA at 4.5:1 or higher.
- Large status text and line icons target 3:1 or higher.
- Primary and destructive action text are checked against their filled backgrounds.
- Muted and disabled copy remains readable and never carries essential meaning alone.
- Every focusable control uses a 2px visible outline with 2px offset.
- Status combines icon, label, and text instead of relying on color alone.

## Spacing and layout

- Base grid: 4px and 8px.
- Allowed spacing steps: 4px, 8px, 12px, 16px, 20px, 24px, 32px, 40px, 48px, 64px.
- Desktop content max width: 1040px.
- Reading measure max width: 680px.
- Header height: 64px.
- Default touch target and button height: 44px.
- Input height: 48px.
- Dense status row minimum height: 56px.
- Mobile gutter: 16px. Desktop gutter: 24px.
- Desktop diagnostic grid uses a wider status column and a narrower metadata column.
- Mobile collapses to one column without horizontal scrolling.
- Content aligns to a small set of shared vertical edges.

## Radius personality

The radius personality is softly precise. Controls use 10px, bounded panels use 16px, and mobile sheets use 24px. Status dots remain circular. Large pill-shaped CTAs are outside the product language.

## Border and shadow language

- Default separation uses a 1px hairline border, whitespace, and a single low-opacity elevation family.
- Primary panels may sit one quiet elevation above the canvas; lists and normal detail rows stay flat.
- Menus and dialogs use the same elevation family instead of a second floating treatment.
- Focus elevation comes from the focus outline, not a colored glow.

## Motion

- Motion style: silk-snap.
- Durations: 160ms for color and control feedback, 180ms for content changes, 240ms for sheets and overlays.
- Product surfaces animate opacity and short spatial transitions only.
- Loading shimmer and live microphone feedback are the only repeating animations; microphone feedback requires a real recording state.
- `prefers-reduced-motion` removes shimmer and shortens transitions to 0.01ms.

## Components

- App chrome: product name, current scope, responsive navigation, and one contextual assistant action
- Daily home: briefing, next event, open tasks, and connected-source availability from real server data
- Assistant composer: one dominant request surface with clear submit, optional attachments,
  and only the connected sources that the server can actually use
- Account connection gate: a compact, factual ChatGPT sign-in state shown only while the
  managed agent cannot answer; it presents the official link and device code without turning
  the home screen into a settings form
- Active task: a bounded request/result surface; it is the single focal point while work runs
- Progress panel: real server-emitted stage, tool, approval, or failure information; never
  simulated thinking text or a fake timer
- Context list: schedule, task, memory, or project rows are shown only when the server returns the related real source
- Conversation stream: chronological user and assistant messages with compact metadata
- Approval panel: explicit action, affected service/data, approve and decline choices
- Inline notice: bounded status-specific explanation with an accessible live region
- Skeleton: matches the final row structure and appears after the request begins
- Buttons: one filled primary action and quiet secondary/icon actions where space is constrained

## State coverage

- Empty: a clear source-specific explanation and useful next action; no invented personal data.
- Account connection: a real server state keeps the composer unavailable until the managed
  ChatGPT connection is ready; the one-time code is presented with an explicit open action.
- Loading: content-shaped skeletons and `aria-busy`; the active action is disabled.
- Active request: real server-emitted processing state remains readable while navigating away.
- Ready: result, context rows, and one accent summary marker.
- Approval needed: action scope and the user's choices are visible together.
- Needs attention: warning summary plus exact affected row or connected service.
- Unreachable: destructive summary with a clear next action.
- Hover and focus: matched feedback on every interactive element.
- Disabled: reduced emphasis with readable label.
- Responsive: 360px through desktop widths without horizontal overflow.
- Reduced motion: no shimmer or translated entrance.

## Do

- Lead with the daily briefing or active request and the next useful action.
- Prefer an asymmetric daily overview, rows, dividers, and grouped surfaces over a grid of equal cards.
- Keep diagnostics user-readable; raw codes belong in developer logs.
- Use one outline icon family with `currentColor`.
- Show only data returned by the server.
- Use Apple platform conventions for safe areas, sheets, keyboard focus, and native-feeling
  transitions; do not copy Apple marketing layouts, assets, or copy.
- Use Gemini-like assistant interaction only for explicit source selection, plans, background
  work, and approval; do not copy Gemini's visual identity, gradients, or component layout.

## Don't

- Do not use a KPI-card dashboard for basic connection status or fabricated personal assistant context.
- Do not add gradients, glass effects, decorative blobs, or emoji icons.
- Do not use generic pale icon chips on every row.
- Do not invent schedules, memories, account state, AI activity, tool use, or approval state.
- Do not expose tokens, internal routes, stack traces, or database terminology.
- Do not hide recovery actions behind hover or a secondary panel.
