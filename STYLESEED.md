# Jimin OS StyleSeed Lock

## Scope

- App type: private personal assistant and daily operating system
- Current surface: daily home, assistant conversations, and contextual action results
- Next surface: calendar, memory, connected-data review, and personal settings
- Platforms: private web, macOS companion, and phone-width mobile client
- Layout type: assistant OS with a desktop sidebar and a compact mobile app shell

## Locked axes

- Key color/accent: Jimin violet #6859d8
- Appearance: follow the operating system; light and dark semantic tokens are defined in DESIGN.md
- Neutral direction: cool lavender-tinted graphite on #f5f6fb canvas
- Radius personality: soft precision; 10px controls, 16px panels, 24px sheets
- Shadow language: low-opacity panel elevation with hairline borders
- Motion style: silk-snap; 160ms, 180ms, and 240ms families
- Type direction: Pretendard Variable, calm sans hierarchy, 600 maximum weight
- Density: comfortable overview, compact rows, touch-safe mobile
- Icon language: one Lucide outline family, 2px stroke, `currentColor`

## Hierarchy lock

1. First gaze: the real daily briefing or active assistant request
2. Primary action: 비서에게 도움 요청하기
3. Secondary scan: next schedule, open tasks, and actual connected-source state
4. Supporting detail: source, time, and action outcome only when returned by the server

The home is a useful daily overview, not a KPI dashboard. It may show schedules,
tasks, and connected-source availability only when the server owns that data. The
assistant remains one action away from every surface and never pretends that an
unconnected provider supplied information.

## Component lock

- One responsive app shell: desktop sidebar and mobile bottom navigation
- One command-style assistant entry and a compact persistent assistant rail on desktop
- One daily briefing focal panel, with asymmetric schedule and task panels underneath
- One source-availability panel only when a provider is not connected
- One inline progress/approval/recovery message when needed
- Content-shaped loading skeletons and source-specific empty states
- Conversation list rows and a chronological message stream within the assistant surface
- Bottom sheets for mobile contextual detail and approvals

Charts, carousels, invented activity feeds, decorative source chips, and fake tool
timelines remain outside the product surface.

## Accessibility lock

- WCAG AA contrast target
- 44px minimum interactive target
- visible 2px focus outline
- icon plus text for every state
- live announcement for refreshed status
- reduced-motion path for every transition and shimmer
- no horizontal overflow at 360px

## StyleSeed adaptations

StyleSeed의 one accent, one radius personality, one shadow language, 8px grid,
visible focus, 44px touch, and state coverage rules are applied. The supplied
reference informs the assistant OS hierarchy, layered panels, contextual sheets,
and motion purpose, but its exact copy, colors, fabricated data, and assets are
not reused.

## Prohibitions

- pure black UI color
- unlocked generic indigo accent
- full-screen decorative gradient
- repeated rounded icon chip
- equal-sized card grid
- emoji UI icon
- decorative status color
- placeholder or fabricated product data
