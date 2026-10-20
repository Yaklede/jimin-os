import { AlertCircle, CheckCircle2, Clock, MinusCircle } from "lucide-react";
import type { ProjectInflowItem } from "../api/googleChat";
import { inflowRecoveryCopy as text } from "../copy/inflowRecovery";
import { inflowDeliverySteps } from "../inflowDeliveryState";
import "./inflowRecovery.css";

const icons = {
  done: CheckCircle2,
  pending: Clock,
  failed: AlertCircle,
  not_requested: MinusCircle,
};

export function InflowDeliveryStatus({ item }: { item: ProjectInflowItem }) {
  return (
    <ol className="inflow-delivery-status" aria-label="저장과 Chat 반영 상태">
      {inflowDeliverySteps(item).map((step) => {
        const Icon = icons[step.state];
        return (
          <li key={step.key} data-state={step.state}>
            <Icon aria-hidden="true" />
            <span>{step.label}</span>
            <strong>{text.state[step.state]}</strong>
          </li>
        );
      })}
    </ol>
  );
}
