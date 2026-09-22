---
title: Hướng dẫn tự triển khai
description: "Mọi thứ Vela vận hành cho bạn, mỗi thành phần làm gì, và cách thay nó bằng bản của riêng bạn — relay, chỉ mục khóa công khai, dữ liệu chuỗi, tỷ giá và các ứng dụng — cùng một thứ duy nhất bạn không thể thay, và cách sống khi không có getvela.app."
source: 3617d6d07f71
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Hướng dẫn tự triển khai

Tiền của bạn nằm trong một hợp đồng Safe trên chuỗi, do các khóa của bạn kiểm soát.
Không thứ gì Vela vận hành có thể di chuyển nó. Thứ Vela vận hành là bộ máy giúp chiếc ví
dễ dùng: một relay gửi giao dịch của bạn lên chuỗi, một chỉ mục giúp thiết bị mới tìm lại
ví, một danh mục dữ liệu chuỗi, một nguồn tỷ giá, và chính các ứng dụng.

Trang này liệt kê từng thành phần đó, cái gì hỏng khi thiếu nó, và cách tự chạy bản của
riêng bạn. Trang cũng nói về thứ duy nhất bạn không thể thay — tên miền mà passkey của bạn
thuộc về — và cần làm gì nếu getvela.app biến mất.

<Callout type="info" title="Trang này dành cho ai">
Bạn nên quen dùng terminal, Docker hoặc Cloudflare Workers, và biết nạp tiền cho một địa
chỉ trên chuỗi. Không có gì ở đây là cần thiết để dùng Vela hằng ngày.
</Callout>

## Bản đồ

| Thành phần | Làm gì | Mặc định của Vela | Thay được không? | Khi thiếu nó |
| --- | --- | --- | --- | --- |
| **Relay** | Nhận thao tác đã ký của bạn, trả gas, gửi nó lên chuỗi, thu khoản phí bạn đã ký | `vela-relay-cf.getvela.app` | Được — chạy [vela-relay](#relay) và trỏ ví tới đó | Bạn không gửi được giao dịch |
| **Chỉ mục khóa công khai** | Đăng ký các khóa của ví mới lên chuỗi; trả lời câu hỏi "khóa này thuộc ví nào?" | `p256-index-v2.getvela.app` | Được — chạy [p256-index](#index) | Không tạo được ví mới; đăng nhập chuyển sang đọc thẳng từ chuỗi |
| **Hợp đồng sổ đăng ký** | Bản ghi công khai vĩnh viễn về các khóa của từng ví | `0x94fD…1EA9` trên Gnosis | Không cần thay — không ai sở hữu nó; ví đọc nó trực tiếp | — |
| **Dữ liệu chuỗi** | Thông tin mạng, danh sách token, logo, bộ mô tả ký minh bạch | `ethereum-data.getvela.app` | Được — chạy [ethereum-data](#chain-data) | Không có danh sách token hay logo; ít giao dịch được giải mã hơn; không thêm được mạng |
| **Tỷ giá** | Giá trị quy đổi sang tiền tệ hiển thị của bạn | `vela-currency.getvela.app` | Được — chạy [vela-currency](#exchange-rates) hoặc bất kỳ nguồn nào tương thích Frankfurter | Ứng dụng chuyển sang tỷ giá Chainlink trên chuỗi nếu có thể (bản máy tính hiện USD) |
| **Nút RPC** | Đọc số dư, mô phỏng giao dịch | Các điểm cuối công khai cho từng mạng | Được — theo từng mạng, trong Cài đặt → Mạng lưới | Vela tự chuyển dự phòng giữa các điểm cuối |
| **Các ứng dụng** | Chính chiếc ví | wallet.getvela.app, các bản build phát hành | Được — [tự biên dịch](#web-app) | — |
| **getvela.app** | Tên miền mà passkey của bạn thuộc về | — | **Không** — xem [bên dưới](#if-getvela-app-disappears) | — |

Ngoài ra còn một số dịch vụ bên thứ ba cũng được liên hệ và không thuộc về Vela: các cơ sở
dữ liệu selector hàm công khai (sourcify, openchain, 4byte) được dùng như phương án cuối
cùng khi giải mã giao dịch, danh mục trình xác thực dùng để gọi tên mẫu khóa bảo mật của
bạn, và các máy chủ đường hầm (tunnel) của Apple và Google khi bạn ký bằng điện thoại qua
việc quét mã QR.

## Thứ duy nhất bạn không thể thay: tên miền của passkey

<span id="if-getvela-app-disappears"></span>

Mỗi passkey gắn với trang web mà nó được tạo ra để dùng. Các khóa của Vela được tạo cho
`getvela.app`. Trình duyệt chỉ đưa chúng cho các trang trên getvela.app hoặc tên miền con
của nó (hoặc cho những nguồn mà getvela.app khai báo là có liên quan), còn passkey tích hợp
trong điện thoại chỉ hoạt động trong những ứng dụng được getvela.app bảo chứng. Bên ngoài
trình duyệt, quy tắc lỏng hơn: Chrome cho phép một tiện ích có quyền với getvela.app dùng
chúng, và một chương trình trên máy tính của bạn có thể trực tiếp xin khóa bảo mật hoặc
điện thoại một chữ ký cho getvela.app — đó là cách các ứng dụng tự biên dịch hoạt động, và
là lý do phần mềm bạn chạy lại quan trọng. Từ đó suy ra hai điều.

**Một bản sao ví web trên tên miền của bạn là một ví khác.** Khi được phục vụ từ
`wallet.example.com`, cùng đoạn mã đó sẽ tạo passkey cho `wallet.example.com` — khóa mới,
và do đó là địa chỉ mới. Nó không thể ký cho một ví được tạo ở wallet.getvela.app. Bản sao
đó vẫn hữu ích: cho một ví bạn tạo ngay trên đó, hoặc để tự vận hành toàn bộ hệ thống từ
đầu.

**Với một ví đã có, những cách sau vẫn hoạt động nếu getvela.app ngừng hoạt động hoặc biến
mất:**

| Lối vào | Khóa dùng được | Lấy ở đâu |
| --- | --- | --- |
| **Tiện ích trình duyệt Vela** (trình duyệt nhân Chromium: Chrome, Edge, Brave) | Bất kỳ khóa nào trình duyệt với tới được: passkey của thiết bị này, khóa bảo mật USB (NFC nếu máy tính hỗ trợ), điện thoại qua mã QR | Tệp zip phát hành trên [GitHub](https://github.com/mondaylabsltd/vela-wallet/releases), hoặc [tự biên dịch](#web-app) |
| **Ứng dụng máy tính hoặc điện thoại bạn tự biên dịch** | Điện thoại qua mã QR, và khóa bảo mật USB | [Tự biên dịch](#web-app) |
| **Ứng dụng từ cửa hàng và ứng dụng máy tính đã công chứng** | Điện thoại qua mã QR và khóa bảo mật thì luôn dùng được; passkey "thiết bị này" chỉ dùng được khi hệ điều hành còn đối chiếu được ứng dụng với getvela.app | Trang phát hành trên GitHub (cửa hàng sau này) |

Tiện ích dùng được các khóa của `getvela.app` vì Chrome cho phép một tiện ích có quyền với
một trang web dùng passkey của trang đó. Trình duyệt kiểm tra quyền này ngay trên máy; chúng
tôi đã đo thử và thấy nó hoạt động, dù chưa thử trong tình huống tên miền thực sự ngừng
hoạt động. Một ứng dụng tự biên dịch dùng được điện thoại hoặc khóa bảo mật vì Vela nói
chuyện trực tiếp với chúng; còn passkey của chính điện thoại ("thiết bị này") đòi hỏi ứng
dụng phải do Vela ký, mà bản của bạn thì không.

[Trang ký](/vi/docs/clear-signing-self-host) tự nó không phải là một lối vào: nó ký những
yêu cầu do một chương trình khác gửi tới, và hiện chưa có ứng dụng Vela nào gửi yêu cầu như
vậy.

<Callout type="warning" title="Ai kiểm soát tên miền thì xin được chữ ký">
Bất kỳ trang nào được phục vụ từ getvela.app hoặc một tên miền con của nó — hoặc bởi bất kỳ
ai kiểm soát tên miền này trong tương lai — đều có thể xin các khóa của bạn một chữ ký, và
lời nhắc của hệ thống hiện "getvela.app", chứ không hiện giao dịch. Passkey ở đâu cũng hoạt
động như vậy. Vì lý do đó, trang web của Vela cấm chính các trang của mình dùng passkey. Đó
cũng là lý do tiện ích và ứng dụng tự biên dịch quan trọng: chúng mang theo mã của riêng
mình, dù theo mặc định vẫn lấy bộ mô tả và dùng các dịch vụ dưới getvela.app.
</Callout>

## Trỏ ví tới dịch vụ của bạn

Mỗi ứng dụng có bốn trường trong **Cài đặt → Nâng cao → Điểm cuối dịch vụ** (trên máy
tính: **Cài đặt → Điểm cuối dịch vụ**): chỉ mục dữ liệu chain, chỉ mục passkey, Vela Relay
và tỷ giá fiat. Mỗi trường hiện giá trị mặc định của Vela cho đến khi bạn đổi; **Khôi phục
mặc định** đưa cả bốn trường về như cũ. Với relay, chỉ mục và dữ liệu chuỗi, ví gọi
`/api/health` và hiện một huy hiệu, chỉ có màu xanh khi điểm cuối trả về đúng tên dịch vụ và
báo `status: "ok"`. Dù thế nào ví cũng lưu những gì bạn nhập — hãy đợi đến khi thấy màu xanh.

| Dịch vụ | `service` trong `/api/health` |
| --- | --- |
| Relay | `vela-relay` |
| Chỉ mục khóa công khai | `webauthn-p256-publickey-registry` |
| Dữ liệu chuỗi | `ethereum-data` |
| Tỷ giá | không kiểm tra theo tên — phải trả về danh sách tỷ giá theo USD |

Cả bốn ứng dụng đều tuân theo cả bốn trường, và một điểm cuối vừa đổi sẽ có hiệu lực
ngay ở lần gọi tiếp theo chứ không phải lần khởi động tiếp theo: mọi đường đi — tạo ví,
đăng nhập, tra tên cho một địa chỉ — đều đọc điểm cuối ngay lúc dùng đến nó. Trên iOS,
chỉ mục passkey còn đổi được ở màn hình đăng nhập khi không kết nối được chỉ mục mặc
định.

(Cho tới tháng 9/2026, điều đó có bốn ngoại lệ; tệ nhất là trang trên iOS chỉ hiện giá
trị giữ chỗ và không lưu gì cả. Tất cả đã được sửa.)

## Tự chạy relay của bạn

<span id="relay"></span>

Relay là [vela-relay](https://github.com/mondaylabsltd/vela-relay) (Rust, MIT). Một bản
triển khai phục vụ mọi chuỗi: ví gọi `https://your-relay/<chainId>`. Nó phải là
vela-relay — ví xin báo giá phí bằng một phương thức riêng của Vela mà các bundler ERC-4337
thông dụng không hỗ trợ.

**Bạn cần**

- Hoặc Docker cùng một Redis và một máy chủ [Iggy](https://iggy.apache.org) bạn đang sẵn
  chạy, hoặc một tài khoản Cloudflare dùng gói **Workers Paid**, cùng Node.js và bộ công cụ
  Rust (có target `wasm32-unknown-unknown`) trên máy của bạn.
- Một `OPERATOR_SECRET` (hex, ít nhất 32 byte). Từ đó suy ra một địa chỉ ngân quỹ và một
  nhóm địa chỉ relayer, giống nhau trên mọi chuỗi. Hãy giữ bí mật: nó kiểm soát tiền của
  relay.
- Gas trên mọi chuỗi bạn muốn phục vụ: gửi coin của chuỗi đó (trên Tempo là pathUSD) vào
  địa chỉ ngân quỹ của bạn. Ngân quỹ sẽ nạp thêm cho các relayer.

**Docker**

```sh
git clone https://github.com/mondaylabsltd/vela-relay
cd vela-relay
cp .env.example .env
# trong .env: VELA_RELAY_IGGY_URL, VELA_RELAY_REDIS_URL, OPERATOR_SECRET,
# VELA_RELAY_CHAIN_DIRECTORY_URL nếu bạn tự chạy dữ liệu chuỗi,
# và VELA_RELAY_IMAGE đặt thành một image phát hành bạn tin tưởng (xem docs/docker.md)
docker compose pull relay
docker compose up -d --no-build
curl --fail http://127.0.0.1:4567/readyz
```

Cách nào cũng được: image đã phát hành là nhanh nhất, còn `docker compose up --build` dựng
đúng thứ đó từ mã nguồn mà bạn đọc được. Nếu không dùng Docker,
`cargo run --release --bin vela-relay` chạy trực tiếp relay.

**Cloudflare Workers**

```sh
cd vela-relay/vela-relay-cf
npx wrangler queues create vela-relay-ops
npx wrangler queues create vela-relay-dlq
npx wrangler secret put OPERATOR_SECRET
# dữ liệu chuỗi của riêng bạn: thêm "VELA_RELAY_CHAIN_DIRECTORY_URL" vào "vars" trong wrangler.jsonc
npx wrangler deploy
```

**Kiểm tra**

```sh
curl https://your-relay/api/health        # {"service":"vela-relay","status":"ok",…}
curl https://your-relay/v1/treasury/100   # địa chỉ ngân quỹ của bạn trên Gnosis, và nó có cần gas không
```

Sau đó nhập `https://your-relay` vào trường **Vela Relay**.

**Cần biết**

- Khoản phí ví trả sẽ vào ngân quỹ của bạn. Ví tính phí theo cùng một cách, dù bạn dùng
  relay nào (xem [mạng & phí](/vi/docs/networks-and-fees)).
- Một mạng tùy chỉnh bạn đã thêm trước khi đổi relay sẽ giữ nguyên địa chỉ relay đã dùng
  khi thêm nó.
- Relay đọc một danh mục chuỗi để biết thông tin từng chuỗi và những stablecoin nó chấp
  nhận. Danh mục đó là `ethereum-data.getvela.app`, trừ khi bạn đặt
  `VELA_RELAY_CHAIN_DIRECTORY_URL` trỏ tới [danh mục của riêng bạn](#chain-data). Thiết lập này có từ vela-relay v0.9.6; các phiên bản cũ hơn luôn đọc bản của Vela.

## Tự chạy chỉ mục khóa công khai

<span id="index"></span>

Chỉ mục là [p256-index](https://github.com/mondaylabsltd/p256-index) (Rust, MIT). Khi một ví
được tạo, nó kiểm tra bằng chứng của từng khóa, rồi ghi cả nhóm khóa vào **hợp đồng sổ đăng
ký** trên Gnosis và trả gas. Hãy tiếp tục dùng sổ đăng ký hiện có tại
`0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`: nó không có chủ sở hữu, bất kỳ địa chỉ nào có
tiền cũng ghi vào được, và mọi ứng dụng Vela đều đọc trực tiếp từ nó. Một sổ đăng ký của
riêng bạn sẽ vô hình với chúng.

**Bạn cần**

- Docker cùng Redis và Iggy (bản máy chủ), hoặc một tài khoản Cloudflare (bản Worker; chính
  README của nó ghi rằng phần ghi lên chuỗi chưa được thử nghiệm trọn vẹn từ đầu đến cuối).
- Một khóa riêng trên Gnosis có xDAI. Đăng ký một ví tốn khoảng 1,1 triệu gas với một khóa
  và khoảng 3,6 triệu với bảy khóa.
- Các thiết lập sau:

```dotenv
P256_INDEX_IGGY_URL=iggy+tcp://user:password@iggy.example:5100
P256_INDEX_REDIS_URL=redis://redis.example:6379/0
P256_INDEX_CONTRACT_ADDRESS=0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9
P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf
PRIVATE_KEY=0x…
```

`P256_INDEX_DOMAIN_REGISTRY` là thứ người ta hay bỏ sót: thiếu nó, máy chủ sẽ phát ra
những thử thách (challenge) mà hợp đồng từ chối, và mọi lần đăng ký đều thất bại. Nó có
trong `.env.example`, và phải trùng với `DOMAIN_REGISTRY` của chính hợp đồng đã triển
khai — từ VERSION 12 của sổ đăng ký, miền thử thách được cố định ngay lúc triển khai, nên
nó không đổi khi hợp đồng được triển khai lại.

**Chạy và kiểm tra**

```sh
git clone https://github.com/mondaylabsltd/p256-index
cd p256-index
cargo run --release -p p256-index-server
curl https://your-index/api/health   # "service":"webauthn-p256-publickey-registry","status":"ok"
```

Máy chủ lắng nghe qua HTTP thường (mặc định cổng 11256); hãy đặt một proxy TLS phía trước,
vì ví chỉ chấp nhận điểm cuối `https://`. Lệnh `docker build -f
p256-index-server/Dockerfile .` cũng chạy được từ một bản clone sạch; nếu dùng Compose,
hãy chép `.env.example` thành `p256-index-server/.env` trước.

**Nếu không có chỉ mục nào trả lời**, các ví đã có vẫn hoạt động: khi đăng nhập, ứng dụng
đọc hợp đồng sổ đăng ký trên Gnosis (rồi đến Ethereum) qua các nút RPC của bạn. Một ví chỉ
có một khóa thậm chí có thể được dựng lại từ hai chữ ký mà không cần tới sổ đăng ký. Tạo ví
mới thì cần có chỉ mục, vì phải có ai đó trả tiền cho việc đăng ký.

## Tự chạy dữ liệu chuỗi

<span id="chain-data"></span>

Dữ liệu chuỗi là [ethereum-data](https://github.com/atshelchin/ethereum-data) (MIT): JSON
tĩnh và hình ảnh cho khoảng 2.600 mạng cùng token của chúng, cộng với các bộ mô tả ERC-7730
mà Vela dùng để giải thích giao dịch.

```sh
docker run -d --name ethereum-data -p 3000:3000 --restart unless-stopped \
  ghcr.io/atshelchin/ethereum-data:latest
curl http://localhost:3000/api/health   # "service":"ethereum-data","status":"ok"
```

README của nó cũng hướng dẫn build từ mã nguồn và triển khai lên Cloudflare. Hãy phục vụ nó
qua HTTPS và nhập địa chỉ vào trường **Chỉ mục dữ liệu chain**.

Relay cũng đọc các tệp này, kể cả một trường riêng của Vela (danh sách `stables` quyết định
những stablecoin nào trả được phí). Hãy trỏ nó tới bản của bạn bằng
`VELA_RELAY_CHAIN_DIRECTORY_URL=https://your-chain-data`; relay lưu dữ liệu của từng mạng vào
bộ nhớ đệm trong một giờ.

## Tự chạy dịch vụ tỷ giá

<span id="exchange-rates"></span>

[vela-currency](https://github.com/mondaylabsltd/vela-currency) (MIT) đăng lại tỷ giá hằng
ngày của Ngân hàng Trung ương châu Âu. Nó không cần khóa API nào.

```sh
docker run -d -p 8080:8080 -v rates-data:/data ghcr.io/mondaylabsltd/vela-currency:latest
curl "http://localhost:8080/v2/rates?base=USD"
```

Nhập `https://your-host/v2/rates?base=USD` vào trường **Tỷ giá fiat**. Bất kỳ dịch vụ nào
tương thích Frankfurter cũng dùng được. Hãy giữ `?base=USD`: mọi phép quy đổi đều giả định
như vậy.

## Tự biên dịch các ứng dụng

<span id="web-app"></span>

Tất cả các ứng dụng đều nằm trong [một kho mã](https://github.com/mondaylabsltd/vela-wallet)
(MIT). README liệt kê các bước build của từng ứng dụng; bản ngắn gọn:

| Ứng dụng | Build | Ký được cho ví getvela.app bạn đang có không? |
| --- | --- | --- |
| Tiện ích trình duyệt | `cd app-web/vela-wallet && pnpm install && pnpm build:extension`, rồi nạp `extension/dist` bằng "Tải tiện ích đã giải nén" tại `chrome://extensions` | Có, với bất kỳ khóa nào |
| Ví web | `cd app-web/vela-wallet && pnpm install && pnpm build`; triển khai dưới dạng một Cloudflare Worker | Không — trên tên miền của bạn, đó là một ví khác (xem ở trên) |
| Máy tính | `cd app-desktop/vela-wallet && cargo run` (script đóng gói có trong README của nó) | Có, với điện thoại qua mã QR hoặc khóa bảo mật USB |
| Android | Tạo các binding của lõi, rồi `./gradlew :app:installDebug` | Có, với điện thoại qua mã QR hoặc khóa bảo mật USB |
| iOS | `./rust/scripts/build-ios-xcframework.sh`, rồi build trong Xcode với team của riêng bạn | Có, với điện thoại qua mã QR hoặc YubiKey cổng USB-C / Lightning (firmware 5.8 trở lên) |

Passkey "thiết bị này" của một ứng dụng tự biên dịch sẽ không dùng được cho các ví
getvela.app: Apple và Google chỉ cho những ứng dụng do Vela ký dùng passkey của
`getvela.app`.

## Thêm một mạng mà Vela không có sẵn

Vela chạy trên bất kỳ chuỗi EVM nào có precompile P-256 và các hợp đồng tiêu chuẩn mà nó
kiểm tra. [Thiết lập chuỗi](/vi/chain-setup) cho bạn biết một chuỗi còn thiếu gì và triển
khai những gì ai cũng triển khai được; [mạng & phí](/vi/docs/networks-and-fees) giải thích
các yêu cầu. Bước kiểm tra có bao gồm hai hợp đồng mà ví nhiều hơn một khóa cần, và đánh
dấu rõ chúng là như vậy — một chuỗi thiếu chúng vẫn chạy được ví một khóa.

## Những gì vẫn trỏ về Vela sau tất cả

Nếu bạn thay mọi thứ ở trên, vẫn còn lại:

- **Danh mục trình xác thực** dùng để gọi tên mẫu khóa bảo mật — chỉ ảnh hưởng hiển thị;
  ứng dụng sẽ dùng một tên chung chung thay thế.
- **Các tệp liên kết của getvela.app**, thứ mà ứng dụng từ cửa hàng cần để dùng passkey
  "thiết bị này". Điện thoại hoặc khóa bảo mật thì không cần chúng.

Và những thứ sau không thuộc về Vela: các cơ sở dữ liệu selector công khai, đường hầm đăng
nhập bằng điện thoại của Apple và Google, và những nhà cung cấp RPC mà bạn chọn.

Tiếp theo: [trang ký mà bạn có thể tự chạy](/vi/docs/clear-signing-self-host).
