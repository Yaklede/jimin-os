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

- Primary family: Wanted Sans Variable
- Fallback: Wanted Sans, Apple SD Gothic Neo, system-ui, sans-serif
- Type sizes: 12px, 14px, 16px, 20px, 24px, 32px
- Line heights: integer pixel values matched to each text role; 18px, 20px, 24px, 28px, 32px, 40px
- Weights: 400, 500, 600. The weight ceiling is 600.
- Body text is 16px on input and primary reading surfaces, 14px for dense supporting rows.
- Headings use sentence case and weight before size for hierarchy.
- Numeric and build metadata use tabular numerals.
- Body copy does not use negative letter-spacing. Labels use 0px letter spacing.

## Color roles

- Canvas: #f7f7f8
- Surface: #ffffff
- Surface subtle: #f0f0f2
- Text strong: #171719
- Text primary: #2e2f33
- Text muted: #70737c
- Text faint: #878a93
- Border: #eaebec
- Border strong: #e1e2e4
- Accent: #087f65
- On accent: #ffffff
- Accent hover: #06664f
- Accent tint: #f0f0f2
- Focus: #087f65
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

- The palette icon beside refresh opens light/dark appearance and primary-color controls. Explicit choices are saved locally and applied before rendering. Without saved choices, the design preview starts dark and regular clients use the operating system appearance at launch.
- The appearance panel sizes to its contents instead of distributing excess height across rows. Mode controls remain 44px and color controls 68px; short viewports scroll inside the panel.
- Representative images can be selected or restored in this panel. PNG/JPEG/WebP inputs are resized locally and saved only on the current device; no server upload or cross-device sync is implied.
- Desktop home inflow grids use a zero-minimum track and viewport-bounded height so expanded assignment forms scroll inside the detail column. Mobile details retain natural page scrolling; no assignment field or submission action may be clipped by the outer card.
- Native Android status and navigation bars use the matching canvas color with
  legible system icons. Mobile content must respect the top and bottom system
  insets.

### Dark color roles

- Canvas: #0f0f10
- Surface: #1b1c1e
- Surface subtle: #212225
- Text strong: #f7f7f8
- Text primary: #c2c4c8
- Text muted: #aeb0b6
- Text faint: #878a93
- Border: #333438
- Border strong: #37383c
- Accent: #68e0bd
- On accent: #0f0f10
- Accent hover: #8cebcf
- Accent tint: #212225
- Focus: #a2f2da
- Success: #68c98e
- Warning: #e2bb7d
- Warning tint: #453826
- Destructive: #e69ba1
- Destructive hover: #efafb4
- Destructive tint: #4b2b32
- Disabled surface: #32333f
- Disabled text: #9899a7
- Overlay text: #0f0f10

The dark scheme preserves the same role hierarchy and mint accent family. It
does not introduce a second decorative accent, pure black canvas, or a
light-only status treatment.

The canvas uses a neutral charcoal and gray. White is reserved for bounded surfaces. The accent appears only on the primary action, focus treatment, selected navigation, and the single current-state marker. Normal detail rows remain neutral; warning and destructive colors encode real exceptions only.

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

- Default separation uses whitespace and a single low-opacity elevation family. Cards and buttons have no perimeter borders; row dividers and keyboard focus indicators remain.
- Primary panels may sit one quiet elevation above the canvas; lists and normal detail rows stay flat.
- Menus and dialogs use the same elevation family instead of a second floating treatment.
- Focus elevation comes from the focus outline, not a colored glow.

## Motion

- Motion style: silk-snap.
- Durations: 160ms for color and control feedback, 180ms for content changes, 240ms for sheets and overlays.
- Product surfaces animate opacity and short spatial transitions only.
- Loading shimmer and live microphone feedback are the only repeating animations; microphone feedback requires a real recording state.
- `prefers-reduced-motion` removes shimmer and shortens transitions to 0ms.

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

## Local design revision — 2026-09-30

- User approved mint as the only primary accent and Wanted Sans across all product text.
- Font is bundled locally with its OFL license; no CDN request is needed.
- Buttons, badges, panels and text stacks use Flex/Grid automatic layout. SVG is artwork inside a layout container. Inline prose and native inputs retain semantic text flow.
- Authored layout dimensions, line heights, spacing, radii and icon dimensions use integer pixels. Grid ratios use integer fr weights, opacity uses integer percentages, motion durations use integer milliseconds, and spatial animation does not scale component geometry.
- Responsive Flex/Grid distributes space naturally; browsers can calculate subpixel positions at arbitrary viewport sizes. Figma Auto Layout requires a separate Figma import/design pass.
- Only local changes are authorized. Do not push, open a PR, publish or deploy until the user requests completion.

## Figma neutral palette revision — 2026-09-30

Reference: https://www.figma.com/design/uV6J2whJE7Qksv2pm3ogop?node-id=74-20294

- Scope: use the referenced Dark design-system color roles in the existing assistant UI. No swatch/documentation screen or reference imagery is placed in the product.
- Figma Background/Normal/Alternative #0f0f10 maps to canvas; Background/Normal/Normal #1b1c1e maps to surface; Background/Elevated/Normal #212225 maps to raised/subtle surface.
- Figma Label/Normal #f7f7f8 and Label/Neutral #c2c4c8 map to strong and primary text. Alternative #aeb0b6 remains opaque for readable supporting copy.
- Figma Line/Solid/Neutral #333438 and Line/Solid/Normal #37383c map to border and strong border.
- Mint remains the user-requested primary action and selected-state accent; neutral backgrounds and panels contain no green tint. The reference blue primary and Pretendard JP do not override the user's mint and Wanted Sans choices.
- Existing Flex/Grid, integer authored dimensions and local-only delivery remain in effect.

### Interaction surface tokens
- Neutral hover surface: #303236 in dark mode, #e6e7eb in light mode. Dark hover raises luminance above both normal and elevated surfaces; selected backgrounds remain distinct.

### Readable overview and spacing revision
- Informational overview surface uses a mint mix of the accent and neutral surface, distinct from mint actions.
- Page/panel/section spacing is 24px desktop, 16px narrow; page content max width 1280px.
- Command launcher: 56px minimum height, 18px type (52px / 16px narrow). Overview heading 18px and body 16px. Selected task circles use the same accent token.

### Latest palette: portfolio health-app reference
User-selected Figma tGbb3DECVBiSkbHeXl9qyI node 1121:29815 supplies the color direction only. Keep the current assistant interface and assets. Reference context confirms mint #8dead9 and charcoal #353a40; remaining dark tokens adapt the screenshot's layered charcoal surfaces for this app.
- Dark canvas #202226, card #2b2e34, elevated surface #353a40, hover #484d57.
- Mint action #8dead9, hover #acf2e5, focus #bdf7eb; on-mint text #202226.
- Text #d2d5dc, secondary #b8bdc7, dividers #41464f / #4b505a.
- Summary uses a restrained mint tint. Request block and grouped result regions use neutral elevation. Existing integer spacing, Wanted Sans, focus visibility and borderless cards/buttons remain.

### Current yellow palette and result hierarchy
- User reference: Figma tGbb3DECVBiSkbHeXl9qyI node 1173:2959, resolved accent #ffdf93 and neutral #222222. Scope is color/hierarchy, not copying persona artwork.
- Dark canvas #222222, surface #2a2a2a, elevated #333333, hover #414141, divider #3b3b3b / #474747.
- Primary #ffdf93, hover #ffe8b2, focus #fff0cd. Light primary #805b12 / hover #65460b for readable white-button contrast.
- Result lists omit completion-like selection circles. Selected row uses a tinted background and arrow; detail panel reads “선택한 일감” and has a larger title. Group shells have no nested shadows.

### Larger typography and distinguishable interaction states
- Font sizes raised by 2px, compact minimum 14px; pixel line heights expanded accordingly. Navigation 16px/24px, group titles and task titles 18px/26px, secondary summaries 16px/24px.
- Hover uses neutral surface-hover. Selected primary navigation/tabs use 16% yellow tint with accent text; selected secondary lists use a 10% tint. Primary/secondary typography retains hierarchy.

### Approved status and editing refinements — 2026-09-30

These roles record the user's earlier badge, connection, selection and form refinements already implemented in the local preview. They supplement the yellow palette; they are semantic or neutral roles, not additional primary accents.

- Due/attention: background #633840 and label #ffc0cc; hover #784650 / #ffe0e6. Upcoming deadlines use #5b472a / #ffdb85.
- Completion: badge #245b46 / #a1f5cd; checked controls #8ce8b8; selected rows #304b3e; completion action #8ce8b8 / #173b29 with hover #a5f2cc.
- Connected services: #35d493 for the green surface mix, #91f2ba for status text and #48735d for the readable icon tile.
- Assignee labels: default #50545c / #f1f2f5; Kim #75603a / #ffe3a3; Song #365f85 / #c6e8ff. Identity remains explicit in the text.
- Editing: panel #45474c, input #33353a / #f7f8fa, secondary action #62666e, supporting text #e0e3e9. Project edit action #51555c / #f5f6f8; yellow count text #332814.
- Earlier alternatives remain in the stylesheet's revision history and are superseded by the final role rules: error #ff627d / #ff8096, warning #ffb43b / #ffc45c, success #56e5aa, softer labels #ffb3c1 / #ffda91, muted labels #cb798a / #e2bb84, deadline backgrounds #3b292e / #352e25 and labels #ff7f96 / #ffbc32, green surface #214b38, former outline #a18751, neutral alternatives #60636a / #737780. Do not use these historical alternatives for new components.
- Grouped assistant results use one continuous surface-subtle background across the assignee lists and selected detail. Task tabs omit the leading icon; count labels use accent/on-accent when selected and surface-hover/text-strong otherwise.

### Conversation profile and composer
- Assistant messages show the representative image as a 40px circular avatar. The default hamster uses a top crop; custom images use a centered cover crop matching the sidebar profile. Name and message occupy the adjacent Grid column with a 12px gap.
- The user-requested message composer perimeter is an explicit exception to borderless cards: border-strong at rest and focus-colored perimeter when its textarea or send button is focused.
- Textarea caret uses the bright focus token. Empty or whitespace input keeps send disabled in neutral gray; nonblank input immediately enables the fully opaque accent/on-accent send button. Existing loading and waiting guards remain.

### Home task detail and follow-up alignment
- Grouped task details use top label, flexible content row and bottom action row in both assignee/date views. Shared 24px desktop / 16px narrow padding applies at both ends.
- Follow-up heading uses a 28px icon column aligned to the 20px/28px title, with 12px horizontal gap and 8px title-to-description gap. A visible new-request button remains available when results are collapsed; on mobile it moves below the title and retains a 44px touch target. Busy requests disable it without hiding it.
- Clicking the home input displays the send control in fully opaque primary color. Readiness, busy state and nonblank draft still determine whether sending is enabled.

### Project overview readability
- Goal creation stays next to the goal section heading as a section action; use surface-hover/text-strong for its readable secondary button.
- Project list heading and unselected-project guidance omit decorative icons. Guidance sits at the top of its panel with 20px/28px title and 16px/24px description.
- Goal and weekly overview headings omit decorative leading icons.
- Weekly overview metric labels use 18px/28px and values 20px/28px. Preserve the existing six-column desktop and two-column narrow layouts.
- Project list cards use a two-column Grid: title and arrow share the first row, while progress metadata spans the full second row. Arrow aligns to the title center, with symmetric inner padding and no absolute positioning. Omit the standalone open-task count.
- Completion-mode cards show the percentage and a 6px horizontal progress track. The neutral track mixes muted/surface tokens for visibility, and accent fill represents the actual clamped 0–100 progress percentage; 0% has no fill. Expose a named progressbar with numeric value to assistive technology.

### Project management forms and select controls
- Project create/edit management controls stack as label, full-width weekly-report checkbox and full-width native select. Desktop reporting title/help share one row with a 48px card; narrow screens wrap help below the title. Operation-mode threshold remains below the management select.
- Edit footer has only right-aligned Delete and Save actions (삭제 / 저장하기), each at least 48px high. Delete retains the existing confirmation and safe focus restoration; secondary button uses surface-hover/text-strong.
- Native single-value selects share a static muted chevron inset 16px from the right and 48px text-end padding across pages. Native keyboard behavior, labels, focus and multiple/list select behavior remain intact.

### Project master/detail alignment
- On desktop, project list and selected detail share intrinsic Grid rows through subgrid: list title/count centered beside the category tabs, then project cards on the same start line with a 16px gap. No duplicated fixed header height or positional offsets.
- Both project titles start at a 20px card inset. The selected project title precedes its status label. Narrow screens retain the existing single-column list/detail navigation.

### Decision empty-state readability
- Decision section headers use the existing 20px/28px card-title role. Counts use readable text color and 18px/26px.
- All decision empty states omit leading clock icons, retain meaningful title/description text, and align to header padding: 24px desktop, 16px narrow. Empty-state titles use 20px/28px; descriptions use 18px/28px with an 8px gap. Other pages' shared empty-state icon presentation stays unchanged.

### Mobile web layout — 2026-10-02
- At web viewport widths up to 720px, use one full-width column and a fixed bottom navigation. Native desktop shells retain the compact rail at narrow widths; desktop web keeps its existing sidebar.
- Bottom navigation: home, projects, assistant text conversation, schedule, and More. More is a native disclosure containing decisions, meetings, memory and settings; Escape closes it and returns focus to its summary. Route changes close it.
- Navigation targets are at least 48px, compact labels 14px/20px. Use existing surface, accent and focus roles. Mobile content reserves safe-area clearance above the footer; the assistant composer fits above it even in a 320×480 shortened viewport.
- Mobile home results omit redundant inline padding on the nested canvas; tables remain one column. Project categories retain intentional horizontal scrolling within their own strip.
- Mobile text inputs use at least 16px to avoid focus zoom; browser pinch zoom is allowed. Hidden-label home shortcuts retain explicit accessible names.

### Mobile grouped task details — 2026-10-02
- At widths up to 720px, selecting a task opens its shared detail directly beneath that row inside its assignee/date group. Only one task is expanded; selecting it again closes it. Switching grouping resets the open disclosure.
- Desktop keeps its separate right detail column. The same detail component, loader, busy/error handling and edit/complete/open actions are used in either placement, with one rendered instance.
- Mobile disclosure uses a native button with aria-expanded and aria-controls, a down/up chevron and the existing yellow selection tint. Details use surface-subtle, 16px padding/gap and natural height; actions follow content and remain reachable by scrolling/Tab.

### Planning month/week calendars — 2026-10-02
- Month mode displays complete Monday–Sunday calendar rows with adjacent-month dates muted/disabled. Week mode displays exactly the Monday–Sunday range already used by the API. Day mode retains its existing lists.
- Calendar date selection reuses the existing range anchor and range request. Below it, dated tasks and schedules match that day; tasks without a deadline remain visible. Confirmed overnight schedules appear on every day they overlap, excluding an exact midnight ending boundary.
- One primary yellow selection, neutral cell surfaces, existing panel/control radii and no cell borders. Numbers 16px/24px, supporting weekday/event labels 14px/20px. Desktop event titles ellipsize; phone cells use compact task/schedule dots and a text legend. Accessible names provide full date and separate item counts.
- Seven minmax(0,1fr) columns at all sizes. Phone cells have a 64px minimum height (84px week), 12px panel inset; desktop cells 112px with 20px inset. No calendar horizontal scroll.
- Native date buttons use one selected Tab stop, arrow-key focus navigation and Home/End; Enter/Space selects. Selected-date focus restores after the existing range load. Loading, error alert, Today and previous/next actions use the existing request owner; no extra listeners/timers/motion.

- Calendar marker alignment: phone task/schedule dots sit above the date number in a permanent 6px marker slot, including empty and adjacent dates. Calendar cells use flex-start so event content never shifts date numbers vertically; desktop titles stay below numbers.

- Calendar date spacing refinement: user-approved circular date highlight,32px desktop/28px phone. Cells use12px top inset; mobile month cells72px minimum and week84px. The6px marker slot and8px dot-to-number gap stay reserved for every date, preserving row alignment.

- Home task detail refinement: omit redundant selected-task label; show status and priority as separate borderless14px pills. Task heading20px/28px desktop16px/24px phone; phone title stays one line with ellipsis only when necessary (full text retained in heading/title and selected row). Phone metadata14px/20px and notes14px/22px,8px copy spacing; keep actions below content.

### Home incoming-report calendar — 2026-10-02
- Incoming reports have a visible 목록 / 일정 view switch on Home. Calendar defaults to month and also supports week, Today and previous/next period controls. Use the shared schedule calendar rather than a separate date grid.
- Dates represent receivedAt, not task due dates. Show a top marker, aligned circular selected date and compact request count. Date count names include full date and request total for assistive technology. Unknown dates remain in the complete list.
- Before selection the calendar occupies the panel; selecting a date shows its request queue and the existing report detail at the right on desktop, below on mobile. Detail close returns focus to the selected date. Queue scrolls internally with all loaded requests available; the entire page does not grow with each request.
- Existing neutral/yellow tokens, panel/control radii and focus indicators remain; no calendar cell borders, new accent or animation. Home incoming requests remain visible below assistant results even when the assistant is focused.
- Public design preview uses26synthetic requests over7received days, labelled as examples, with no external conversation contents or private links. Production still uses actual Home response data.


### Home calendar placement and compact typography (2026-10-02)
User approved moving the received-request calendar directly below the Home greeting and opening 일정 by default. The original source at HEAD908da1c is viewable separately with synthetic fixtures. Body16px, page30px, section22px, card18px, category14px; larger explicit text roles reduced2px (over34px reduced4px). Existing small metadata and date numerals retain their size. Mobile inputs remain16px, native touch targets remain44px. Calendar date selection continues to show the existing report panel beside it on desktop and below on mobile.


### Denser web task grid (2026-10-02)
User requested another overall reduction and original date-view arrangement. Body14px, category13px, page28px, section20px, card16px; explicit16px+ text reduced2px while existing small text stays readable. Page/panel20px, assistant nested16px, group12px, row8px by12px. Web task groups use two equal columns above720px; right detail above900px, narrow web detail below two-column groups; mobile remains one column and inline selection detail. Touch targets44px and mobile inputs16px retained.

## Appearance controls — user approved 2026-10-05
- One active primary accent at a time: yellow (default), mint, blue, purple, or pink. Each has an AA-tested light/dark pair; swatches are the only simultaneous accent samples.
- Keep current neutral canvas/card hierarchy, borderless cards/fields and green completion feedback. Light mode also changes edit forms, labels, counters and status-pill contrast.
-44px palette trigger next to refresh. Native auto popover, labeled mode/color buttons with pressed state; Escape/outside click/close dismissal. Compact header keeps these controls available on mobile.
- Existing compact type scale,10px controls/16px panels and no additional motion remain in place.

### Mode-specific appearance role values
- Light accent/hover/on-accent: yellow #805b12 / #65460b / #ffffff; mint #076d57 / #055642 / #ffffff; blue #2459b8 / #18448f / #ffffff; purple #7144ad / #59318d / #ffffff; pink #a63665 / #85264e / #ffffff.
- Dark accent/hover/on-accent: yellow #ffdf93 / #ffe8b2 / #222222; mint #8dead9 / #b0f3e6 / #222222; blue #9bc7ff / #bddaff / #222222; purple #d0b8ff / #dfcfff / #222222; pink #ffafd1 / #ffcee4 / #222222.
- Light supporting roles: assignee neutral #e6e7eb, warm #f1e3ca / #6c4c0a, cool #dfedff / #26527b; edit #e6e7eb, field #f7f7f8, secondary #e6e7eb / #171719.
- Light completion: #278a53 / #ffffff, hover #1e6d41, selected tint #e4f3e9. Light warning label #8a5b1e. Dark supporting roles retain the approved values above.

## Light appearance polish — user approved2026-10-05
- Separate vivid action fill from text/focus. Primary fill/hover/on-fill/label: yellow #ffd43b / #f5be18 / #222222 / #805400; mint #11d6a0 / #06be8d / #123b2d / #005e46; blue #2563eb / #1d4ed8 / #ffffff / #1d4ed8; purple #8b3dff / #7625ed / #ffffff / #6524c8; pink #de1168 / #cc085a / #ffffff / #a80a49. Dark pairs unchanged.
- Light-mode static perimeter borders and separators are removed; cards/fields remain distinct through surface colors, spacing and existing shadow roles. Preserve visible keyboard focus and native checkbox/radio marks.
- Completion remains green. One primary hue selected at a time; no added animation or layout change.

## Aligned workspace surfaces — user approved2026-10-05
- The approved light/dark calendar mockups establish one shared content grid:1280px maximum with20px desktop/16px mobile outer gutters. Header command surface and every destination's content view share both edges.
- The borderless header surface groups the existing command launcher and refresh/appearance controls.16px panel corners, existing panel shadow and mode surface hierarchy; retain all existing actions and compact text.
- Calendar tiles use16px soft corners,8px desktop/4px phone grid gaps; top6px marker slot remains reserved with8px gap to circular date numerals. Calendar period buttons use existing10px control radius and44px targets.
- Retain one active accent, current palette pairs, green completion, visible keyboard focus and actual calendar data/flows. No screenshot example tasks are inserted into product data.


## Flex-reference calendar adaptation — 2026-10-05
The user requested the public flex time-tracking calendar as a visual reference. Weekly calendars now show Monday–Sunday columns and a local-clock time axis. Actual schedules use their duration, midnight-to-midnight events and tasks with a due date occupy the all-day strip, and home requests appear at their received timestamp. Request blocks label receipt time, never a fabricated work duration. Overlaps use lanes; mobile scrolls only the labeled calendar region. Day view remains unchanged.
Light neutrals use canvas #f5f6f8, white cards, subtle #eef1f4, stronger header/control fill #e4e9ee, strong text #24292e and muted text #596875. Inputs retain distinct neutral fill without perimeter borders. Selected light calendar dates use black #24292e circles and white labels. Calendar categories use mint/manual, blue/connected, purple/received request and amber/task fills with readable labels. These encode entry kinds independently of the saved primary color. The selectable light mint accent is #7affdc with charcoal text; other saved primary choices and dark surfaces remain. Existing shared edges, soft radii and compact typography are retained.


### Weekly calendar and light neutral token roles
| Role | Light value | Dark value |
| --- | --- | --- |
| Internal time grid | #e5ebef | #41464b |
| Manual schedule fill / label | #d2f3ef / #245958 | #284942 / #b5ecdd |
| Connected schedule fill / label | #dfedff / #245584 | #2d425b / #bfdcff |
| Received request fill / label | #efe4fb / #61407e | #473b5c / #e4ceff |
| All-day task fill / label | #fff3c7 / #6c5319 | #4a4430 / #ffe3a3 |
| Neutral hover | #e3e8ed | Existing surface-hover |
| Body text | #38434d | Existing text |
| Supporting text | #5c6a75 | Existing muted |
| Header command text | #46545f | Existing text |
| Selected light date circle / label | #24292e / #ffffff | Existing accent / on-accent |
| Light mint action / hover / label | #7affdc / #55ecc8 / #24292e | Existing dark mint pair |
These supersede earlier light mint and supporting-neutral values for this approved revision. The subtle internal time grid is necessary to read time positions; exterior card and field borders remain absent. AA checks cover readable event labels, muted labels and header copy.


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

Additional fixed light roles: completion hover #0e5a39; cool assignee identity #2458a6 on #eaf2ff.

### Light header separation — 2026-10-05
Superseded by the shared responsive command header revision below.

### Responsive command header — 2026-10-06
User requests the original launcher-and-adjacent-icons arrangement in both modes.
The header is a transparent grid: bounded question launcher, plain date label,
then separate44px refresh and appearance buttons. The launcher uses the approved
neutral surface token; there is no surrounding date/icon card. Wide layouts use
16px separation. At720px and below, keep the launcher and icons on the first row
with12px separation and place the date on a second row with8px gap. Mobile launcher
height48px, icon targets44px, date12px/20px. Command text ellipsizes while retaining
its full accessible name. Preserve original gutters, mode/primary preferences,
keyboard focus and existing refresh/assistant behavior.

### Light request queue scroll surface — 2026-10-05
Received-request queue shares its subtle neutral background across header and scrollable list. The fixed scroll hint uses the control-contrast fill with a12px fade above it, so partially clipped rows transition smoothly and the stationary footer is distinct. Scrollbar track follows the queue surface; selected request retains the chosen primary tint. No exterior borders or functional changes.

### Incoming decision card alignment — 2026-10-05
Metadata labels use shared intrinsic-width columns and stay on one line. Attention badge belongs in the action group, centered vertically beside the project action on desktop. Project action retains its original44px height and width by replacing the removed arrow/gap with12px additional padding on each side; neutral at rest, chosen primary on hover and keyboard focus. Mobile retains the full-width button with centered status above it. Existing callbacks/loading/disabled behavior remain.

### Conversation surfaces and alignment — 2026-10-05
The question launcher now uses the neutral control-contrast fill, separated from the date surface by the existing16px gap. The light chat composer uses a continuous subtle neutral fill with a transparent textarea and one2px rounded inset focus indicator on the whole composer. New-conversation icon/text share the same center without description margins; history text fills its single-column grid with8px normal padding. Existing send, disabled and responsive behavior retained.

### Mobile assistant reply contrast — 2026-10-05
Light mobile conversation canvas is gray; assistant replies use the white surface token and existing quiet panel shadow so the message is clearly separated. User replies retain chosen primary; dark and desktop bubble roles remain. No exterior border or new accent.

### User-selected light blue — 2026-10-06
Light blue primary is#4594FB; hover#3287EE and charcoal#191f28 action labels preserve AA text contrast. Standalone blue labels and focus use existing darker blue; primary tints derive from the new base. Dark blue and other palettes retained.

### Light blue label correction — 2026-10-06
User explicitly overrides previous darker standalone blue/charcoal action text: light blue accent, standalone labels and focus use#4594FB; solid primary text/icons use white, hover#3287EE retained. This is an approved contrast exception: white/base3.05:1 meets icon/large-text3:1, below4.5:1 for normal small text; accent labels on tinted surfaces also below normal-text AA. Other palettes and semantic statuses unchanged. All earlier contrary light-blue rules are superseded.
# Dark status contrast revision — 2026-10-06

Dark status badges use bright bounded fills with dark semantic ink so they separate
from charcoal cards: attention #664600 on #ffe3a3, success #174d35 on #a1edc5,
danger #82243d on #ffc0cc, neutral/disconnected #333d4b on #d3d9e2. Apply these
shared roles to decisions, task details and due/completion labels, connections and
meeting statuses. Existing alert surfaces retain their restrained dark treatment.
All five dark primary palettes retain their existing fills and labels; accent tint
now mixes20% active primary with the card surface rather than using fixed gray.
Supporting faint text uses#aeb4be. Current layouts, sizes, light semantics and the
user-approved light-blue contrast exception remain unchanged.
