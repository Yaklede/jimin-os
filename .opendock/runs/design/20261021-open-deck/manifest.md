# Design Run Manifest

Status: reviewed

## Target Files

- `apps/desktop/src/App.tsx`
- `apps/desktop/src/components/PlanningWorkspace.tsx`
- `apps/desktop/src/components/ScheduledWorkPanel.tsx`
- `apps/desktop/src/components/HomeWorkspace.tsx`
- `apps/desktop/src/components/HomeInflowReview.tsx`

## Design Contract

- DESIGN.md reviewed: yes
- STYLESEED.md reviewed or updated: preserve approved existing lock
- Key color/accent: existing mint; no visual CSS changes
- Radius personality: existing soft precision
- Motion style: existing silk-snap

## Palette Planning

Palette Source: existing approved Jimin OS design system
Palette Mood: calm, precise personal assistant
Palette Role Map: reuse existing canvas/surface/text/action/semantic roles
Contrast Plan: existing components and labels retained
Color Risks: no new colors or accent added

## Layout Planning

Layout Type: existing assistant OS navigation and modal forms
First Gaze: requested existing destination; no new Jimin OS screen
Primary Action: existing create form or scheduled-work list
Section Architecture: existing home/calendar/meetings/assistant structure retained
Reference Categories: existing project component system
Reference Notes: StyleSeed preservation rule applied; request adds navigation intents, not visual redesign
Do Not Copy: screenshot, exact copy, brand asset, paid/private reference content

## Component System Planning

Component Inventory: existing PlanningWorkspace, PlanningCreateDialog, ScheduledWorkPanel, OS shell
Typography Token Plan: reuse current Wanted Sans role tokens
Spacing Token Plan: existing component spacing unchanged
Radius Token Plan: existing controls/panels/dialog roles unchanged
Shadow Token Plan: existing panel elevation unchanged
State Coverage: disconnected/auth expired/action failure/dirty modal navigation block; existing modal empty/loading/success/error states reused

## Review

- Typography: unchanged
- Colors: unchanged
- Spacing: unchanged
- Radius: unchanged
- Component decisions: existing forms own create/edit confirmation
- Token usage: no CSS additions
- Interaction states: bounded acknowledged intents; no background focus change
- Responsive behavior: bridge enabled only on macOS
- Accessibility: existing dialog focus/labels/touch targets retained

## Exceptions

No visual contract exception requested. Full backend repository scan has unchanged findings recorded in docs/integrations/open-deck.md.
