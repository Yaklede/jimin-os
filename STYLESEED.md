# Jimin OS StyleSeed Lock

## Scope

- App type: private personal assistant and daily operating system
- Current surface: daily home, assistant conversations, and contextual action results
- Next surface: calendar, memory, connected-data review, and personal settings
- Platforms: private web, macOS companion, and phone-width mobile client
- Layout type: assistant OS with a desktop sidebar and a compact mobile app shell

## Locked axes

- Key color/accent: mint #68e0bd dark / #087f65 light, restricted to primary action and selection
- Appearance: local persistent light/dark selection from the palette icon; initial regular-client mode follows the operating system, design-preview default is dark.
- Neutral direction: Figma neutral charcoal/gray; dark canvas #0f0f10, surface #1b1c1e, elevated surface #212225
- Radius personality: soft precision; 10px controls, 16px panels, 24px sheets
- Shadow language: low-opacity panel elevation without card or button perimeter borders
- Motion style: silk-snap; 160ms, 180ms, and 240ms families
- Type direction: Wanted Sans Variable, calm sans hierarchy, 600 maximum weight
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

- Hover: neutral controls use a dedicated surface-hover token (#303236 dark, #e6e7eb light), brighter than dark elevated surfaces. Primary and danger actions retain their semantic color.

- Informational summary panel uses semantic mint (a mint mix of the accent and neutral surface) while mint remains the action and selection accent. Shared outer/panel spacing is 24px desktop, 16px narrow.

- Latest user reference: layered charcoal canvas/cards (#202226 / #2b2e34 / #353a40), brighter mint #8dead9; neutral hover #484d57. Adapt color roles only; retain current layout and assets.

- Current user-approved trial: soft yellow #ffdf93 on neutral charcoal, one action accent. Result-detail selection uses row background/arrow rather than check icon; named detail area is the visual priority.

- User-approved 2026-10-05: selectable yellow/mint/blue/purple/pink primary families with one active accent and mode-specific AA contrast. Green completion remains a semantic status color.

- User-approved light-mode polish2026-10-05: vivid primary fills with separate readable label/focus roles; static borders removed, focus indicators retained. Dark palette unchanged.

- User-approved2026-10-05 aligned light/dark surfaces: shared1280px content grid,20px desktop/16px mobile gutter,16px header/calendar outer corners,8px desktop/4px phone date gaps. Existing compact typography,10px controls and circular selected dates retained.


## Flex-reference calendar adaptation — 2026-10-05
The user requested the public flex time-tracking calendar as a visual reference. Weekly calendars now show Monday–Sunday columns and a local-clock time axis. Actual schedules use their duration, midnight-to-midnight events and tasks with a due date occupy the all-day strip, and home requests appear at their received timestamp. Request blocks label receipt time, never a fabricated work duration. Overlaps use lanes; mobile scrolls only the labeled calendar region. Day view remains unchanged.
Light neutrals use canvas #f5f6f8, white cards, subtle #eef1f4, stronger header/control fill #e4e9ee, strong text #24292e and muted text #596875. Inputs retain distinct neutral fill without perimeter borders. Selected light calendar dates use black #24292e circles and white labels. Calendar categories use mint/manual, blue/connected, purple/received request and amber/task fills with readable labels. These encode entry kinds independently of the saved primary color. The selectable light mint accent is #7affdc with charcoal text; other saved primary choices and dark surfaces remain. Existing shared edges, soft radii and compact typography are retained.


## Calendar accent consistency and visible period navigation — 2026-10-05
User corrected the fixed category colors: calendar content must follow the selected primary color in both modes. Month task/schedule/report labels and week all-day/timed blocks share calendar-entry-fill (20% active accent mixed with the current card surface) and calendar-entry-text (the AA-tested active accent label). This supersedes the earlier fixed mint/blue/purple/amber category roles; dates, times, source metadata and all-day placement still convey item meaning. Light selected date circles remain charcoal/white as requested.
Schedule and home period navigation group the two44px arrows and current label into one borderless control-contrast surface with4px inset/gap,10px control corners and centered readable labels. Mobile navigation retains its full-width responsive row. Existing focus/loading/navigation behavior and date callbacks remain.

## Current light-mode revision — 2026-10-05

User approved Toss Business as a reference for neutral hierarchy and compact badges.
This section supersedes earlier light palette and connection-badge revisions.
Canvas #f4f5f7, white surfaces, fields/subtle #f2f4f6, hover #e8ebed, control #e9edf1,
strong text #191f28, body #333d4b and supporting #626d7b. Light cards are flat,
borderless; menus retain quiet elevation and focus stays visible.
Primary actions, selected navigation, request highlights and calendar entries follow
the selected accent. New light preferences default blue; saved choices and dark defaults remain.
Fixed semantic badges: success #14734b on #e8f7ef, attention #946000 on #fff4df,
danger #b3233b on #fff0f2, disconnected #4e5968 on #f2f4f6.
These semantic colors and neutral surfaces never follow the primary selector.
Badges use12px/20px semibold text,6px corners and compact padding without decorative dots.
Clickable refresh labels preserve44px hit areas. Assignee identity and charcoal selected
calendar circles remain fixed. Existing layout, data flow and responsive behavior retained.

User light-blue revision2026-10-06: primary#4594FB, hover#3287EE, action text#191f28. Existing darker standalone label/focus blue keeps AA; dark palette unchanged.

### Light blue label correction — 2026-10-06
User explicitly overrides previous darker standalone blue/charcoal action text: light blue accent, standalone labels and focus use#4594FB; solid primary text/icons use white, hover#3287EE retained. This is an approved contrast exception: white/base3.05:1 meets icon/large-text3:1, below4.5:1 for normal small text; accent labels on tinted surfaces also below normal-text AA. Other palettes and semantic statuses unchanged. All earlier contrary light-blue rules are superseded.
# Dark status visibility — 2026-10-06

User requests legible dark badges and primary states on every page. Shared dark
status fills/labels: attention#ffe3a3/#664600, success#a1edc5/#174d35,
danger#ffc0cc/#82243d, neutral#d3d9e2/#333d4b. These fixed semantic colors apply
across decisions, tasks/due labels, connections and meetings; connection refresh
hover retains its semantic family. Dark selected tint follows the active primary
at20% on the surface, and faint metadata uses#aeb4be. Preserve all primary
palettes, layout/typography, light mode and existing alert surfaces.

## Responsive command header — 2026-10-06
User-approved original arrangement: bounded question launcher at left, independent
refresh and primary-color icons at right, transparent wrapper in both modes.
Desktop date is a plain label between launcher and icons. At720px and below the
date moves to a separate second row; the48px launcher remains visible beside two
44px icon targets. Existing neutral tokens,16px launcher radius,10px icon corners,
compact type and keyboard behavior retained. This supersedes the earlier mobile
date-only card.
