# Mobile conversation composer regression

Date: 2026-10-06
Status: fixed

## Symptom and cause

On the public preview, tapping the mobile microphone navigated to a conversation,
but the message field was behind the fixed bottom navigation. At390×844 the
composer occupied y790–854 while navigation started at768.
The shell grid already allocated100px to the topbar; mobile content nevertheless
requested100dvh, oversizing that grid row by100px.

## Fix

- Mobile chat content uses100% of its allocated shell grid row.
- Web phone navigation clearance increases from90px to the existing100px scale
  so wrapped labels at320px also clear the composer.
- Light input uses a darker neutral derived from existing control and muted
  tokens, with the existing panel shadow to distinguish it from the gray canvas.
  Follow-up:320×568,390×844 and1280×900 verified; focus retains one2px
  primary indicator, and typing/clearing enables/disables send.

Only `apps/desktop/src/styles.css` was changed in application code.

## Browser regression (RED → GREEN)

Reproduce: home → mobile microphone → conversation. Verify the labeled textarea
exists, its complete form remains above navigation, and the document does not
overflow horizontally. Read-only DOM check below ran through CUA.
The unchanged public version failed with -86px clearance; the fixed local version
passed with24px at390×844 and6px at320×568. Both modes passed.

| Mode | Viewport | Composer bottom | Navigation top | Clearance | Result |
| --- | --- | --- | --- | --- | --- |
| dark | 320×568 | 468 | 474 | 6 | PASS |
| dark | 390×460 | 360 | 384 | 24 | PASS |
| dark | 390×844 | 744 | 768 | 24 | PASS |
| dark | 430×932 | 832 | 856 | 24 | PASS |
| light | 320×568 | 468 | 474 | 6 | PASS |
| light | 390×460 | 360 | 384 | 24 | PASS |
| light | 390×844 | 744 | 768 | 24 | PASS |
| light | 430×932 | 832 | 856 | 24 | PASS |
| light | 1280×900 | 856 | None | 44 | PASS |
| dark | 1280×900 | 856 | None | 44 | PASS |

Desktop navigation is hidden, so its bottom limit is the viewport edge.

```js
() => { const composer=document.querySelector('.assistant-request-field'); const input=document.querySelector('#agent-message'); const nav=document.querySelector('.os-mobile-nav'); if(!composer||!input||!nav) return {pass:false,reason:'Missing composer or mobile navigation'}; const formRect=composer.getBoundingClientRect(); const inputRect=input.getBoundingClientRect(); const navRect=nav.getBoundingClientRect(); const navVisible=getComputedStyle(nav).display!=='none'; const bottomLimit=navVisible?Math.min(innerHeight,navRect.top):innerHeight; const inputVisible=inputRect.width>0&&inputRect.height>0&&inputRect.top>=0&&formRect.bottom<=bottomLimit; return {pass:inputVisible&&document.documentElement.scrollWidth<=innerWidth,viewport:{width:innerWidth,height:innerHeight},composer:{top:formRect.top,bottom:formRect.bottom},input:{top:inputRect.top,bottom:inputRect.bottom},navigationTop:navVisible?navRect.top:null,contentBottom:document.querySelector('.os-content')?.getBoundingClientRect().bottom,gap:bottomLimit-formRect.bottom,horizontalOverflow:document.documentElement.scrollWidth>innerWidth}; }
```

## Interaction verification

- Mouse/pointer activation and Enter on microphone navigated from home to chat.
- Textarea accepted an unsent draft; send enabled after typing and disabled when cleared.
- At390×460 the transcript scrolled from0 to269.5px while composer remained visible.
- Existing inline error after `새 요청` in the offline preview did not cover input.
  New empty-conversation flow could not be reached because the preview does not
  provide the conversation archival service; that existing unrelated behavior was
  not changed by this fix.
- No real iOS/Android keyboard or safe-area hardware was available. Shortened
  browser viewport verification is not a physical-device keyboard test.
- No new listener, animation, dependency, focus behavior or sending logic.

## Checks

- TypeScript typecheck passed.
- Related OsShell/mobile capability/conversation response tests:18 passed.
- Release validation: full frontend suite55files/340tests passed.
- CSS formatting and diff whitespace checks passed.
- Design and interaction quality gates passed; run manifests marked completed.

## Prevention

When adding shell header rows, preserve the child's allocated-height contract.
Re-run the geometry check at320px and a shortened mobile viewport before release.
Bug notes are stored here because workspace instructions prohibit modifying `.agents/`.
