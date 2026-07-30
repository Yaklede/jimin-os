export const meetingSpeakerRecoveryCopy = {
  speakerNamesDescription:
    "이름이 빠졌다면 발언자를 추가한 뒤 해당 발언에 지정해 주세요.",
  addSpeaker: "발언자 추가",
  speakerReviewTitle: "발언자를 확인해 주세요",
  speakerReviewDescription: (participants: number, speakers: number) =>
    `참석자는 ${participants}명이고 회의록에서 구분된 발언자는 ${speakers}명이에요. 실제로 두 명 이상이 말했다면 발언자를 추가하고 발언 구간을 나눠 주세요.`,
  reviewSpeakers: "발언자 확인하기",
} as const;
