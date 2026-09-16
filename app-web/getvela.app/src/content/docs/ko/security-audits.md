---
title: 감사와 알려진 문제
description: Vela가 의존하는 모든 온체인 컨트랙트, 누가 감사했는지, 감사된 버전이 실제 배포된 것과 일치하는지, 그리고 무엇이 전혀 감사되지 않았는지.
---

“감사됨”은 특정 코드의 특정 버전에 대한 주장입니다. 그래서 이 페이지는 그 단어를
흔드는 대신, 보고서 자체와 배포 주소 자체, 그리고 감사된 버전과 배포된 버전의 차이를
제시합니다. 아울러 무엇이 **감사되지 않았는지**도 적습니다. 그 목록도 앞의 것만큼
무게를 지니기 때문입니다.

마지막 확인: 2026년 8월. 여기서 오류를 찾으면 알려 주세요. 고치겠습니다.

## 자금이 지나는 길

당신의 돈에 닿을 수 있는 컨트랙트는 네 겹입니다. 네 겹 모두 공개된 감사가 있는 제3자
컨트랙트이며, 배포 주소는 모두 공식 표준 배포입니다.

### Safe v1.4.1 — 계정 본체

당신의 지갑은 [Safe](https://github.com/safe-global/safe-smart-account) 프록시입니다.
SafeL2 싱글턴, 프록시 팩토리, 호환 폴백 핸들러, 그리고 배치용 MultiSend.

[Ackee Blockchain이 Safe v1.4.0을 감사](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
했습니다(최종 보고 2023년 3월). 지적 11건, 크리티컬과 하이는 없음. 우리가 배포하는
v1.4.1과 감사된 v1.4.0의 차이는 ERC-4337 호환을 위한 한 줄 수정뿐입니다
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)).
MultiSend의 로직은
[G0 Group이 감사한 v1.3.0](https://github.com/safe-global/safe-smart-account/tree/main/docs)
이후 바뀌지 않았습니다. 모든 주소가
[safe-deployments](https://github.com/safe-global/safe-deployments)의 표준 배포와
일치하며, 이 컨트랙트들은
[Safe Foundation 버그 바운티](https://docs.safefoundation.org/security/bug-bounty)
대상입니다(크리티컬 최대 100만 달러).

감사가 다루지 않는 것이 하나 있습니다. 2025년 바이비트 사건입니다. 그 공격이 장악한
것은 Safe 공식 웹 프런트엔드의 빌드 파이프라인이었지 컨트랙트가 아니었습니다.
[공식 포렌식 결론](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
은 Safe 스마트 컨트랙트에 취약점이 없었다고 밝혔습니다. 우리는 이를 웹과 운영 계층에
대한 교훈으로 읽습니다. 바로 그 계층이, 당신이 우리를 엄격히 봐야 할 곳이기도 합니다.

### Safe4337Module v0.3.0 — ERC-4337 어댑터

`0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`에 배포. v0.3.0의 표준 주소입니다
(Sourcify 정확 일치 — 온체인 바이트코드가 곧 감사된 코드).
[Ackee Blockchain이 감사](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)
했고(최종 보고 2024년 3월), 정보 수준을 넘는 미해결 지적은 없습니다. 우리가 쓰는
v0.3.0 + EntryPoint v0.7 + Safe 1.4.1 이상 조합은 감사와 릴리스 노트가 기술한 바로
그 구성입니다.

이 모듈의 이력에는 공개된 문제가 하나 있습니다. v0.1.0(2023년)은 `initCode`와
`paymasterAndData`에 서명하지 않아 가스 그리핑 경로가 있었습니다. 이는
[v0.2.0에서 수정](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module)
되었고, v0.1.0이 테스트넷 밖으로 나간 적은 없습니다. 우리가 쓰는 v0.3.0은 그 수정을
물려받았습니다.

### SafeWebAuthnSharedSigner v0.2.1 — 패스키 서명기

`0x94a4F6affBd8975951142c3999aEAB7ecee555c2`에 배포. v0.2.1의 표준 주소입니다
(Safe 싱글턴 팩토리를 통해 모든 체인에서 동일).

“shared(공유)”가 뜻하는 것과 뜻하지 않는 것: 공유되는 것은 *컨트랙트 배포*이며,
Safe 싱글턴이 공유되는 것과 같습니다. 당신의 키는 공유되지 않습니다. 각 Safe가
delegatecall로 `configure()`를 호출해 자기 P-256 공개 키를 자기 스토리지에
저장합니다. 서명기 인스턴스 하나가 나타내는 것은 Safe당 정확히 하나의 패스키이고,
다른 사람의 Safe가 당신 것을 쓸 수는 없습니다.

여기서는 버전이 중요합니다. v0.2.0 감사는 공유 서명기가 범위 밖이라고
[명시](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.0/audit.md)
했습니다 — 그때 그 컨트랙트는 아직 없었습니다. 우리가 배포한 것을 다루는 감사는
v0.2.1의 것들입니다.
[Hats Finance 감사 대회](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
(2024년 6~7월: 하이 0, 미디엄 0, 로우 3건 — 모두 수정)와
[Certora의 릴리스 커밋 리뷰](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)
에서 새로운 지적은 없었습니다. 출시 이후 컨트랙트 수준의 취약점은 공개되지 않았고,
패스키 관련 컨트랙트도 Safe Foundation 바운티 대상입니다.

Safe 자체 문서는 자격 증명 하나를 계정의 유일한 키로 삼기보다, 패스키 소유에 복구
경로를 함께 두라고 권합니다. Vela가 이를 어떻게 다루는지는
[복구와 로그인](/ko/docs/recovery)에 있습니다.

온체인 P-256 검증은 RIP-7212 프리컴파일을 직접 쓰며, Solidity 폴백 검증기는 없습니다.
네트워크를 활성화하기 전에 앱이 실제 서명으로 프리컴파일을 시험하고, 검증에 실패하면
그 네트워크를 거절합니다. 솔직한 단서 둘. 최초의 RIP-7212 명세에는 경계 사례의 결함이
있었고 [EIP-7951](https://eips.ethereum.org/EIPS/eip-7951)이 그것을 고치려고
쓰였습니다(형식이 올바른 WebAuthn 서명에는 영향이 없습니다). 그리고 한 번의 시험으로,
어떤 체인의 구현이 드문 실행 맥락에서 어긋날 수 있는 모든 경우를 잡을 수는 없습니다.

### EntryPoint v0.7 — ERC-4337의 진입점

`0x0000000071727De22E5E9d8BAf0edAc6f37da032`에 배포.
[v0.7.0 표준 배포](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)입니다.
[OpenZeppelin이 감사](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
했고(이더리움 재단 의뢰, 2024년 1월), 크리티컬 0, 하이 0, 미디엄 5건으로 모두
해결되었으며, 감사된 커밋이 곧 배포된 릴리스입니다. EntryPoint v0.7.0은 이더리움
재단의 [ERC-4337 버그 바운티](https://docs.erc4337.io/community/bug-bounty)
대상입니다(최대 25만 달러).

## 우리가 지켜보는 알려진 문제

### EntryPoint 그리핑 경로

2026년 2월, Trust Security의 보안 연구자들이 v0.9 이전의 모든 EntryPoint — 우리가
쓰는 v0.7 포함 — 에 영향을 주는 그리핑·검열 경로를
[공개](https://erc4337.substack.com/p/improving-useroperation-execution)했습니다.
서명된 UserOperation을 채굴 전에 가로챌 수 있는 공격자는 그것을 자신이 통제하는 콜
프레임 안에서 실행해 내부 실행을 되돌릴 수 있습니다. 오퍼레이션은 실패하지만 가스는
청구됩니다. 이더리움 재단은 이 발견에 5만 달러의 포상금을 지급하고, 자금 탈취가 아닌
검열·그리핑 경로로 분류했습니다. 실제로 악용된 적은 없습니다.

할 수 있는 일: 수수료를 낭비하고 거래를 늦추는 것. 할 수 없는 일: 자금을 훔치거나
서명을 위조하는 것. Vela의 노출은 좁습니다. UserOperation이 공개 멤풀을 거치지 않고
릴레이로 바로 가므로 가로챌 기회가 거의 없고, 최악의 경우도 이미 동의한 수수료 범위
안에 머뭅니다. 수정은 EntryPoint v0.9(2025년 11월)에만 있고, v0.7 자체에는 패치를
적용할 수 없습니다. 주변 스택 — 특히 Safe의 4337 모듈 계열 — 이 v0.9를 지원하는 데
맞춰 옮겨 갈 예정이며, 그때 이곳에 적겠습니다.

## 감사되지 않은 것

- **Vela 자체 컨트랙트.** 우리가 쓴 작은 컨트랙트 둘이 Gnosis에 배포되어 있습니다.
  [패스키 공개 키 인덱스](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools)
  (당신의 기기가 공개 키를 찾도록 돕는 추가 전용 레지스트리)와 그 배치 보조입니다.
  감사되지 않았습니다. 구조상 자금을 보유하지 않고, 소유자도 없으며, 업그레이드도
  불가능합니다. 발견을 위한 계층이지 승인을 위한 계층이 아닙니다. 지출 권한은 언제나
  당신의 Safe 안에 설정된 패스키에서 나옵니다. 현실적인 최악은 그리핑(누군가 인덱스
  항목을 선점하는 것)이고, 복구가 불편해질 수는 있어도 돈을 옮기지는 못합니다. 예전
  수수료 설계에 있던 가스 정산 분배 컨트랙트는 더 이상 거래 경로에 없습니다.
- **Multicall3.** 자체 README가
  [분명히 적고 있습니다](https://github.com/mds1/multicall3): “이 컨트랙트는 감사되지
  않았습니다.” 우리는 작성자가 안전하다고 설명한 방식 그대로 — 잔액, 토큰 메타데이터,
  가격을 읽는 배치 읽기 — 로만 씁니다. Vela가 거기에 승인을 주는 일은 없고, 자금을
  보유시키지도 않습니다. 버그의 최악은 잘못된 읽기 값입니다.
- **CREATE2 배포자.**
  [Arachnid 결정적 배포 프록시](https://github.com/Arachnid/deterministic-deployment-proxy)
  는 생태계 표준의 무상태 배포자이며 정식 감사는 없습니다. 어떤 체인에서 빠져 있거나
  변조되어 있으면, 우리의 네트워크 검사는 안전한 쪽으로 실패합니다.
- **Tempo와 pathUSD.** 내장 네트워크 열두 개 중 하나인 Tempo에는 네이티브 코인이
  없고, 가스는 pathUSD 스테이블코인으로 정산됩니다. 2026년 8월 기준으로 Tempo의 코어
  프로토콜에도 pathUSD에도 공개된 보안 감사가 없고 버그 바운티도 없습니다. 독립적인
  [DefiLlama 담보 평가](https://artifacts.llama.fi/md-exports/pathusd-collateral-assessment-april2026-1776332825042.md)
  (2026년 4월)는 pathUSD를 고위험으로 평가했습니다. 이는 체인 수준의 위험이고 어떤
  지갑도 줄일 수 없습니다. Tempo에 둔 자금도, 그곳의 가스 정산도 그 위험을 물려받습니다.
  Tempo는 이 목록에서 가장 새롭고 가장 덜 검증된 체인으로 보고, 잔액 규모도 거기에
  맞추세요. 감사가 나오면 이 절을 갱신하겠습니다.
- **Vela 자체.** 우리 앱과 백엔드 서비스는 제3자 감사를 받지 않았습니다. 이 페이지에서
  가장 큰 단서이고, 사이트 헤더에도 적어 두었습니다. 솔직한 세부는
  [Vela는 알파입니다](/blog/vela-is-in-alpha)에 있습니다. 적은 금액부터 시작하세요.
  코드를 읽어 보세요.

## 직접 확인하기

위의 모든 주소는 공개된 표준 배포이고, 공식 레지스트리와 대조할 수 있습니다 —
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments),
그리고 [EntryPoint 릴리스 노트](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| 컨트랙트 | 주소 |
| ----------------------------------- | -------------------------------------------- |
| SafeL2 singleton v1.4.1 | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1 | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| CompatibilityFallbackHandler v1.4.1 | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| MultiSend v1.4.1 | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| SafeModuleSetup v0.3.0 | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0 | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1 | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| EntryPoint v0.7 | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3 | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| 패스키 공개 키 인덱스(Gnosis) | `0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3` |
