<!-- OPENDOCK:START id=files:REFERENCE_RESEARCH.md dock=opendock/design-ultrawork path=REFERENCE_RESEARCH.md -->
# Design Reference Research

Design Ultrawork는 레퍼런스를 복사하기 위한 도구가 아니라, 작업 전에 구조와 판단 기준을 잡기 위한 리서치 레이어입니다.

## 원칙

- `DESIGN.md`가 최우선 design contract입니다.
- 레퍼런스 사이트는 inspiration입니다. 이미지, 문구, 인터랙션, 코드, asset을 그대로 복사하지 않습니다.
- 참고한 내용은 layout intent, hierarchy, density, navigation pattern, CTA role, motion purpose처럼 추상화해서 기록합니다.
- 유료/로그인/저작권이 있는 콘텐츠는 자동 수집하거나 dock payload에 포함하지 않습니다.
- 결과물에는 출처 URL 자체보다 “무엇을 배웠고 어떻게 다르게 적용했는지”를 남깁니다.

## Reference Map

| 작업 유형 | 우선 참고 |
|---|---|
| 랜딩 페이지 | Landing Love, Saaspo, CTA Gallery, Navbar Gallery |
| SaaS 웹사이트 | Saaspo, Landing Love, Component Gallery |
| 쇼핑몰/커머스 | Mobbin, Component Gallery, CTA Gallery, Navbar Gallery |
| 블로그/에디토리얼 | Curated Design, Component Gallery |
| 포트폴리오 | Curated Design, Landing Love, Rebrand Gallery |
| 브랜드/리브랜딩 | Rebrand Gallery, Curated Design |
| 컬러 팔레트 | Coolors, Color Hunt, Adobe Color |
| 내비게이션 | Navbar Gallery |
| CTA/전환 섹션 | CTA Gallery |
| 컴포넌트/디자인 시스템 | Component Gallery |
| 모션/인터랙션 | Landing Love, AppMotion, 60fps.design |
| 아이콘 시스템 | Hugeicons, current project icon set |

## Sites

- Web Design: https://curated.design
- Landing Pages: https://landing.love
- SaaS Websites: https://saaspo.com
- Navbar: https://navbar.gallery
- CTA Sections: https://cta.gallery
- Animation: https://appmotion.design
- Mobile Apps: https://mobbin.com/?via=abraham
- Brands: https://rebrand.gallery
- Icons: https://hugeicons.com/?via=Abraham
- Design Systems: https://component.gallery
- Color Palettes: https://coolors.co/
- Hand-picked Palettes: https://colorhunt.co/
- Color Theory / Themes: https://color.adobe.com/

## 기록 형식

작업 전에 `.opendock/runs/design/<run-id>/manifest.md`에 아래 항목을 채웁니다.

```md
Layout Type: ecommerce | blog | portfolio | landing | saas | dashboard | mobile | brand | component
Reference Categories: landing, cta, navbar
Reference Notes: Hero는 product-first, CTA는 single primary action, nav는 compact dropdown이 적합.
Palette Source: Coolors explore + Adobe Color
Palette Mood: professional, fresh, not beige-heavy
Palette Role Map: canvas, surface, text, border, primary, secondary, focus, semantic colors
Contrast Plan: body text, CTA, disabled, focus ring 대비 확인
Color Risks: muddy warmth, extra accent colors, low contrast, semantic color confusion 방지
Do Not Copy: screenshot, exact copy, brand asset, paid/private reference content
```

## 추출 기준

- Layout: first viewport, section order, grid, density, scan path
- Hierarchy: headline, supporting copy, media, proof, CTA의 우선순위
- Interaction: hover, focus, reduced motion, menu behavior, transition purpose
- Conversion: primary CTA, secondary CTA, trust signal, pricing/demo/contact action
- Brand: color temperature, typography voice, icon tone, imagery treatment
- Component: state, responsive behavior, a11y expectation, empty/error/loading variant
<!-- OPENDOCK:END id=files:REFERENCE_RESEARCH.md dock=opendock/design-ultrawork path=REFERENCE_RESEARCH.md -->
