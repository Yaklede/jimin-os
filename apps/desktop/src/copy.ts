export const copy = {
  productName: "Jimin OS",
  scope: "개인 서버",
  pageTitle: "연결 상태",
  pageDescription: "내 데이터와 일정이 머무는 서버의 현재 상태를 확인해요.",
  actions: {
    checkAgain: "다시 확인하기",
    checkAgainLabel: "서버 상태 다시 확인하기",
    checking: "확인하고 있어요",
  },
  summary: {
    checkingTitle: "서버 상태를 확인하고 있어요",
    checkingBody: "현재 연결과 데이터 준비 상태를 불러오는 중이에요.",
    readyTitle: "서버에 연결됐어요",
    readyBody:
      "이 기기에서 개인 서버에 닿을 수 있고, 데이터를 사용할 준비도 끝났어요.",
    attentionTitle: "서버에 연결됐지만 준비가 더 필요해요",
    attentionBody:
      "아래 항목을 확인해 주세요. 준비가 끝나면 다시 확인할 수 있어요.",
    disconnectedTitle: "서버에 연결하지 못했어요",
    disconnectedBody: "서버 주소와 실행 상태를 확인한 뒤 다시 시도해 주세요.",
  },
  groups: {
    readinessTitle: "준비 상태",
    readinessDescription: "앱과 데이터가 지금 사용할 수 있는지 확인해요.",
    serverTitle: "서버 정보",
    serverDescription: "현재 연결 대상과 마지막으로 받은 정보를 보여줘요.",
  },
  checks: {
    appResponse: "앱 응답",
    appReady: "정상적으로 응답해요",
    appDisconnected:
      "응답을 받지 못했어요. 서버 실행 상태를 확인한 뒤 다시 시도해 주세요.",
    configuration: "앱 준비",
    configurationReady: "필요한 설정을 불러왔어요",
    configurationAttention:
      "앱 설정을 불러오지 못했어요. 서버 설정을 확인한 뒤 다시 시도해 주세요.",
    dataStore: "데이터 저장소",
    dataStoreReady: "데이터를 읽고 쓸 준비가 됐어요",
    dataStoreAttention:
      "데이터 저장소를 준비하지 못했어요. 서버에서 저장소 연결을 확인해 주세요.",
    dataStructure: "데이터 구조",
    dataStructureReady: "현재 앱과 같은 구조를 사용해요",
    dataStructureAttention:
      "데이터 구조를 준비하지 못했어요. 잠시 후 다시 확인해 주세요.",
    checking: "확인 중",
    ready: "준비됨",
    attention: "확인 필요",
    disconnected: "연결 안 됨",
  },
  details: {
    address: "서버 주소",
    localServer: "로컬 테스트 서버",
    build: "서버 버전",
    structureVersion: "데이터 구조 버전",
    checkedAt: "마지막 확인",
    waiting: "확인 전",
  },
  liveRegion: {
    checking: "서버 상태를 다시 확인하고 있어요.",
    ready: "서버 연결과 데이터 준비 상태를 확인했어요.",
    attention:
      "서버에 연결됐지만 준비가 필요한 항목이 있어요. 아래 상태를 확인해 주세요.",
    disconnected:
      "서버에 연결하지 못했어요. 서버 실행 상태를 확인한 뒤 다시 시도해 주세요.",
  },
  footer: "이 화면은 서버가 직접 보낸 상태만 표시해요.",
} as const;
