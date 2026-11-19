# ITSM links on scheduled Google Chat reminders

- Reminders that already show a completion code keep the visible
  `[T-…]` label. If the task has an enabled, owner-scoped ITSM binding,
  that label opens the exact HTTPS ticket. No ticket is inferred from
  the title, notes, or public reference links.
- The title, deadline, assignee mentions, and completion code are unchanged.
  Missing, disabled, malformed, or mismatched bindings retain the ordinary
  text label. Rules without completion codes stay as they were. No ticket
  request, credential, or new provider permission is needed to build a link.
- The plain preview and run history have the same message breaks as the
  outbound variant and do not show raw provider markup. The optional rich
  body is stored only in the existing private delivery payload; neither the
  client contract nor the database schema changes.
- The outbound hyperlink uses [Google Chat's documented link format](https://developers.google.com/workspace/chat/format-messages#format_links).
  Only linked reminder chunks set Chat markup explicitly. Older queued
  reminders, normal assignments, and completion notices retain their
  previous transport. Link syntax is never cut at a message boundary.
- The same code, tracked thread receipt, sender permission, and review
  flow still govern completion replies. The link does not grant access:
  recipients open it under their existing ITSM permissions.

## Verification

- The isolated scheduled reply contract reproduced the missing link before
  the change. It checks the verified ticket in the queued outbound body,
  plain preview compatibility, and an accepted `완료 [T-…]` reply.
- Storage and API tests cover title-only and detailed modes, a row without an
  ITSM binding, malformed links and mismatching IDs, chunk boundaries,
  legacy payload fallback, preserved mentions and stable message IDs.
- Passed: Rust formatting, API/Google/Storage all-target Clippy with warnings
  denied, 278 unit tests, API build, 57 isolated PostgreSQL scenarios and
  four scheduled-work/Chat contracts. The scoped Backend Ultrawork scanner
  passed its package/document checks; Rust and SQL were verified separately
  by compilation, lint, review, and the isolated tests.
- No production Google Chat message was sent and no ITSM ticket was changed.
