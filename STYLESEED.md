# Jimin OS StyleSeed Lock

## Scope

- App type: personal productivity, operator, developer tool
- Current surface: assistant-first home, active request, and explicit assistant conversations
- Next surface: plan, memory, connected-data review, and device management
- Platforms: private web, macOS companion, and phone-width mobile client
- Layout type: compact assistant work surface, not a marketing page or KPI dashboard

## Locked axes

- Key color/accent: muted pine #287c68
- Appearance: follow the operating system; light and dark semantic tokens are
  defined in DESIGN.md
- Neutral direction: cool green-tinted graphite on #f3f5f2 canvas
- Radius personality: small and precise; 6px controls, 8px surfaces
- Shadow language: flat hairline separation; elevation only for future overlays
- Motion style: snap; 140ms, 180ms, and 240ms families
- Type direction: Pretendard Variable, compact sans hierarchy, 600 maximum weight
- Density: compact desktop, touch-safe mobile
- Icon language: one Lucide outline family, 2px stroke, `currentColor`

## Hierarchy lock

1. First gaze: the active request/result; otherwise the assistant composer
2. Primary action: Jimin OS에 도움 요청하기
3. Secondary scan: recent conversations and actual active work
4. Supporting detail: connected source, time, device, and server status only when the active
   request needs it

The assistant home is a conversation entry point, not a static dashboard. It
offers the composer and a few generic ways to start a request; personal schedule,
task, and saved context appear only when a request has actually called for them.
The conversation view continues the same request rather than switching to an
unrelated chat product. Both views use the same accent, radius, icon, and border
language.

Only the active request/result and primary action use the accent. Normal
context rows remain neutral so warning and destructive exceptions retain meaning.

## Component lock

- One app chrome with responsive navigation
- One dominant assistant composer or active-request surface
- One compact ChatGPT account connection gate when the managed runtime is not ready
- Context rows only when a real source is selected for the active request
- One inline progress/approval/recovery message when needed
- Content-shaped loading skeletons
- Conversation list rows, a chronological message stream, and a labelled
  request composer within one work surface
- Sidebar on desktop and bottom navigation on mobile once top-level assistant,
  plan, and memory destinations exist

Charts, KPI tiles, carousels, decorative source chips, and fake tool timelines
are outside this surface because they do not help the current request.

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
