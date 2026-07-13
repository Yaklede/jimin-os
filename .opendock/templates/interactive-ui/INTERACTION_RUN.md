<!-- OPENDOCK:START id=files:.opendock/templates/interactive-ui/INTERACTION_RUN.md dock=opendock/interactive-ui-ultrawork path=.opendock/templates/interactive-ui/INTERACTION_RUN.md -->
# Interaction Run Manifest

Status: draft
Interaction Type: describe the control or flow
Framework: React, Vue, Svelte, vanilla, or other existing stack
Implementation Tier: CSS
Library Decision: none - use platform capabilities and existing project code
Library Installation: none - this dock never installs libraries automatically
Primary Trigger: describe the user input that starts the interaction
Primary Feedback: describe the immediate visible and accessible response
Primary Completion: describe the observable condition that confirms the interaction completed
Recovery Path: describe how the user recovers, retries, cancels, or returns to a stable state
Focus Contract: describe focus ownership before, during, and after completion or recovery
Motion Complexity Evidence: not applicable - explain why CSS or WAAPI is sufficient
Special Choice Evidence: not applicable - no special timeline or SVG choreography is used

## Target Files

List only files created or changed for this interaction task. Do not list directories or unrelated historical UI files.

- `src/example.tsx`
- `src/example.css`

## Interaction State Matrix

Replace every placeholder with an observed behavior or a detailed `not applicable - <reason>`.

| State | Behavior and evidence |
| --- | --- |
| idle | describe the stable starting state |
| hover | describe supplemental hover feedback or why it is not applicable |
| focus | describe visible focus and focus ownership |
| pressed/active | describe press acknowledgement and activation |
| loading | describe progress feedback or why no async work exists |
| error | describe recovery feedback or why failure is impossible here |
| disabled | describe deduplication/unavailable behavior or why it is not applicable |
| reduced motion | describe the no-motion or low-motion branch |

## Input Parity Evidence

Keyboard Evidence: record keys, focus order, activation, close, and focus restoration observed
Touch Evidence: record touch or Pointer Events behavior and target behavior observed
Focus Evidence: record visible focus, focus ownership, and any restoration observed

## Motion Evidence

Reduced Motion Evidence: record the media/runtime branch and observed result

## Async State Evidence

Loading Evidence: record the loading behavior or a detailed non-applicable reason
Error Evidence: record the error and recovery behavior or a detailed non-applicable reason
Disabled Evidence: record duplicate prevention/unavailable behavior or a detailed non-applicable reason

## Cleanup Evidence

Cleanup Evidence: record timer, animation frame, WAAPI, listener, observer, subscription, and unmount/cancel cleanup checked

## Responsive And Overflow Evidence

Horizontal Overflow Evidence: record viewport/zoom/long-content checks and observed result

## Validation Evidence

Validation Commands: record exact lint, test, build, browser, or device checks executed
Validation Result: record passed/failed result and concrete observations

## Exceptions

None. For an accepted exception, record the human owner, affected rule, reason, and follow-up.
<!-- OPENDOCK:END id=files:.opendock/templates/interactive-ui/INTERACTION_RUN.md dock=opendock/interactive-ui-ultrawork path=.opendock/templates/interactive-ui/INTERACTION_RUN.md -->
