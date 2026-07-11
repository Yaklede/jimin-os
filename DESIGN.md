# Jimin OS Design Contract

Jimin OS의 제품 화면은 개인 데이터와 연결 서비스를 바탕으로 사용자의 일을
정리하고 실행을 돕는 AI 비서다. 장식보다 현재 요청의 명료함, 실제 진행 상태,
높은 정보 밀도, 차분한 상호작용을 우선한다.

## Product direction

- App type: personal AI assistant, productivity, operator, developer tool
- Surfaces: private web client, macOS companion shell, and responsive mobile client
- Mood: restrained, compact, calm, precise
- Visual reference categories: Linear의 정보 밀도, Raycast의 선명한 상태 표현, GitHub의 행 중심 구조
- Reference assets, 문구, 브랜드 요소를 복사하지 않는다.
- 첫 화면에서는 아직 구현되지 않은 일정, 기억, AI 도구 사용, 승인 상태를 미리
  노출하지 않는다.

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

- Canvas: #f3f5f2
- Surface: #ffffff
- Surface subtle: #f8f9f7
- Text strong: #1f2623
- Text primary: #34413c
- Text muted: #68756f
- Text faint: #8a958f
- Border: #dce2de
- Border strong: #bcc8c2
- Accent: #287c68
- Accent hover: #216956
- Accent tint: #e4f0ec
- Focus: #1f6f5e
- Warning: #9a6828
- Warning tint: #f6eee2
- Destructive: #ad4949
- Destructive hover: #943d3d
- Destructive tint: #f7e9e8
- Disabled surface: #e7ebe8
- Disabled text: #7a8580
- Overlay text: #f4f7f5

## System appearance

- The app follows the operating system's light or dark appearance. It does not
  expose an in-app theme override in this phase.
- Native Android status and navigation bars use the matching canvas color with
  legible system icons. Mobile content must respect the top and bottom system
  insets.

### Dark color roles

- Canvas: #171c19
- Surface: #202722
- Surface subtle: #272f29
- Text strong: #f0f5f1
- Text primary: #d8e1db
- Text muted: #aab8af
- Text faint: #819087
- Border: #3a463f
- Border strong: #536258
- Accent: #57a78e
- Accent hover: #68b79f
- Accent tint: #203b31
- Focus: #8fd3bb
- Warning: #d9af6d
- Warning tint: #423822
- Destructive: #df8e8e
- Destructive hover: #eca2a2
- Destructive tint: #452a2a
- Disabled surface: #313b34
- Disabled text: #9aa79f
- Overlay text: #112019

The dark scheme preserves the same role hierarchy and pine accent family. It
does not introduce a second decorative accent, pure black canvas, or a
light-only status treatment.

The canvas uses the quiet green-tinted neutral. White is reserved for bounded surfaces. The accent appears only on the primary action, focus treatment, and the single current-state marker. Normal detail rows remain neutral; warning and destructive colors encode real exceptions only.

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

The radius personality is small and precise. Controls use 6px, bounded surfaces and overlays use 8px, and status dots are circular. Large pill-shaped CTAs and oversized rounded cards are outside the product language.

## Border and shadow language

- Default separation uses a 1px hairline border and whitespace.
- Flat product surfaces do not float above the canvas.
- A single low-opacity elevation family is reserved for future menus and dialogs, not status panels.
- Focus elevation comes from the focus outline, not a colored glow.

## Motion

- Motion style: snap.
- Durations: 140ms for color and control feedback, 180ms for small fades, 240ms for overlays.
- Product surfaces animate opacity and transform only.
- Loading shimmer is the only repeating animation.
- `prefers-reduced-motion` removes shimmer and shortens transitions to 0.01ms.

## Components

- App chrome: product name, current scope, navigation, and one contextual action
- Assistant composer: one dominant request surface with clear submit, optional attachments,
  and only the connected sources that the server can actually use
- Active task: a bounded request/result surface; it is the single focal point while work runs
- Progress panel: real server-emitted stage, tool, approval, or failure information; never
  simulated thinking text or a fake timer
- Context list: static schedule, task, memory, or project rows that the assistant can cite
- Conversation stream: chronological user and assistant messages with compact metadata
- Approval panel: explicit action, affected service/data, approve and decline choices
- Inline notice: bounded status-specific explanation with an accessible live region
- Skeleton: matches the final row structure and appears after the request begins
- Buttons: one filled primary action and quiet secondary/icon actions where space is constrained

## State coverage

- Empty: composer and honest next action; no invented personal data.
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

- Lead with the active request and next useful action.
- Prefer rows, dividers, and grouped surfaces over a grid of equal cards.
- Keep diagnostics user-readable; raw codes belong in developer logs.
- Use one outline icon family with `currentColor`.
- Show only data returned by the server.
- Use Apple platform conventions for safe areas, sheets, keyboard focus, and native-feeling
  transitions; do not copy Apple marketing layouts, assets, or copy.
- Use Gemini-like assistant interaction only for explicit source selection, plans, background
  work, and approval; do not copy Gemini's visual identity, gradients, or component layout.

## Don't

- Do not use a KPI-card dashboard for basic connection status or personal assistant context.
- Do not add gradients, glass effects, decorative blobs, or emoji icons.
- Do not use generic pale icon chips on every row.
- Do not invent schedules, memories, account state, AI activity, tool use, or approval state.
- Do not expose tokens, internal routes, stack traces, or database terminology.
- Do not hide recovery actions behind hover or a secondary panel.
