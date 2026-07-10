# Jimin OS Design Contract

Jimin OS의 제품 화면은 개인 데이터와 자동화 상태를 빠르게 판단하는 도구다. 장식보다 상태의 명료함, 높은 정보 밀도, 차분한 상호작용을 우선한다.

## Product direction

- App type: personal productivity, operator, developer tool
- Surfaces: macOS desktop shell and responsive mobile client
- Mood: restrained, compact, calm, precise
- Visual reference categories: Linear의 정보 밀도, Raycast의 선명한 상태 표현, GitHub의 행 중심 구조
- Reference assets, 문구, 브랜드 요소를 복사하지 않는다.
- 첫 화면에서는 아직 구현되지 않은 일정, 기억, AI 데이터를 미리 노출하지 않는다.

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

- App header: product name, current scope, one primary refresh action
- Connection summary: one dominant status statement with supporting recovery copy
- Status list: static rows with semantic icon, label, state text, and optional detail
- Metadata list: build and schema values aligned in compact rows
- Inline notice: bounded status-specific explanation with an accessible live region
- Skeleton: matches the final row structure and appears after the request begins
- Buttons: one filled primary action and quiet icon action where space is constrained

## State coverage

- Default: last confirmed server state is readable without animation.
- Loading: content-shaped skeletons and `aria-busy`; the refresh action is disabled.
- Ready: neutral detail rows and one accent summary marker.
- Needs attention: warning summary plus exact failed dependency rows.
- Unreachable: destructive summary with a clear next action.
- Hover and focus: matched feedback on every interactive element.
- Disabled: reduced emphasis with readable label.
- Responsive: 360px through desktop widths without horizontal overflow.
- Reduced motion: no shimmer or translated entrance.

## Do

- Lead with the state and next useful action.
- Prefer rows, dividers, and grouped surfaces over a grid of equal cards.
- Keep diagnostics user-readable; raw codes belong in developer logs.
- Use one outline icon family with `currentColor`.
- Show only data returned by the server.

## Don't

- Do not use a KPI-card dashboard for basic connection status.
- Do not add gradients, glass effects, decorative blobs, or emoji icons.
- Do not use generic pale icon chips on every row.
- Do not invent schedules, memories, account state, or AI activity.
- Do not expose tokens, internal routes, stack traces, or database terminology.
- Do not hide recovery actions behind hover or a secondary panel.
