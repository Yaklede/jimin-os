---
name: opendock-korea-macro-research
description: 한국 거시경제 리서치에서 ECOS, KOSIS, 기준일, 단위, 공표 주기, 지표 정의, 한계, 반대 시나리오를 점검할 때 사용합니다.
---

# Korea Macro Research

한국 거시경제 리서치를 작성하거나 검토할 때 사용합니다.

## 절차

1. `KOREA_MACRO_RESEARCH.md`를 읽습니다.
2. run 문서가 없으면 `.opendock/templates/korea-macro-research/MACRO_RESEARCH_RUN.md`를 복사해 만듭니다.
3. 지표, 출처, 기준일, 단위, 공표 주기, 계절조정 여부를 먼저 고정합니다.
4. 수준, 변화율, 전년동월비, 전월비, 보조 지표를 분리합니다.
5. 결론에는 가능 경로, 반대 시나리오, 데이터 한계를 포함합니다.
6. 완료 전 `node .opendock/harness/opendock__korea-macro-research/check.mjs`를 실행합니다.

## 주의

단일 지표만으로 투자나 사업 결정을 단정하지 않습니다.
