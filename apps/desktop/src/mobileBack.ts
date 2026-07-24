type MobileBackHandler = () => boolean;

type MobileBackEntry = {
  handler: MobileBackHandler;
  order: number;
  priority: number;
};

declare global {
  interface Window {
    __JIMIN_OS_ANDROID_BACK__?: () => boolean;
  }
}

let nextOrder = 0;
const handlers: MobileBackEntry[] = [];

export function registerMobileBackHandler(
  handler: MobileBackHandler,
  priority = 0,
): () => void {
  const entry = {
    handler,
    order: nextOrder++,
    priority,
  };
  handlers.push(entry);
  return () => {
    const index = handlers.indexOf(entry);
    if (index >= 0) handlers.splice(index, 1);
  };
}

export function handleMobileBack(): boolean {
  const candidates = [...handlers].sort(
    (left, right) => right.priority - left.priority || right.order - left.order,
  );
  for (const candidate of candidates) {
    if (candidate.handler()) return true;
  }
  return false;
}

export function installAndroidBackBridge(): () => void {
  const previousHandler = window.__JIMIN_OS_ANDROID_BACK__;
  window.__JIMIN_OS_ANDROID_BACK__ = handleMobileBack;
  return () => {
    if (previousHandler) {
      window.__JIMIN_OS_ANDROID_BACK__ = previousHandler;
      return;
    }
    delete window.__JIMIN_OS_ANDROID_BACK__;
  };
}
