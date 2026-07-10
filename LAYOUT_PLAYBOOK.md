<!-- OPENDOCK:START id=files:LAYOUT_PLAYBOOK.md dock=opendock/design-ultrawork path=LAYOUT_PLAYBOOK.md -->
# Layout Playbook

작업 전에 화면 유형을 먼저 고르고, 그 유형에 맞는 구조를 설계합니다. 바로 UI를 만들지 말고 `DESIGN.md`, `REFERENCE_RESEARCH.md`, 이 문서를 순서대로 확인합니다.

## 공통 결정

- Layout Type: 이 작업은 어떤 화면인가?
- First Gaze: 사용자의 첫 시선이 어디로 가야 하는가?
- Primary Action: 가장 중요한 행동은 무엇인가?
- Section Architecture: 어떤 순서로 정보를 보여줄 것인가?
- Density: 여유 있는 마케팅형인지, 반복 사용을 위한 작업형인지?
- Motion Purpose: motion이 이해를 돕는가, 단순 장식인가?

## Ecommerce

권장 구조:

1. Compact nav with search, cart, account
2. Product/category hero or campaign band
3. Featured categories
4. Product grid with filter/sort
5. Trust signal: delivery, return, review, secure payment
6. Recommendation or bundle section
7. Final CTA or newsletter

주의:

- 상품 카드 높이가 내용에 따라 흔들리지 않게 합니다.
- 가격, 할인, 재고, 리뷰 정보의 위계를 명확히 합니다.
- 필터는 모바일에서 sheet/drawer로 전환할 수 있어야 합니다.
- CTA는 `Add to cart`, `View details`, `Checkout`처럼 행동을 분명히 씁니다.

## Blog / Editorial

권장 구조:

1. Editorial header with topic and search
2. Featured article or latest issue
3. Category navigation
4. Article list with metadata
5. Author/series/trending module
6. Newsletter or follow CTA

주의:

- 제목, 요약, 날짜, 저자, 카테고리의 hierarchy를 과하게 경쟁시키지 않습니다.
- 본문 가독성을 위해 line length와 spacing을 안정적으로 유지합니다.
- 광고/CTA가 본문 flow를 압도하지 않게 합니다.

## Portfolio

권장 구조:

1. Name or studio identity as first viewport signal
2. Role, specialty, proof point
3. Selected work grid
4. Case study preview with impact metric
5. About/process
6. Contact CTA

주의:

- 포트폴리오는 장식보다 작업물과 결과가 먼저 보여야 합니다.
- 카드형 반복 요소는 동일한 image ratio와 metadata structure를 유지합니다.
- 과한 motion은 project inspection을 방해하지 않아야 합니다.

## Landing Page

권장 구조:

1. Hero: offer, proof, primary CTA
2. Social proof
3. Problem or before-state
4. Product/value demonstration
5. Feature/use case sections
6. Pricing, demo, or contact CTA
7. FAQ
8. Final CTA

주의:

- 첫 화면에서 무엇을 제공하는지 즉시 보여야 합니다.
- CTA는 한 화면에 너무 많이 경쟁하지 않아야 합니다.
- proof 없이 claim만 늘어놓지 않습니다.

## SaaS Website

권장 구조:

1. Hero with product outcome
2. Product screenshot or workflow demo
3. Core workflow
4. Integrations or ecosystem
5. Security/trust
6. Pricing or demo CTA

주의:

- 기능 나열보다 사용자가 완성하는 workflow를 보여줍니다.
- 복잡한 dashboard screenshot은 annotation과 함께 배치합니다.
- B2B라면 security, compliance, admin control을 빠뜨리지 않습니다.

## Dashboard / Work Tool

권장 구조:

1. Global nav or sidebar
2. Key status / metrics
3. Primary work queue
4. Filters and saved views
5. Detail panel or action rail
6. Empty, loading, error states

주의:

- 작업 도구는 마케팅 hero처럼 만들지 않습니다.
- 정보 밀도는 높되, grouping과 alignment로 scanning을 돕습니다.
- repeat action, keyboard/focus, disabled/loading state가 필요합니다.

## Mobile App

권장 구조:

1. Clear top app bar or contextual header
2. Primary content list/card/feed
3. Sticky or reachable primary action
4. Bottom navigation only when top-level sections이 명확할 때
5. Empty/error/loading state

주의:

- 터치 target은 최소 44px를 지킵니다.
- 긴 label은 줄바꿈되거나 truncation rule이 있어야 합니다.
- 모바일에서 horizontal scroll은 blocker입니다.
<!-- OPENDOCK:END id=files:LAYOUT_PLAYBOOK.md dock=opendock/design-ultrawork path=LAYOUT_PLAYBOOK.md -->
