---
title: 감사와 알려진 문제
description: "Vela가 의존하는 모든 컨트랙트, 누가 어느 버전을 감사했는지, 감사받은 버전이 실제 배포된 버전인지, 우리가 지켜보는 미해결 발견 사항, 그리고 감사를 전혀 받지 않은 것을 정리했습니다."
source: c4c50ad89f2f
---

"감사를 받았다"는 말은 특정 버전의 특정 코드에 대한 주장입니다. 그래서 이 페이지는 보고서,
커밋, 배포 주소를 구체적으로 제시하고, 감사를 **받지 않은** 것도 함께 적습니다. 그 목록도
똑같이 중요하기 때문입니다.

마지막 검토: 2026년 9월 22일. 오류를 발견하면 알려 주세요. 고치겠습니다.

## 자금 경로

자금에 손댈 수 있는 컨트랙트는 모두 제3자 코드의 공식 배포이며, 공개된 검토 보고서가
있습니다.

### Safe v1.4.1 — 계정 그 자체

지갑은 SafeL2 싱글톤과 SafeProxyFactory를 쓰는
[Safe](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) 프록시입니다. 묶음 거래는
MultiSend를 거칩니다.

[Ackee Blockchain이 Safe v1.4.0을 감사했습니다](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(최종 보고서 2023년 3월 16일, 수정 검토 3월 28일). 발견 사항은 11건으로 치명(critical)이나
높음(high) 등급은 없었고, 중간(medium) 등급 2건은 코드를 바꾸지 않고 인지(acknowledged) 처리되었습니다.
감사 범위는 SafeL2, SafeProxyFactory, CompatibilityFallbackHandler, MultiSendCallOnly,
SignMessageLib였습니다. v1.4.1은 v1.4.0과 기능상 한 줄만 다른데, 모듈 설정 과정의 ERC-4337
호환성 수정입니다([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)).
Safe는 Ackee와 협의해 재감사가 필요 없다고 결론 내렸습니다. MultiSend는 v1.3.0 이후 로직이
바뀌지 않았고, v1.3.0은
[G0 Group이 감사했습니다](https://github.com/safe-global/safe-smart-account/tree/main/docs).
모든 주소는 [safe-deployments](https://github.com/safe-global/safe-deployments)와 일치합니다.
핵심 컨트랙트는 [Safe Foundation 버그 바운티](https://docs.safefoundation.org/security/bug-bounty)
대상이며, 최고 등급 포상금은 최대 100만 달러입니다.

2025년 바이비트 사건은 컨트랙트 결함이 아닙니다. 공격자는 Safe 웹 인터페이스가 불러오는
JavaScript를 변조했고, Safe의
[포렌식 조사 성명](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)은
컨트랙트에서 취약점을 찾지 못했습니다. [이 사건을 다룬 페이지](/ko/docs/bybit-attack)에서 같은
유형의 공격이 왜 우리 것을 포함한 모든 지갑 인터페이스와 관련이 있는지 설명합니다.

### Safe4337Module v0.3.0 — ERC-4337 어댑터

`0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`에 배포되어 있고(Sourcify 완전 일치), Safe의 폴백
핸들러로도 설정됩니다. 세 차례 검토를 받았습니다.
[보고서](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md):

- **Ackee Blockchain**, 최종 보고서 2024년 3월: 경고 1건(컴파일러 옵티마이저 사용)이 인지
  처리되었고, 그보다 높은 등급의 미해결 사항은 없습니다.
- **Certora**, 2026년 8월: **중간** 등급 1건. 인지 처리되었지만 v0.3.0에서 **수정되지
  않았습니다**. *권한 변경이 같은 번들 안에서 이미 검증을 통과한 이후 UserOperation을 무효로
  만들지 않는다*는 내용입니다. 아래 "알려진 문제"를 참고하세요.
- **Nethermind**, 2026년 8월: 발견 사항 없음.

지갑을 배포할 때 이 모듈을 활성화하는 SafeModuleSetup v0.3.0(`0x2dd6…5b47`)은 Certora와
Nethermind 검토 범위에 포함되었습니다.

이 모듈에는 공개된 문제가 하나 있었습니다. v0.1.0은 `initCode`와 `paymasterAndData`에
서명하지 않아 가스 그리핑 경로가 있었고,
[v0.2.0에서 수정되었습니다](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module).
Safe에 따르면 v0.1.0은 테스트넷 밖에서 쓰이지 않았습니다. Vela는 v0.3.0을 EntryPoint v0.7,
Safe 1.4.1과 함께 쓰며, 이는 모듈 릴리스에 명시된 구성입니다.

### Safe 패스키 모듈 v0.2.1 — 서명자

첫 번째 키는 `0x94a4F6affBd8975951142c3999aEAB7ecee555c2`의 **SafeWebAuthnSharedSigner**가
검증합니다. "공유(shared)"란 Safe 싱글톤처럼 컨트랙트 배포를 공유한다는 뜻이지, 키를
공유한다는 뜻이 아닙니다. 각 Safe는 자신의 P-256 공개 키를 자기 스토리지에 저장합니다.

추가한 키는 각자 서명자 컨트랙트를 가집니다. 이 컨트랙트는
`0x1d31F259eE307358a26dFb23EB365939E8641195`의 **SafeWebAuthnSignerFactory**가 만들며,
`0x4E27b51350e6c2083EE19011120F50DAfEc5CA50`의 **SafeWebAuthnSigner 싱글톤**을 가리키는
프록시입니다.

v0.2.1의 이 컨트랙트들을 다룬 검토
([보고서](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)):

- [Hats Finance 감사 대회](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
  (2024년 6~7월): 높음·중간 등급 없음, 낮음 등급 3건은 모두 수정.
- **Certora**의 릴리스 커밋 검토: 새로운 발견 사항 없음. (그 전의 v0.2.0 감사에는 공유
  서명자가 아직 감사되지 않았다고 적혀 있습니다. 공유 서명자는 그 감사 이후에 추가되었습니다.)
- **Nethermind**, 2026년 8월: 발견 사항 없음.

릴리스 이후 컨트랙트 수준의 취약점이 공개된 적은 없으며, 패스키 컨트랙트도 Safe Foundation
바운티 대상입니다.

패스키 서명은 체인의 **EIP-7951 / RIP-7212** 프리컴파일이 검증하며, 대체 검증기는 없습니다. 네트워크를
활성화하기 전에 앱이 실제 서명으로 프리컴파일을 확인합니다. 주의할 점이 두 가지 있습니다.
최초의 RIP-7212 명세에는 경계 조건의 결함이 있으며
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951)이 이를 수정합니다(어차피 실패해야 하는
입력에만 영향을 주고, 올바른 형식의 WebAuthn 서명에는 영향이 없습니다). 그리고 한 번의 점검으로
체인 구현이 달라질 수 있는 모든 경우를 잡아낼 수는 없습니다.

### EntryPoint v0.7 — 오퍼레이션 실행

`0x0000000071727De22E5E9d8BAf0edAc6f37da032`에 배포된
[공식 v0.7.0 릴리스](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)입니다.
이더리움 재단의 의뢰로
[OpenZeppelin이 감사했습니다](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
(2024년 1월). 치명·높음 등급은 없고 중간 등급 5건이 있었으며, 발견 사항 24건이 모두
해결되었습니다. 수정 검토 커밋은 릴리스와 일치합니다. 이더리움 재단의
[ERC-4337 버그 바운티](https://docs.erc4337.io/community/bug-bounty) 대상입니다(최대 25만
달러).

## 지켜보고 있는 알려진 문제

### 한 번들 안에서의 권한 변경(Safe4337Module, Certora M-01)

EntryPoint는 번들 안의 모든 오퍼레이션을 먼저 검증한 뒤에 실행합니다. 그래서 어떤 오퍼레이션이
소유자 하나를 제거하더라도, 같은 번들 뒤쪽에 있는 그 소유자가 서명한 오퍼레이션은 여전히 검증을
통과하고 실행됩니다. Safe는 이를 인지했지만 v0.3.0을 바꾸지 않았습니다.

Vela 앱은 소유자 변경을 만들지 않으므로 Vela가 이 문제를 일으키는 일은 없습니다. 그래도
중요합니다. dApp이 지갑에 소유자를 바꾸라고 요청할 수 있고(아래 "빈틈" 참고), 다른 Safe 도구로
유출된 키를 제거하는 경우에도 같은 번들 안에서 그 키가 차단된다고 기대할 수 없습니다.

### 서명된 오퍼레이션 가로채기(v0.9 이전 EntryPoint)

2026년 2월, 연구자들이 v0.7을 포함해 v0.9 이전의 모든 EntryPoint에 영향을 주는 그리핑·검열
경로를 [공개했습니다](https://erc4337.substack.com/p/improving-useroperation-execution). 서명된
오퍼레이션이 체인에 올라가기 전에 손에 넣은 사람은 그것을 자신이 통제하는 호출 안에서 실행해
내부 실행을 강제로 되돌릴 수 있습니다. 오퍼레이션은 실패하고 다시 서명해야 합니다. (Vela의 인밴드
수수료 방식에서는 수수료 전송도 함께 되돌려지므로, 가스는 사용자가 아니라 릴레이가 떠안습니다.)
재진입 방지가 걸린 컨트랙트를 호출하거나 일시적인 상태 때문에 되돌려질 수 있는 오퍼레이션이
영향을 받으며, 단순 전송은 영향이 없습니다. 출금 흐름을 노려 반복하면 한동안 자금을 쓰지 못하게
만들 수 있습니다. 서명을 위조하거나 자금의 행선지를 바꿀 수는 없습니다.

Vela의 릴레이는 공유 멤풀을 거치지 않고 오퍼레이션을 직접 제출하지만, 대기 중인 `handleOps`
트랜잭션은 공개 멤풀에서 여전히 보이므로 노출을 줄일 뿐 없애지는 못합니다. 수정은 EntryPoint
v0.9(2025년 11월)에만 있고, v0.7은 패치할 수 없습니다. 이전은 Safe의 4337 모듈이 v0.9를
지원하는지에 달려 있으며, 이전하게 되면 이 페이지에 적겠습니다.

### Vela 자체 방어의 빈틈

컨트랙트 발견 사항은 아니지만, 지갑이 생각보다 덜 보호해 주는 부분입니다. 모두 수정 대상으로
추적하고 있습니다.

- **지갑이 자기 자신을 호출하는 것을 막지 않습니다.** dApp은 내 Safe에 `enableModule`,
  `addOwnerWithThreshold`, `setFallbackHandler`, `setGuard`를 요청할 수 있고, 이 중 어느
  것이든 한 번 서명하면 계정을 넘겨주게 됩니다. Vela는 이런 호출을 디코딩하지만 막지는
  않습니다. 대상이 내 주소인 요청은 모두 거절하세요.
- **승인 가드는 "무제한" 금액만 막습니다**(2^200 이상, Permit2는 2^152 이상). 금액이 크지만
  유한한 승인, 서명 방식의 승인, NFT `setApprovalForAll`은 주의 표시만 할 뿐 막지 않습니다.
- **가져온 디스크립터는 인증되지 않습니다.** 체인 데이터 서버에서 가져온 디스크립터는
  컨트랙트와 일치하기만 하면 "검증됨"으로 표시되며, 그 서버를 믿을 수 있는 만큼만 믿을 수
  있습니다.
- **독립 서명 페이지는 아직 어떤 앱과도 연결되어 있지 않습니다.**
- **네트워크 확인에서 Safe의 패스키 서명자 팩토리를 확인하지 않습니다.** 두 번째부터 일곱 번째
  키에는 이 팩토리가 필요하며, 팩토리 없이 추가한 네트워크에서는 첫 번째 키만 서명할 수
  있습니다.
- **웹사이트가 패스키와 같은 도메인에서 제3자 분석 스크립트를 불러옵니다.** 사이트는
  Permissions-Policy 헤더로 자기 페이지에서 패스키를 쓰지 못하게 막고, 키를 다루는 페이지에서는
  이 스크립트를 불러오지 않습니다.

## 감사를 받지 않은 것

- **Vela 자체 컨트랙트.** `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`의
  [공개 키 레지스트리](https://github.com/mondaylabsltd/p256-index/tree/main/contracts)(Gnosis,
  이더리움과 Base에서도 같은 주소), `0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf`의 최초
  레지스트리 배포(이 주소는 모든 등록 서명의 도메인에 들어갑니다), 그리고 이들이 대체한 예전
  인덱스(`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`, 읽기 전용 기록)입니다. 모두 감사를 받지
  않았습니다. 자금을 보관하지 않고, 소유자가 없으며, 업그레이드할 수 없습니다. 권한을 주는
  계층이 아니라 지갑을 찾아 주는 계층입니다. 자금을 쓸 권한은 Safe에 설정된 키에서만 나옵니다.
  현실적으로 최악의 실패는 새 기기에서 지갑을 찾기 어려워지는 것이지, 자금이 움직이는 것이
  아닙니다.
- **Multicall3.** README에
  ["This contract is unaudited."](https://github.com/mds1/multicall3)라고 적혀 있습니다. Vela는
  잔액, 토큰 정보, 가격 견적 같은 일괄 읽기에만 쓰며, 승인이나 자금과는 절대 엮지 않습니다.
- **결정적 배포 컨트랙트**(Arachnid의 CREATE2 프록시와 Safe의 싱글톤 팩토리). 생태계 표준이고
  상태가 없지만 정식 감사는 받지 않았습니다. 이것들이 없으면 Vela의 네트워크 확인은 실패로
  처리합니다. 다만 그 주소에 코드가 있는지를 확인할 뿐, 바이트 단위로 일치하는지는 확인하지
  않습니다.
- **Tempo.** 24개 내장 네트워크 중 하나로, 네이티브 코인이 없어서 Vela는 pathUSD
  스테이블코인으로 가스를 냅니다. 2026년 9월 현재 Tempo의
  [보안 정책](https://github.com/tempoxyz/.github/blob/main/SECURITY.md)에는 프로토콜이 아직
  감사 중이며 운영 중인 버그 바운티가 없다고 적혀 있습니다. Tempo에 보관한 자금과 그곳에서 내는
  가스에는 이런 체인 수준의 위험이 따릅니다. 목록에서 가장 새롭고 가장 덜 검증된 체인으로
  보세요.
- **Vela 자체.** 앱, 백엔드 서비스, 위의 컨트랙트는 제3자 감사를 받지 않았고, 예정된 감사도
  없습니다. 이 페이지에서 가장 큰 단서입니다. 자세한 내용은
  [Vela is in alpha](/blog/vela-is-in-alpha)에 있습니다. 적은 금액부터 시작하고, 코드를 읽어
  보세요.

## 직접 확인하기

아래 주소는 모두 공개된 공식 배포입니다.
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments),
[EntryPoint 릴리스](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)와
대조해 확인하세요.

| 컨트랙트                                  | 주소                                         |
| ----------------------------------------- | -------------------------------------------- |
| SafeL2 싱글톤 v1.4.1                      | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1                   | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| MultiSend v1.4.1                          | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| CompatibilityFallbackHandler v1.4.1 ¹     | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| SafeModuleSetup v0.3.0                    | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0                     | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1           | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| SafeWebAuthnSignerFactory v0.2.1          | `0x1d31F259eE307358a26dFb23EB365939E8641195` |
| SafeWebAuthnSigner 싱글톤 v0.2.1          | `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` |
| EntryPoint v0.7                           | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3                                | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| 공개 키 레지스트리(Vela, 감사 안 됨)      | `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` |

¹ 네트워크를 추가할 때 확인합니다. 실제로 Safe는 4337 모듈을 폴백 핸들러로 씁니다.
