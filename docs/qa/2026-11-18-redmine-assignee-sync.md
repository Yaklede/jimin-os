# Redmine assignee synchronization

## Cause and scope

The existing queue carried the deadline and completion intent but not the
assignee. Its outbound Redmine issue update therefore never sent
`assigned_to_id`, even when the two systems used exactly the same spelling.
The queue now reads the current assignee values from the linked, noncancelled
tasks in the same snapshot as the deadline. Assignee-only edits use the
existing ticket version and worker lease checks.

## Contract

- Match the Jimin OS name to the Redmine display name ignoring Unicode
  whitespace only. Search users in the ticket's own Redmine project, not
  across the instance. Fetch all bounded membership pages before accepting
  a unique user ID; don't select groups or guess between same-name users.
- Check the candidate's assignable project roles. Redmine applies its own
  ticket permissions and workflow; a fresh issue read must confirm the ID.
  Recheck the issue project and the local lease before writing; a detected
  move or stale lease rejects the assignee update. Already matching IDs are no-ops.
- If multiple names remain after normalization (including multiple owners
  entered in one field or different people on a shared ticket), don't pick
  one. Blank names do not clear the existing Redmine assignee.
- Send the assignee separately from the existing deadline and status
  update. Assignee rejection or mismatch leaves an error in the existing
  ITSM recovery path but does not block the deadline or completion update.
- No new app routes, client build, credential scope, migration, historical
  backfill, or production test write. Existing allowlisted origin,
  no-redirect policy, response size limits and owner-verified links apply.
  The new reads use fixed paths and provider-returned numeric IDs; names
  are compared in memory and never sent in query strings or logged.

Redmine contracts: [issue updates](https://www.redmine.org/projects/redmine/wiki/rest_issues),
[project memberships](https://www.redmine.org/projects/redmine/wiki/Rest_Memberships),
and [assignable role response](https://github.com/redmine/redmine/blob/4.2-stable/app/views/roles/show.api.rsb).

## Regression evidence

- Before the worker change, the loopback regression `sends_assignee_id_for_a_spaced_redmine_name`
  failed on the actual PUT: expected `assigned_to_id: 81`, got null.
- The final loopback fixture covers reassignment, a unique member on a
  later page, regular and nonbreaking spaces, idempotent retry, an
  ambiguous and an incomplete directory, groups, an unassignable role,
  rejected or ignored PUTs, and a superseded job. No live Redmine calls.
- The isolated PostgreSQL contract checks that an assignee-only edit
  invalidates a leased job, the new job carries the updated name, and
  cancelled tasks contribute no assignee. Prior deadline/status semantics
  and owner-scoped retry remain under the existing regression suite.
- Final gates: Rust formatting, all-targets clippy with warnings denied,
  API/Agent build, 328 Agent/API/Storage unit tests (two opt-in live model
  tests excluded), 57 PostgreSQL scenarios and four scheduled Chat contracts
  passed. The scoped Backend Ultrawork scanner passed its package/docs checks;
  it excludes Rust, so the Rust/SQL paths were checked by lint, compilation,
  manual contract review, and the isolated tests above.
- Changes are local to the development worktree; no merge, production
  deployment, existing ticket backfill, or live assignment was performed.
