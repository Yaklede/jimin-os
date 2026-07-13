import { push } from "./utils.mjs";

const timerPairs = [
  ["setTimeout", "clearTimeout"],
  ["setInterval", "clearInterval"],
  ["requestAnimationFrame", "cancelAnimationFrame"],
];

function validateCleanup(target, failures) {
  for (const [start, cleanup] of timerPairs) {
    if (new RegExp(`\\b${start}\\s*\\(`).test(target.text) && !new RegExp(`\\b${cleanup}\\s*\\(`).test(target.text)) {
      push(failures, "timer-cleanup", target.rel, `${start} needs matching ${cleanup} cleanup in the same target file.`);
    }
  }

  const addsListener = /\baddEventListener\s*\(/.test(target.text);
  const removesListener = /\bremoveEventListener\s*\(/.test(target.text);
  const abortsListener = /\bsignal\s*:|new\s+AbortController\s*\(|\.abort\s*\(/.test(target.text);
  const oneShotListener = /addEventListener\s*\([\s\S]{0,240}\bonce\s*:\s*true/.test(target.text);
  if (addsListener && !removesListener && !abortsListener && !oneShotListener) {
    push(
      failures,
      "listener-cleanup",
      target.rel,
      "addEventListener needs removeEventListener, AbortSignal, or an explicit one-shot listener in the same target file.",
    );
  }
}

export function validateSource(run, tier, targets, failures) {
  const combined = targets.map((target) => `\n/* ${target.rel} */\n${target.text}`).join("\n");

  for (const target of targets) {
    if (/\btransition-all\b|\btransition\s*:\s*all\b/i.test(target.text)) {
      push(failures, "transition-all", target.rel, "List the properties that transition instead of using transition-all.");
    }

    for (const match of target.text.matchAll(/<(div|span|li)\b([^>]*)>/gi)) {
      const attributes = match[2];
      if (!/(?:onClick|onclick|@click)\s*=/i.test(attributes)) continue;
      const hasRole = /\brole\s*=/.test(attributes);
      const hasTabIndex = /\btabIndex\s*=|\btabindex\s*=/.test(attributes);
      const hasKeyboard = /onKeyDown\s*=|onKeyUp\s*=|onKeyPress\s*=|@keydown\s*=|@keyup\s*=/i.test(attributes);
      if (!(hasRole && hasTabIndex && hasKeyboard)) {
        push(
          failures,
          "nonsemantic-click-target",
          target.rel,
          `Clickable <${match[1]}> needs a native control or complete role, focus, and keyboard behavior.`,
        );
      }
    }

    validateCleanup(target, failures);
  }

  const hasFocusSupport = /:focus(?:-visible|-within)?\b|\bonFocus\b|\bonBlur\b|\bfocus-visible:|\bfocus:/i.test(combined);
  const cssHover = /:hover\b/i.test(combined);
  const jsHover = /\bonMouse(?:Enter|Leave)\b|addEventListener\s*\(\s*["'](?:mouseover|mouseenter|mouseleave)["']/i.test(combined);
  const hoverMedia = /@media\s*\(\s*hover\s*:\s*hover\s*\)/i.test(combined);
  if ((jsHover && !hasFocusSupport) || (cssHover && !hasFocusSupport && !hoverMedia)) {
    push(failures, "hover-only-behavior", run.manifestRel, "Hover behavior needs a focus or persistent equivalent.");
  }

  const mouseOnly = /\bonMouse(?:Down|Move|Up)\b|addEventListener\s*\(\s*["']mouse(?:down|move|up)["']/i.test(combined);
  const pointerOrTouch = /\bonPointer(?:Down|Move|Up|Cancel)\b|\bonTouch(?:Start|Move|End|Cancel)\b|addEventListener\s*\(\s*["'](?:pointer|touch)/i.test(
    combined,
  );
  if (mouseOnly && !pointerOrTouch) {
    push(failures, "mouse-only-behavior", run.manifestRel, "Mouse-only interaction needs Pointer Events or an equivalent touch path.");
  }

  if (/\boutline\s*:\s*(?:none|0)\b|\boutline-none\b/i.test(combined) && !/:focus-visible\b|\bfocus-visible:/i.test(combined)) {
    push(failures, "focus-indicator-suppressed", run.manifestRel, "Do not suppress focus outlines without a visible focus-visible replacement.");
  }

  const hasMotion =
    /\btransition(?:-property)?\s*:|\banimation\s*:|@keyframes|\.animate\s*\(|requestAnimationFrame|\btransition-(?:all|colors|opacity|transform|shadow)\b|framer-motion|motion\/react/i.test(
      combined,
    );
  const hasReducedMotion = /prefers-reduced-motion|useReducedMotion|reducedMotion|motion-reduce:|matchMedia\s*\([^)]*prefers-reduced-motion/i.test(
    combined,
  );
  if (hasMotion && !hasReducedMotion) {
    push(failures, "reduced-motion-missing", run.manifestRel, "Motion source needs a prefers-reduced-motion or equivalent runtime branch.");
  }

  const hasAsync = /\bfetch\s*\(|\baxios\b|\buseMutation\b|\bcreateAsyncThunk\b|\basync\s+function\b|\basync\s*(?:\([^)]*\)|[A-Za-z_$][\w$]*)\s*=>/i.test(
    combined,
  );
  if (hasAsync && !/\b(?:isLoading|loading|pending|isPending|aria-busy)\b/i.test(combined)) {
    push(failures, "loading-state-missing", run.manifestRel, "Async interaction source needs an observable loading or pending state.");
  }
  if (hasAsync && !/\b(?:error|hasError|isError|catch)\b/i.test(combined)) {
    push(failures, "error-state-missing", run.manifestRel, "Async interaction source needs an error and recovery path.");
  }
  if (hasAsync && !/\b(?:disabled|aria-disabled|isSubmitting|inFlight)\b/i.test(combined)) {
    push(failures, "disabled-state-missing", run.manifestRel, "Async interaction source needs disabled or duplicate-submission protection.");
  }

  const hasOverflowMitigation = /overflow-x\s*:\s*(?:auto|scroll|clip)|\boverflow-x-(?:auto|scroll|clip)\b|max-width\s*:\s*100%|\bmax-w-full\b/i.test(
    combined,
  );
  let fixedWidthRisk = /\bwidth\s*:\s*100vw\b|\bw-screen\b/i.test(combined);
  for (const match of combined.matchAll(/\b(?:min-)?width\s*:\s*(\d+)px\b/gi)) {
    if (Number(match[1]) >= 768) fixedWidthRisk = true;
  }
  if (fixedWidthRisk && !hasOverflowMitigation) {
    push(failures, "horizontal-overflow-risk", run.manifestRel, "100vw or large fixed width needs a documented containment/overflow strategy in source.");
  }

  const usesMotion = /from\s*["'](?:framer-motion|motion(?:\/react)?)["']|require\s*\(\s*["'](?:framer-motion|motion(?:\/react)?)["']|<motion\./i.test(
    combined,
  );
  const usesSpecial = /from\s*["'](?:gsap|animejs|anime\.js)["']|require\s*\(\s*["'](?:gsap|animejs|anime\.js)["']|<(?:animateMotion|animateTransform)\b/i.test(
    combined,
  );
  if (usesMotion && tier !== "motion") {
    push(failures, "library-tier-mismatch", run.manifestRel, "Motion usage must match an explicit Motion implementation tier.");
  }
  if (tier === "motion" && !usesMotion) {
    push(failures, "library-tier-mismatch", run.manifestRel, "Motion tier is selected but no Motion usage appears in target files.");
  }
  if (usesSpecial && tier !== "special") {
    push(failures, "library-tier-mismatch", run.manifestRel, "Special timeline/SVG usage must match an explicit special implementation tier.");
  }
}
