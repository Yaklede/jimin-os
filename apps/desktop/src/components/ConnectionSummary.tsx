import { CheckCircle2, CircleAlert, CircleX, LoaderCircle } from "lucide-react";

import { copy } from "../copy";

export type ConnectionMode = "checking" | "ready" | "attention" | "unavailable";

interface ConnectionSummaryProps {
  mode: ConnectionMode;
}

const summaryByMode = {
  checking: {
    title: copy.summary.checkingTitle,
    body: copy.summary.checkingBody,
    Icon: LoaderCircle,
  },
  ready: {
    title: copy.summary.readyTitle,
    body: copy.summary.readyBody,
    Icon: CheckCircle2,
  },
  attention: {
    title: copy.summary.attentionTitle,
    body: copy.summary.attentionBody,
    Icon: CircleAlert,
  },
  unavailable: {
    title: copy.summary.disconnectedTitle,
    body: copy.summary.disconnectedBody,
    Icon: CircleX,
  },
} as const;

export function ConnectionSummary({ mode }: ConnectionSummaryProps) {
  const { title, body, Icon } = summaryByMode[mode];

  return (
    <section
      className={`connection-summary connection-summary--${mode}`}
      aria-labelledby="status-title"
    >
      <div className="connection-summary__icon" aria-hidden="true">
        <Icon className={mode === "checking" ? "spin" : undefined} />
      </div>
      <div>
        <p className="eyebrow">{copy.scope}</p>
        <h1 id="status-title">{title}</h1>
        <p className="connection-summary__body">{body}</p>
      </div>
    </section>
  );
}
