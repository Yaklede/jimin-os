# Reviewed web preview branch

- Branch: codex/responsive-design-preview
- Starting commit:908da1c (main)
- Fork: https://github.com/annieppss/jimin-os
- Upstream: https://github.com/Yaklede/jimin-os
- Reviewed production preview: https://jimin-os-design-preview.vercel.app
- Deployment: dpl_AQKCkVy5C35BE9A89pNbyVreigiV, READY

The branch preserves the source of the reviewed Vercel preview: responsive shell, light/dark appearance and configurable primary, month/week calendars, received-request review/registration, project task navigation and compact shared surfaces. It includes local font/image assets with the bundled font license, preview fixtures, build flag declarations, static routing config and verification documentation. Compiled dist files and local credentials are excluded. The unrelated tracked OpenDock run-status edit is left outside this commit.

The preview build uses VITE_DESIGN_PREVIEW=1; fixture API operations remain isolated from a real server. Normal builds retain the existing backend configuration. Vercel publication does not produce installed native binaries.

Validation before publication: TypeScript passed;55files/340tests passed; formatting, preview production build and scoped design/interaction gates passed. Public stylesheet index-B8yxP-SW.css and script index-CPZLjcfQ.js match the reviewed build. Responsive geometry and appearance interactions were checked locally and publicly; details are in the accompanying verification documents.

The repository secret scanner flagged TaskSelectionControl.tsx and styles.css because its unanchored sk- pattern matches inside task- CSS identifiers. All6 hits were inspected and proved to start inside a task- identifier; none is credential material. No scanner or protection was bypassed or changed. Other scanner patterns produced no findings.

The bundled font license had a trailing space on line21; it was removed for the staged whitespace check without changing its license text or the font binary.
