---
name: opendock-korea-equity-research
description: 한국 주식 리서치에서 KRX, OpenDART, 기준일, 공시 확인, 리스크, 반대 시나리오, 비추천 고지를 점검할 때 사용합니다.
---

# Korea Equity Research

한국 주식 리서치를 작성하거나 검토할 때 사용합니다.

## 절차

1. `KOREA_EQUITY_RESEARCH.md`를 읽습니다.
2. run 문서가 없으면 `.opendock/templates/korea-equity-research/EQUITY_RESEARCH_RUN.md`를 복사해 만듭니다.
3. 종목, 시장, 기준일, 데이터 출처, 공시 확인 범위를 먼저 고정합니다.
4. 가격, 거래량, 재무, 공시, 업종 비교, 거시 변수를 분리해서 봅니다.
5. 결론에는 리스크, 상승/하락 시나리오, 데이터 한계, 비추천 고지를 포함합니다.
6. 완료 전 `node .opendock/harness/opendock__korea-equity-research/check.mjs`를 실행합니다.

## 주의

매수/매도 추천, 목표가 보장, 수익 단정 표현을 만들지 않습니다.
