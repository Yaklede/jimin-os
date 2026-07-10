import type { LucideIcon } from "lucide-react";
import { Check, CircleAlert, CircleX } from "lucide-react";

import { copy } from "../copy";

export type RowState = "ready" | "attention" | "unavailable";

export interface StatusRowData {
  label: string;
  description: string;
  state: RowState;
  Icon: LucideIcon;
}

interface StatusGroupProps {
  title: string;
  description: string;
  rows: StatusRowData[];
  loading?: boolean;
}

const stateCopy = {
  ready: copy.checks.ready,
  attention: copy.checks.attention,
  unavailable: copy.checks.disconnected,
} as const;

const stateIcon = {
  ready: Check,
  attention: CircleAlert,
  unavailable: CircleX,
} as const;

export function StatusGroup({
  title,
  description,
  rows,
  loading = false,
}: StatusGroupProps) {
  return (
    <section
      className="panel"
      aria-labelledby={`group-${toDomId(title)}`}
      aria-busy={loading}
    >
      <div className="panel__header">
        <div>
          <h2 id={`group-${toDomId(title)}`}>{title}</h2>
          <p>{description}</p>
        </div>
      </div>

      {loading ? (
        <ul className="status-list" aria-label={copy.actions.checking}>
          {[0, 1, 2, 3].map((index) => (
            <li className="status-row status-row--loading" key={index}>
              <span className="skeleton skeleton--icon" />
              <span className="status-row__copy">
                <span className="skeleton skeleton--label" />
                <span className="skeleton skeleton--description" />
              </span>
              <span className="skeleton skeleton--state" />
            </li>
          ))}
        </ul>
      ) : (
        <ul className="status-list">
          {rows.map(({ label, description: rowDescription, state, Icon }) => {
            const StateIcon = stateIcon[state];
            return (
              <li className="status-row" key={label}>
                <Icon className="status-row__leading-icon" aria-hidden="true" />
                <span className="status-row__copy">
                  <span className="status-row__label">{label}</span>
                  <span className="status-row__description">
                    {rowDescription}
                  </span>
                </span>
                <span className={`status-tag status-tag--${state}`}>
                  <StateIcon aria-hidden="true" />
                  <span>{stateCopy[state]}</span>
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

function toDomId(value: string): string {
  return value.replace(/\s+/g, "-");
}
