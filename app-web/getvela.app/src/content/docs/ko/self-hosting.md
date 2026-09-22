---
title: 셀프 호스팅 가이드
description: "Vela가 대신 운영하는 모든 것, 각각의 역할, 그리고 릴레이, 공개 키 인덱스, 체인 데이터, 환율, 앱을 직접 운영하는 것으로 바꾸는 방법. 바꿀 수 없는 단 하나와, getvela.app 없이 지내는 방법도 다룹니다."
source: de484cb33065
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# 셀프 호스팅 가이드

자금은 온체인의 Safe 컨트랙트에 있고, 사용자의 키가 제어합니다. Vela가 운영하는 어떤 것도
자금을 움직일 수 없습니다. Vela가 운영하는 것은 지갑을 편리하게 해 주는 장치들입니다. 거래를
제출하는 릴레이, 새 기기가 지갑을 찾도록 돕는 인덱스, 체인 데이터 디렉터리, 환율 피드, 그리고
앱 자체입니다.

이 페이지는 이 구성 요소를 하나하나 짚으며, 각각이 없으면 무엇이 안 되는지, 어떻게 직접
운영하는지 설명합니다. 바꿀 수 없는 단 하나, 즉 패스키가 속한 도메인과, getvela.app이
사라졌을 때 할 일도 다룹니다.

<Callout type="info" title="이 페이지가 필요한 사람">
터미널, Docker나 Cloudflare Workers를 다룰 수 있고, 체인의 주소에 자금을 넣을 줄 알아야
합니다. Vela를 평소에 쓰는 데는 이 페이지의 내용이 전혀 필요 없습니다.
</Callout>

## 전체 구성

| 구성 요소 | 하는 일 | Vela 기본값 | 바꿀 수 있나 | 없으면 |
| --- | --- | --- | --- | --- |
| **릴레이** | 서명된 오퍼레이션을 받아 가스를 내고 제출하며, 서명으로 동의한 수수료를 받음 | `vela-relay-cf.getvela.app` | 가능 — [vela-relay](#relay)를 운영하고 지갑이 그것을 쓰게 설정 | 보낼 수 없음 |
| **공개 키 인덱스** | 새 지갑의 키를 온체인에 등록하고, "이 키는 어느 지갑에 속하나?"에 답함 | `p256-index-v2.getvela.app` | 가능 — [p256-index](#index) 운영 | 새 지갑을 만들 수 없음. 로그인은 체인을 직접 읽는 방식으로 대체 |
| **레지스트리 컨트랙트** | 지갑마다 키를 기록한 영구 공개 기록 | Gnosis의 `0x94fD…1EA9` | 바꿀 필요 없음 — 소유자가 없고, 지갑이 직접 읽음 | — |
| **체인 데이터** | 네트워크 정보, 토큰 목록, 로고, 클리어 서명 디스크립터 | `ethereum-data.getvela.app` | 가능 — [ethereum-data](#chain-data) 운영 | 토큰 목록과 로고가 없고, 디코딩되는 거래가 줄며, 네트워크 추가가 실패함 |
| **환율** | 표시 통화로 법정화폐 금액 표시 | `vela-currency.getvela.app` | 가능 — [vela-currency](#exchange-rates)나 Frankfurter 호환 소스 운영 | 앱이 가능한 곳에서는 온체인 Chainlink 환율로 대체(데스크톱은 USD로 표시) |
| **RPC 노드** | 잔액 읽기, 거래 시뮬레이션 | 네트워크별 공개 엔드포인트 | 가능 — 설정 → 네트워크에서 네트워크별로 | Vela가 엔드포인트 사이에서 자동 전환 |
| **앱** | 지갑 그 자체 | wallet.getvela.app, 릴리스 빌드 | 가능 — [직접 빌드](#web-app) | — |
| **getvela.app** | 패스키가 속한 도메인 | — | **불가** — [아래](#if-getvela-app-disappears) 참고 | — |

Vela의 것이 아닌 제3자 서비스에도 몇 가지 연결합니다. 거래를 디코딩할 때 마지막 수단으로
쓰는 공개 함수 셀렉터 데이터베이스(sourcify, openchain, 4byte), 보안 키 모델명을 알려 주는
인증자 디렉터리, 그리고 QR 코드를 스캔해 휴대폰으로 서명할 때 거치는 Apple과 Google의 터널
서버입니다.

## 바꿀 수 없는 단 하나: 패스키의 도메인

<span id="if-getvela-app-disappears"></span>

패스키는 그것을 만든 웹사이트에 속합니다. Vela의 키는 `getvela.app`용으로 만들어집니다.
브라우저는 getvela.app과 그 하위 도메인의 페이지(또는 getvela.app이 관련 출처로 선언한 곳)에만
패스키를 내어 주고, 휴대폰에 내장된 패스키는 getvela.app이 보증하는 앱에서만 작동합니다.
브라우저 밖에서는 규칙이 느슨합니다. Chrome은 getvela.app 권한을 받은 확장 프로그램이 이
패스키를 쓰도록 허용하고, 컴퓨터의 프로그램은 보안 키나 휴대폰에 getvela.app 서명을 직접
요청할 수 있습니다. 직접 빌드한 앱이 작동하는 방식이 바로 이것이고, 어떤 소프트웨어를
실행하느냐가 중요한 이유이기도 합니다. 여기서 두 가지가 따라옵니다.

**웹 지갑을 내 도메인에 올리면 다른 지갑이 됩니다.** 같은 코드라도 `wallet.example.com`에서
서비스하면 `wallet.example.com`용 패스키를 만듭니다. 키가 새로 생기니 주소도 새로 생깁니다.
wallet.getvela.app에서 만든 지갑에는 서명할 수 없습니다. 그래도 쓸모는 있습니다. 거기서 새로
만든 지갑에 쓰거나, 전체 스택을 처음부터 직접 운영할 때 쓸 수 있습니다.

**기존 지갑이라면, getvela.app이 내려가거나 없어져도 다음 방법은 계속 작동합니다.**

| 방법 | 쓸 수 있는 키 | 받는 곳 |
| --- | --- | --- |
| **Vela 브라우저 확장 프로그램**(Chromium 기반 브라우저: Chrome, Edge, Brave) | 브라우저가 닿을 수 있는 모든 키. 이 기기의 패스키, USB 보안 키(컴퓨터가 지원하면 NFC도), QR로 연결한 휴대폰 | [GitHub](https://github.com/mondaylabsltd/vela-wallet/releases)의 릴리스 zip, 또는 [직접 빌드](#web-app) |
| **직접 빌드한 데스크톱 앱이나 휴대폰 앱** | QR로 연결한 휴대폰, USB 보안 키 | [직접 빌드](#web-app) |
| **스토어 앱과 공증된 데스크톱 앱** | QR 휴대폰과 보안 키는 언제나. "이 기기" 패스키는 운영체제가 앱을 getvela.app과 대조해 확인할 수 있는 동안만 | GitHub 릴리스(스토어는 나중에) |

확장 프로그램이 `getvela.app` 키를 쓸 수 있는 것은, Chrome이 어떤 사이트의 권한을 받은 확장
프로그램에 그 사이트의 패스키 사용을 허용하기 때문입니다. 이 권한은 브라우저가 로컬에서
확인합니다. 실제로 작동하는 것은 확인했지만, 도메인이 정말로 내려간 상태에서 테스트해 보지는
않았습니다. 직접 빌드한 앱이 휴대폰이나 보안 키를 쓸 수 있는 것은 Vela가 이들과 직접 통신하기
때문입니다. 휴대폰 자체의 패스키("이 기기")를 쓰려면 앱이 Vela의 서명을 받아야 하는데, 직접
빌드한 앱은 그렇지 않습니다.

[서명 페이지](/ko/docs/clear-signing-self-host)는 그 자체로는 들어가는 방법이 아닙니다.
다른 프로그램이 보낸 요청에 서명할 뿐인데, 아직 이 페이지로 요청을 보내는 Vela 앱이 없습니다.

<Callout type="warning" title="도메인을 가진 쪽은 서명을 요청할 수 있습니다">
getvela.app이나 그 하위 도메인에서 서비스되는 페이지는, 그리고 앞으로 이 도메인을 갖게 될
누구든, 사용자의 키에 서명을 요청할 수 있습니다. 이때 시스템 확인 창에는 거래 내용이 아니라
"getvela.app"이 표시됩니다. 패스키는 어디서나 이렇게 작동합니다. 그래서 Vela 웹사이트는 자기
페이지가 패스키를 쓰지 못하게 막아 둡니다. 확장 프로그램과 직접 빌드한 앱이 중요한 이유도
여기에 있습니다. 이들은 자기 코드를 직접 가지고 다닙니다. 다만 기본 설정으로는 여전히
getvela.app 아래의 서비스에서 디스크립터를 가져오고 그 서비스를 씁니다.
</Callout>

## 지갑이 내 서비스를 쓰게 하기

각 앱의 **설정 → 고급 → 서비스 엔드포인트**(데스크톱은 **설정 → 서비스 엔드포인트**)에는
입력란이 네 개 있습니다. 체인 데이터, 패스키 인덱스, Vela 릴레이, 법정화폐 환율입니다. 바꾸기
전까지는 각 입력란에 Vela 기본값이 표시되며, **기본값으로 재설정**을 누르면 네 개가 모두
되돌아갑니다. 릴레이, 인덱스, 체인 데이터에 대해서는 지갑이 `/api/health`를 호출해 상태
배지를 보여 주는데, 엔드포인트가 올바른 서비스 이름을 밝히고 `status: "ok"`를 돌려줄 때만
초록색이 됩니다. 배지 색과 상관없이 입력한 값은 저장되니, 초록색이 될 때까지 확인하세요.

| 서비스 | `/api/health`의 `service` |
| --- | --- |
| 릴레이 | `vela-relay` |
| 공개 키 인덱스 | `webauthn-p256-publickey-registry` |
| 체인 데이터 | `ethereum-data` |
| 환율 | 이름으로 확인하지 않음 — USD 기준 환율 목록을 돌려줘야 함 |

현재 각 앱이 이 설정을 얼마나 따르는지는 다음과 같습니다.

| 앱 | 서비스 엔드포인트 | 네트워크별 RPC |
| --- | --- | --- |
| 웹과 확장 프로그램 | 체인 데이터, 릴레이, 법정화폐 환율. 패스키 인덱스는 이름 조회에 쓰이지만, 지갑 생성과 로그인에는 여전히 Vela의 인덱스를 씀 | 지원 |
| 데스크톱 | 네 가지 모두. 새 패스키 인덱스는 재시작하거나 로그아웃한 뒤에 적용됨 | 지원 |
| Android | 네 가지 모두. 단, 주소의 이름을 조회할 때는 여전히 Vela의 인덱스에 물어봄 | 지원 |
| iOS | **아직 안 됨**: 페이지에 자리 표시용 값이 보이고 저장되지 않음. 기본 인덱스에 연결할 수 없을 때는 로그인 화면에서 패스키 인덱스를 바꿀 수 있음 | 읽기 전용 |

이런 빈틈은 버그이며, 추적하고 있습니다.

## 릴레이 직접 운영하기

<span id="relay"></span>

릴레이는 [vela-relay](https://github.com/mondaylabsltd/vela-relay)(Rust, MIT)입니다. 배포 하나가
모든 체인을 서비스하며, 지갑은 `https://your-relay/<chainId>`를 호출합니다. 반드시 vela-relay여야
합니다. 지갑이 Vela 전용 메서드로 수수료 견적을 요청하는데, 일반 ERC-4337 번들러는 이 메서드를
구현하지 않기 때문입니다.

**필요한 것**

- 이미 운영 중인 Redis와 [Iggy](https://iggy.apache.org) 서버에 Docker, 또는 **Workers Paid**
  요금제의 Cloudflare 계정과 내 컴퓨터의 Node.js, Rust 툴체인(`wasm32-unknown-unknown` 타깃
  포함).
- `OPERATOR_SECRET`(16진수, 최소 32바이트). 이 값에서 트레저리 주소 하나와 릴레이어 주소
  여러 개가 도출되며, 모든 체인에서 같습니다. 릴레이의 자금을 제어하는 값이니 비밀로 지키세요.
- 서비스하려는 모든 체인의 가스. 그 체인의 코인(Tempo는 pathUSD)을 트레저리 주소로 보내면,
  트레저리가 릴레이어들에 자금을 채워 줍니다.

**Docker**

```sh
git clone https://github.com/mondaylabsltd/vela-relay
cd vela-relay
cp .env.example .env
# .env에 VELA_RELAY_IGGY_URL, VELA_RELAY_REDIS_URL, OPERATOR_SECRET을 채우고,
# 체인 데이터를 직접 운영한다면 VELA_RELAY_CHAIN_DIRECTORY_URL도 넣고,
# VELA_RELAY_IMAGE는 신뢰하는 릴리스 이미지로 설정 (docs/docker.md 참고)
docker compose pull relay
docker compose up -d --no-build
curl --fail http://127.0.0.1:4567/readyz
```

게시된 이미지를 쓰는 편이 좋습니다. `docker compose up --build`로 소스에서 빌드하면 현재
Dockerfile에서는 실패할 수 있습니다. Docker 없이 `cargo run --release --bin vela-relay`로 바로
실행할 수도 있습니다.

**Cloudflare Workers**

```sh
cd vela-relay/vela-relay-cf
npx wrangler queues create vela-relay-ops
npx wrangler queues create vela-relay-dlq
npx wrangler secret put OPERATOR_SECRET
# 체인 데이터를 직접 운영한다면 wrangler.jsonc의 "vars"에 "VELA_RELAY_CHAIN_DIRECTORY_URL" 추가
npx wrangler deploy
```

**확인하기**

```sh
curl https://your-relay/api/health        # {"service":"vela-relay","status":"ok",…}
curl https://your-relay/v1/treasury/100   # Gnosis의 트레저리 주소와 가스 충전이 필요한지 여부
```

그다음 **Vela 릴레이** 입력란에 `https://your-relay`를 넣습니다.

**알아 둘 것**

- 지갑이 내는 수수료는 내 트레저리로 들어갑니다. 어떤 릴레이를 쓰든 지갑은 같은 방식으로
  수수료를 계산합니다([네트워크와 수수료](/ko/docs/networks-and-fees) 참고).
- 릴레이를 바꾸기 전에 추가한 사용자 지정 네트워크는 추가할 때 지정한 릴레이 주소를 그대로
  씁니다.
- 릴레이는 체인별 정보와, 수수료로 받아 주는 스테이블코인 목록을 체인 디렉터리에서 읽습니다.
  `VELA_RELAY_CHAIN_DIRECTORY_URL`을 [직접 운영하는 체인 데이터](#chain-data)로 설정하지 않으면
  `ethereum-data.getvela.app`에서 읽습니다. 이 설정은 2026년 9월에 추가되었고, 그보다 오래된
  릴레이 빌드는 항상 Vela의 사본을 읽습니다.

## 공개 키 인덱스 직접 운영하기

<span id="index"></span>

인덱스는 [p256-index](https://github.com/mondaylabsltd/p256-index)(Rust)입니다. 지갑이 만들어질
때 모든 키의 증명을 확인한 뒤, 키 묶음을 Gnosis의 **레지스트리 컨트랙트**에 기록하고 가스비를
냅니다. 기존 레지스트리 `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`를 계속 쓰세요. 소유자가
없고, 자금이 있는 주소라면 누구나 기록할 수 있으며, 모든 Vela 앱이 이 레지스트리를 직접
읽습니다. 따로 만든 레지스트리는 앱에서 보이지 않습니다.

**필요한 것**

- Redis와 Iggy를 갖춘 Docker(서버 버전), 또는 Cloudflare 계정(Worker 버전. 이 버전의 README에는
  온체인 기록을 아직 끝까지 테스트하지 않았다고 적혀 있습니다).
- xDAI가 있는 Gnosis 개인 키. 지갑 하나를 등록하는 데 키가 하나면 약 110만 가스, 일곱 개면 약
  360만 가스가 듭니다.
- 다음 설정:

```dotenv
P256_INDEX_IGGY_URL=iggy+tcp://user:password@iggy.example:5100
P256_INDEX_REDIS_URL=redis://redis.example:6379/0
P256_INDEX_CONTRACT_ADDRESS=0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9
P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf
PRIVATE_KEY=0x…
```

서버의 예시 파일에는 빠져 있지만 `P256_INDEX_DOMAIN_REGISTRY`는 꼭 필요합니다. 이 값이 없으면
서버가 컨트랙트에서 거부되는 챌린지를 내주게 되어, 모든 등록이 실패합니다.

**실행하고 확인하기**

```sh
git clone https://github.com/mondaylabsltd/p256-index
cd p256-index
cargo run --release -p p256-index-server
curl https://your-index/api/health   # "service":"webauthn-p256-publickey-registry","status":"ok"
```

서버는 일반 HTTP로 수신합니다(기본 포트 11256). 지갑은 `https://` 엔드포인트만 받으므로 앞에
TLS 프록시를 두세요. 이 글을 쓰는 시점에 소스의 Dockerfile은 빌드되지 않을 수 있으며, Cargo로
빌드하면 됩니다. 저장소에는 아직 라이선스 파일이 없습니다.

**응답하는 인덱스가 하나도 없을 때도** 기존 지갑은 작동합니다. 로그인할 때 앱이 RPC 노드를 통해
Gnosis(그다음 이더리움)의 레지스트리 컨트랙트를 읽습니다. 키가 하나뿐인 지갑은 레지스트리 없이
서명 두 번만으로 다시 구성할 수도 있습니다. 새 지갑을 만들 때는 인덱스가 꼭 필요합니다. 등록
비용을 누군가는 내야 하기 때문입니다.

## 체인 데이터 직접 운영하기

<span id="chain-data"></span>

체인 데이터는 [ethereum-data](https://github.com/atshelchin/ethereum-data)(MIT)입니다. 약 2,600개
네트워크와 그 토큰의 정적 JSON과 이미지, 그리고 Vela가 거래를 설명하는 데 쓰는 ERC-7730
디스크립터가 들어 있습니다.

```sh
docker run -d --name ethereum-data -p 3000:3000 --restart unless-stopped \
  ghcr.io/atshelchin/ethereum-data:latest
curl http://localhost:3000/api/health   # "service":"ethereum-data","status":"ok"
```

소스에서 빌드하는 방법과 Cloudflare에 배포하는 방법은 README에 있습니다. HTTPS로 서비스하고
**체인 데이터** 입력란에 주소를 넣으세요.

릴레이도 이 파일들을 읽으며, 그중에는 Vela 전용 필드도 있습니다(`stables` 목록이 수수료를 낼
수 있는 스테이블코인을 정합니다). `VELA_RELAY_CHAIN_DIRECTORY_URL=https://your-chain-data`로
릴레이가 내 사본을 읽게 하세요. 릴레이는 네트워크별 항목을 한 시간 동안 캐시합니다.

## 환율 직접 운영하기

<span id="exchange-rates"></span>

[vela-currency](https://github.com/mondaylabsltd/vela-currency)(MIT)는 유럽중앙은행이 매일
발표하는 환율을 다시 게시합니다. 키가 필요 없습니다.

```sh
docker run -d -p 8080:8080 -v rates-data:/data ghcr.io/mondaylabsltd/vela-currency:latest
curl "http://localhost:8080/v2/rates?base=USD"
```

**법정화폐 환율** 입력란에 `https://your-host/v2/rates?base=USD`를 넣으세요. Frankfurter 호환
서비스라면 무엇이든 됩니다. `?base=USD`는 빼지 마세요. 모든 환산이 이를 전제로 합니다.

## 앱 직접 빌드하기

<span id="web-app"></span>

모든 앱은 [저장소 하나](https://github.com/mondaylabsltd/vela-wallet)(MIT)에 있습니다. 앱별 빌드
방법은 README에 있고, 요약하면 다음과 같습니다.

| 앱 | 빌드 | getvela.app의 기존 지갑에 서명할 수 있나 |
| --- | --- | --- |
| 브라우저 확장 프로그램 | `cd app-web/vela-wallet && pnpm install && pnpm build:extension` 후 `chrome://extensions`에서 `extension/dist`를 압축해제된 확장 프로그램으로 로드 | 가능, 어떤 키로든 |
| 웹 지갑 | `cd app-web/vela-wallet && pnpm install && pnpm build`, Cloudflare Worker로 배포 | 불가 — 내 도메인에서는 다른 지갑이 됨(위 참고) |
| 데스크톱 | `cd app-desktop/vela-wallet && cargo run`(패키징 스크립트는 해당 README 참고) | 가능, QR로 연결한 휴대폰이나 USB 보안 키로 |
| Android | 코어 바인딩을 생성한 뒤 `./gradlew :app:installDebug` | 가능, QR로 연결한 휴대폰이나 USB 보안 키로 |
| iOS | `./rust/scripts/build-ios-xcframework.sh` 실행 후 내 개발자 팀으로 Xcode에서 빌드 | 가능, QR로 연결한 휴대폰이나 USB-C / Lightning YubiKey(펌웨어 5.8 이상)로 |

직접 빌드한 앱에서는 "이 기기" 패스키로 getvela.app 지갑에 서명할 수 없습니다. Apple과 Google은
Vela가 서명한 앱에만 `getvela.app` 패스키 사용을 허용하기 때문입니다.

## Vela에 내장되지 않은 네트워크 추가하기

Vela는 P-256 프리컴파일과, Vela가 확인하는 표준 컨트랙트를 갖춘 EVM 체인이라면 어디서든
작동합니다. [체인 설정](/ko/chain-setup)에서 체인에 무엇이 빠졌는지 확인하고 누구나 배포할 수
있는 것은 배포할 수 있으며, 요건은 [네트워크와 수수료](/ko/docs/networks-and-fees)에서
설명합니다. 빈틈이 하나 있습니다. 키가 두 개 이상인 지갑은 그 체인에 Safe의 패스키 서명자
팩토리도 있어야 하는데, 아직 이 부분은 확인하지 않습니다. 팩토리가 없으면 그 체인에서는 첫 번째
키만 서명할 수 있습니다.

## 이렇게 다 바꿔도 Vela를 가리키는 것

위의 것을 모두 바꿔도 다음은 남습니다.

- **보안 키 모델명을 알려 주는 인증자 디렉터리** — 표시에만 영향을 줍니다. 연결되지 않으면 앱이
  일반적인 이름을 보여 줍니다.
- **getvela.app의 연결 파일** — 스토어 앱이 "이 기기" 패스키를 쓰려면 필요합니다. 휴대폰이나
  보안 키에는 필요 없습니다.

그리고 다음은 Vela의 것이 아닙니다. 공개 셀렉터 데이터베이스, 휴대폰 로그인에 쓰이는 Apple과
Google의 터널, 그리고 직접 고른 RPC 공급자입니다.

다음: [직접 운영할 수 있는 서명 페이지](/ko/docs/clear-signing-self-host).
