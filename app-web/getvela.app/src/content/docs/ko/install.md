---
title: Vela 설치
description: "웹, 브라우저 확장 프로그램, 데스크톱, 휴대폰까지 Vela를 쓰는 모든 방법. 각각의 비용과 할 수 있는 일, 기기에 필요한 조건을 정리했습니다."
source: f88fdfac1001
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Vela 설치

같은 지갑이 여러 곳에서 돌아가고, 어디서 열든 같은 키로 같은 주소가 열립니다. 필요에
맞게 고르면 되고, 여러 개를 함께 써도 됩니다. 내려받기는 [Vela 받기](/ko/get-started)에
있습니다.

| | 무엇인가 | 비용 | 상태 |
| --- | --- | --- | --- |
| **웹** | 최신 브라우저에서 여는 [wallet.getvela.app](https://wallet.getvela.app/) | 무료 | 이용 가능 |
| **브라우저 확장 프로그램** | 도구 모음에 두는 지갑. dApp에 연결됩니다 | 무료 | 내려받아 직접 로드. 아직 Chrome 웹 스토어에 없음 |
| **데스크톱** | macOS, Windows, Linux용 네이티브 앱 | 무료 | Vela 받기 페이지나 GitHub에서 내려받기 |
| **iPhone, Android** | 네이티브 앱 | 스토어에서 1회 구매 | 아직 스토어에 없음. 소스에서 직접 빌드 가능 |

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">웹 지갑 열기 →</a>

## 웹

설치할 것이 없습니다. [wallet.getvela.app](https://wallet.getvela.app/)을 열고 지갑을
만들거나 로그인하면 바로 쓸 수 있습니다. 계정 목록은 이 브라우저에 저장됩니다. 다른
기기에서는 키 중 하나로 다시 로그인하면 됩니다.

## 브라우저 확장 프로그램

Chromium 기반 브라우저인 Chrome, Edge, Brave에서 쓸 수 있습니다(Chrome 116 이상).
지갑을 도구 모음에 두고, dApp이 지갑에 바로 연결할 수 있게 해 줍니다. Chrome 웹 스토어에
올라가기 전까지는 다음과 같이 설치합니다.

1. [Vela 받기](/ko/get-started)에서 확장 프로그램을 내려받아, 계속 둘 폴더에 압축을
   풉니다. 브라우저는 그 폴더에서 확장 프로그램을 실행합니다.
2. `chrome://extensions`를 열고 **개발자 모드**를 켭니다.
3. **압축해제된 확장 프로그램을 로드합니다**를 누르고 그 폴더를 선택합니다.

같은 지갑입니다. 확장 프로그램과 웹 지갑은 같은 `getvela.app` 패스키를 쓰기 때문에, 같은
키로 같은 주소가 열립니다.

## 데스크톱

창 안에 웹 페이지를 띄운 것이 아니라 네이티브 앱입니다. **Windows** 10·11(x64, ARM),
**macOS** 11 이상, **Linux**(.deb, .rpm, Flatpak / x64, ARM)를 지원합니다.

- **Windows**에서는 설치 프로그램에 아직 코드 서명이 없어서 "Windows의 PC 보호" 경고가
  뜹니다. **추가 정보**를 누른 다음 **실행**을 누르세요.
- **macOS** 빌드는 별도 단계에서 Apple의 서명과 공증을 거치므로 다른 플랫폼보다 늦게
  나올 수 있습니다. Mac 버튼에 "곧 제공"이라고 표시되어 있다면, 공증을 마친 가장 최근
  Mac 빌드를 GitHub 릴리스 페이지에서 받을 수 있습니다.
- **Linux**에서 USB 보안 키를 쓰려면 시스템이 앱에 키 접근을 허용해야 합니다. .deb와
  .rpm 패키지는 이 규칙을 자동으로 설치합니다.

macOS와 Windows용 데스크톱 앱에는 dApp을 쓰기 위한 내장 브라우저가 있습니다. 모든
패키지의 체크섬은
[GitHub 릴리스 페이지](https://github.com/mondaylabsltd/vela-wallet/releases)에 있습니다.

## iPhone과 Android

iOS 17.4 이상, Android 10 이상에서 돌아가는 네이티브 앱입니다. App Store와 Google
Play에서 1회 구매로 판매할 예정이며, **아직 스토어에는 없습니다**. 코드가 공개되어 있으니
무료로 직접 빌드할 수 있습니다. 차이는 하나입니다. 직접 서명한 빌드로는 휴대폰 자체의
패스키로 getvela.app 지갑에 서명할 수 없습니다. 다른 휴대폰으로 스캔하는 방법과 USB 보안
키는 쓸 수 있습니다. [앱 직접 빌드하기](/ko/docs/self-hosting#web-app)를 참고하세요.

## dApp에서 Vela 쓰기

<span id="dapps"></span>

dApp은 다른 브라우저 지갑에 연결할 때와 같은 방식(EIP-1193, EIP-6963)으로 Vela에
연결합니다.

- 데스크톱 브라우저에서는 **Vela 브라우저 확장 프로그램**으로
- **데스크톱 앱**(macOS, Windows), **iPhone 앱**, **Android 앱** 안에서는 각 앱의 내장
  브라우저로

wallet.getvela.app의 웹 지갑은 dApp에 연결하지 않으며, WalletConnect도 지원하지
않습니다. dApp이 보내는 요청은 모두 서명하기 전에 디코딩해서 보여 줍니다.
[클리어 서명](/ko/docs/clear-signing)을 참고하세요.

## 기기에 필요한 것

Vela는 **패스키**로 서명합니다. 최근 몇 년 사이에 나온 기기라면 거의 모두 패스키를
지원합니다.

| 기기 | 지원 조건 |
| --- | --- |
| iPhone, iPad, Mac | iOS / iPadOS 16 이상, 최신 Safari 또는 Chrome이 있는 macOS |
| Android | Google Play 서비스가 있는 최신 Android, 또는 USB 보안 키 |
| Windows | Chrome 또는 Edge에서 Windows Hello, 또는 보안 키 |
| Linux | 보안 키, 또는 가까이 있는 휴대폰(QR 코드 스캔) |

기기 자체에 패스키를 저장할 수 없다면 다른 휴대폰이나 하드웨어 보안 키를 쓰세요. 앱마다
어떤 종류의 키를 지원하는지는 [서명 키와 보안 키](/ko/docs/signers)에 정리되어 있습니다.

## 공식 주소는 이것뿐입니다

- **getvela.app** — 이 사이트, 그리고 내려받기
- **wallet.getvela.app** — 웹 지갑
- **github.com/mondaylabsltd** — 코드와 릴리스 패키지

<Callout type="warning" title="설치하기 전에 확인하세요">
"Vela 설치"나 "지갑 인증"을 하라며 다른 곳으로 안내하는 것이 있다면 멈추세요. Vela는
시드 구문을 요구하지 않습니다. 애초에 시드 구문이 없습니다.
</Callout>

다음: [지갑 만들기](/ko/docs/create-wallet).
