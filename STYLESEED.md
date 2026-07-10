# Jimin OS StyleSeed Lock

## Scope

- App type: personal productivity, operator, developer tool
- Current surface: server connection and readiness diagnostic
- Platforms: macOS desktop-first responsive UI with a phone-width validation view
- Layout type: compact operational console, not a marketing page or KPI dashboard

## Locked axes

- Key color/accent: muted pine #287c68
- Neutral direction: cool green-tinted graphite on #f3f5f2 canvas
- Radius personality: small and precise; 6px controls, 8px surfaces
- Shadow language: flat hairline separation; elevation only for future overlays
- Motion style: snap; 140ms, 180ms, and 240ms families
- Type direction: Pretendard Variable, compact sans hierarchy, 600 maximum weight
- Density: compact desktop, touch-safe mobile
- Icon language: one Lucide outline family, 2px stroke, `currentColor`

## Hierarchy lock

1. First gaze: current server connection state
2. Primary action: 상태 다시 확인하기
3. Secondary scan: process, database, and migration readiness rows
4. Supporting detail: server address, build, schema, last checked time

Only the overall state marker and primary action use the accent. Healthy detail rows remain neutral so warning and destructive exceptions retain meaning.

## Component lock

- One app header
- One connection summary surface
- One readiness row group
- One server information group
- One inline recovery message when needed
- Content-shaped loading skeletons

Inputs, charts, KPI tiles, carousels, sidebars, and bottom navigation are outside this first surface because they do not help the current task.

## Accessibility lock

- WCAG AA contrast target
- 44px minimum interactive target
- visible 2px focus outline
- icon plus text for every state
- live announcement for refreshed status
- reduced-motion path for every transition and shimmer
- no horizontal overflow at 360px

## StyleSeed adaptations

StyleSeed의 one accent, one radius personality, one shadow language, 8px grid, visible focus, 44px touch, state coverage 규칙을 적용한다. 모바일 KPI 카드 레시피와 모든 내용을 카드로 만드는 규칙은 이 화면의 operational-console 목적과 충돌하므로 사용하지 않는다. 대신 하나의 주 표면 안에서 행과 hairline divider로 상태를 구분한다.

## Prohibitions

- pure black UI color
- generic indigo accent
- gradient background
- repeated rounded icon chip
- equal-sized card grid
- emoji UI icon
- decorative status color
- placeholder or fabricated product data
