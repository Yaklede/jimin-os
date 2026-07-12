---
name: opendock-korea-real-estate-research
description: 한국 부동산 리서치에서 공식 출처, 기준일, 지역, 거래유형, 한계, 반대 시나리오, 비추천 고지를 점검할 때 사용합니다.
---

# Korea Real Estate Research

한국 부동산 리서치를 작성하거나 검토할 때 사용합니다.

## 절차

1. `KOREA_REAL_ESTATE_RESEARCH.md`를 읽습니다.
2. run 문서가 없으면 `.opendock/templates/korea-real-estate-research/REAL_ESTATE_RESEARCH_RUN.md`를 복사해 만듭니다.
3. 질문, 지역, 기간, 거래유형, 주택유형, 기준일, 출처를 먼저 고정합니다.
4. 실거래가, 전월세, 가격지수, 거래량, 거시지표를 출처별 정의와 함께 비교합니다.
5. 결론에는 반대 시나리오, 데이터 한계, 추가 확인 질문을 포함합니다.
6. 완료 전 `node .opendock/harness/opendock__korea-real-estate-research/check.mjs`를 실행합니다.

## 주의

투자 추천, 가격 상승 보장, 매수 유도 표현을 만들지 않습니다.
