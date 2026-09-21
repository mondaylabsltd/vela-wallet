---
title: 소개
description: "Vela가 무엇인지 여섯 줄로 정리하고, 사람들이 흔히 묻는 질문마다 답이 있는 곳을 바로 연결해 두었습니다."
source: 5aa87a77142a
---

# Vela 문서

Vela는 시드 구문이 없는 **이더리움 및 EVM 네트워크용 자기 수탁형 지갑**입니다. 지갑은
수정하지 않은 [Safe](/ko/docs/account-contract) 스마트 계정이고, 서명은 **패스키**로
합니다. 휴대폰이나 컴퓨터에 있는 패스키, 다른 휴대폰, 하드웨어 보안 키 모두 쓸 수 있습니다.

- **키는 최대 일곱 개.** 지갑을 만들 때 정하며, 그중 어느 하나로도 서명할 수 있습니다.
  Vela는 키를 갖고 있지 않고, 지갑에 대한 어떤 권한도 없습니다.
- **24개 네트워크, 하나의 주소.** 요건을 충족하는 EVM 네트워크라면 직접 추가할 수도
  있습니다.
- **읽고 나서 서명합니다.** 거래는 알기 쉬운 말로 풀어서 보여 주고, 풀지 못한 부분은
  그렇다고 표시합니다.
- **우리 없이도 돌아갑니다.** 앱과 서비스는 오픈소스이고 다른 것으로 바꿀 수 있습니다.
  한계는 [셀프 호스팅 가이드](/ko/docs/self-hosting)에 적어 두었습니다.
- **알파 단계.** 실제로 작동하고 실제 자금을 담고 있지만, 아직 초기입니다. 적은 금액부터
  시작하세요. [여기서 말하는 알파의 의미](/blog/vela-is-in-alpha)

## 답 찾기

| 알고 싶은 것 | 볼 곳 |
| --- | --- |
| 어떤 앱을 설치할지, 비용은 얼마인지 | [Vela 설치](/ko/docs/install) · [Vela 받기](/ko/get-started) |
| 지갑을 만드는 방법, 어떤 키를 쓸지 | [지갑 만들기](/ko/docs/create-wallet) · [서명 키와 보안 키](/ko/docs/signers) |
| 휴대폰을 잃어버렸거나 패스키를 삭제했을 때 할 일 | [복구와 로그인](/ko/docs/recovery) |
| 나중에 키를 추가하거나 바꿀 수 있는지 | [서명 키와 보안 키](/ko/docs/signers) |
| 거래 수수료가 왜 그 금액인지 | [네트워크와 수수료](/ko/docs/networks-and-fees) |
| 내 체인을 지원하는지, 추가하려면 어떻게 하는지 | [네트워크와 수수료](/ko/docs/networks-and-fees) · [체인 설정](/ko/chain-setup) |
| dApp에서 Vela를 쓰는 방법 | [Vela 설치 → dApp](/ko/docs/install#dapps) |
| 실제로 무엇에 서명하는지 확인하는 방법 | [클리어 서명](/ko/docs/clear-signing) · [바이비트 해킹 사건](/ko/docs/bybit-attack) |
| getvela.app이 내려가면 어떻게 되는지 | [셀프 호스팅 가이드 → getvela.app이 없을 때](/ko/docs/self-hosting#if-getvela-app-disappears) |
| 전부 직접 운영하는 방법 | [셀프 호스팅 가이드](/ko/docs/self-hosting) |
| Vela가 감사를 받았는지 | [감사와 알려진 문제](/ko/docs/security-audits) |
| 내 지갑에 관해 무엇이 공개되는지 | [지갑 만들기 → 공개되는 정보](/ko/docs/create-wallet#what-is-public) · [개인정보 처리방침](/privacy) |

## 더 깊이 읽기

- [Vela를 만든 이유](/ko/docs/why-vela) — 만든 과정과, 우리가 택한 트레이드오프.
- [백서](/ko/docs/whitepaper) — 아키텍처, 그리고 정확히 무엇을 신뢰하게 되는지.
- [계정 컨트랙트](/ko/docs/account-contract) — 자금을 담고 있는 컨트랙트.

Vela를 만들어 가는 과정은 [블로그](/blog)에 적고 있습니다.
