---
title: Vela 설치
description: Vela는 브라우저에서 돌아갑니다 — 설치도, 앱스토어도 필요 없습니다. 웹 지갑을 열거나, 패스키에 필요한 기기 조건을 먼저 확인하세요.
---

# Vela 설치

Vela는 **브라우저 안에서** 돌아갑니다. 내려받을 것도 없고 앱스토어를 거칠 필요도
없습니다. 웹 지갑을 열면 1분도 안 되어 지갑을 만들거나 복구할 수 있습니다.

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">웹 지갑 열기 →</a>

같은 코드로 만든 같은 지갑이 iOS와 안드로이드에서도 돌아갑니다. **네이티브 모바일
앱은 곧 출시됩니다.** 출시되어도 계정은 특정 앱이 아니라 온체인에 있으므로, 패스키와
지갑이 그대로 따라옵니다.

## 기기에 필요한 것

Vela는 **패스키**(WebAuthn)로 서명하므로, 이를 지원하는 기기와 브라우저가 필요합니다.
사실상 최근 몇 년 사이의 환경이면 대부분 됩니다.

| 플랫폼 | 패스키 지원 | 동기화 주체 |
| -------- | --------------- | --------- |
| 아이폰 / 아이패드 / 맥 | iOS·iPadOS 16 이상, 최신 Safari | iCloud 키체인 |
| 안드로이드 | 안드로이드 9 이상, 최신 Chrome | Google 비밀번호 관리자 |
| 데스크톱 | 최신 Chrome, Edge, Safari, Firefox | 사용 중인 플랫폼의 패스키 저장소 |

지갑이 새 기기로 따라오게 하려면, 플랫폼의 패스키 동기화를 켜 두세요(애플은 iCloud
키체인, 안드로이드·Chrome은 Google 비밀번호 관리자). 작동 방식은
[복구와 로그인](/ko/docs/recovery)에 있습니다.

## 공식 주소는 이 둘뿐

Vela가 오픈소스라는 점이 핵심이지만, 동시에 지금 보고 있는 것이 진짜인지 스스로
확인해야 한다는 뜻이기도 합니다. 공식 주소는 다음 둘뿐입니다.

- **getvela.app** — 이 사이트
- **wallet.getvela.app** — 지갑

“Vela를 설치하라”며 다른 곳으로 보내는 것이 있다면, 멈추고 이 둘과 대조하세요.
코드는 [github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet)
에 공개되어 있습니다.

다음: [지갑 만들기](/ko/docs/create-wallet).
