---
title: Whitepaper
description: Vela hoạt động ra sao và bạn phải — hoặc không phải — tin vào những gì để dùng nó. Kiến trúc, mô hình bảo mật, khôi phục, và cách tự kiểm chứng tất cả.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Trạng thái: alpha · v0.1">
Trang này mô tả Vela hoạt động thế nào ở hiện tại và bạn phải — hoặc không phải — tin
vào những gì để dùng nó. Nó chọn thật thà thay vì tiếp thị. Vela đang ở
<a href="/blog/vela-is-in-alpha">giai đoạn alpha</a> — hãy bắt đầu với số nhỏ. Vela
không có token. Mọi điều ở đây đều kiểm chứng được bằng mã nguồn mở.
</Callout>

## Tóm tắt

Vela là **ví hợp đồng thông minh tự quản** cho các mạng EVM. Mỗi ví là một tài khoản
thông minh [Safe](https://github.com/safe-fndn/safe-smart-account) do một **passkey**
kiểm soát — một chứng danh WebAuthn (P-256) do hệ điều hành của thiết bị giữ, mã hóa
đầu-cuối, và mở khóa bằng Face ID, Touch ID hoặc vân tay. Không có cụm từ khôi phục,
không có khóa riêng tư nào để bạn chép, cất hay đánh mất.

Vela, với tư cách công ty, không bao giờ giữ khóa hay tiền của bạn và **không thể
chuyển, đóng băng hay tịch thu chúng**. Ứng dụng, relay giao dịch và các dịch vụ hỗ
trợ đều mã nguồn mở và tự vận hành được. Thứ bạn phải tin rút lại chỉ còn: các hợp
đồng thông minh đã được kiểm toán, kho passkey của hệ điều hành, và — chỉ cho tính sẵn
sàng — một relay mà bạn thay được hoặc tự chạy được.

## Vì sao Vela tồn tại

Hầu hết ví bắt bạn đánh đổi:

- **Ví dùng cụm từ khôi phục** đặt một bí mật 12–24 từ trước mặt mọi người dùng. Đó là
  điểm hỏng duy nhất và là mục tiêu lừa đảo thường trực.
- **Ví lưu ký** bỏ đi cụm từ khôi phục nhưng nắm giữ tiền của bạn — đưa trở lại đúng
  cái rủi ro đối tác mà crypto sinh ra để loại bỏ.
- **Ký mù** — duyệt những dòng hex khó hiểu mà bạn không đọc nổi — đã trở thành bình
  thường trong hệ sinh thái và đứng sau một phần lớn các vụ ví bị rút sạch.

Vela muốn dễ dùng như một ứng dụng lưu ký trong khi vẫn giữ bạn hoàn toàn tự quản:
không cụm từ khôi phục, không ai giữ tiền, và không có giao dịch nào bạn không đọc được
trước khi ký.

## Nguyên tắc thiết kế

1. **Tự quản, không ngoại lệ.** Khóa được tạo ra trên thiết bị của bạn và do trình cung
   cấp passkey của hệ điều hành giữ, mã hóa đầu-cuối. Máy chủ của Vela chỉ thấy dữ liệu
   công khai.
2. **Hãy kiểm chứng, đừng tin suông.** Toàn bộ hệ thống — ứng dụng và cả bốn dịch vụ
   phía sau — đều mã nguồn mở theo giấy phép MIT.
3. **Không ký mù.** Giao dịch được giải mã thành ý định con người đọc được ở mọi nơi có
   mô tả; những lệnh gọi lạ bị đánh dấu chứ không bị giấu.
4. **Làm ít thôi.** Ví giữ ETH và token ERC-20 và kết nối tới những dApp bạn chọn. Ít mã
   phải tin hơn, bề mặt tấn công nhỏ hơn.

## Kiến trúc

```text
Ứng dụng Vela (iOS / Android / web, một mã nguồn)
  • Passkey (WebAuthn P-256, trình cung cấp passkey của hệ điều hành)
  • Dựng và ký UserOperation
  • Giao diện ký minh bạch (ERC-7730)
        │  UserOperation đã ký
        ▼
Relay của Vela (ERC-4337, tự vận hành được)
  • Gửi handleOps tới EntryPoint
  • Không thể sửa hay giả mạo giao dịch của bạn
        ▼
Chuỗi EVM
  EntryPoint v0.7 → tài khoản thông minh Safe
  Bộ ký WebAuthn xác minh P-256 trên chuỗi
```

### Mô hình tài khoản

Ví của bạn là một tài khoản thông minh **Safe v1.4.1** (một hợp đồng proxy) vận hành
qua trừu tượng hóa tài khoản **ERC-4337** (EntryPoint v0.7) với **Safe 4337 Module** và
một **bộ ký WebAuthn** làm chủ sở hữu tài khoản.

Địa chỉ mang tính **tất định** và **phản thực**: nó được tính từ khóa công khai của
passkey bằng `CREATE2` trước khi có giao dịch nào được gửi, nên bạn nhận tiền vào đó
được trước cả khi nó được triển khai. Tài khoản tự triển khai chính mình, trả bằng số
dư của nó, trong giao dịch đầu tiên của bạn.

### Khóa và xác thực

Việc xác thực dùng **passkey WebAuthn** trên đường cong **P-256**. Khóa riêng tư được
tạo trên thiết bị của bạn và do trình cung cấp passkey của hệ điều hành (Chuỗi khóa
iCloud hoặc Trình quản lý mật khẩu Google) giữ, mã hóa đầu-cuối, đồng bộ giữa các thiết
bị của bạn. **Máy chủ của Vela chỉ thấy khóa công khai của bạn.** Mỗi lần ký đều cần xác
thực sinh trắc học mới — không có khóa phiên sống lâu. Xem
[passkey hoạt động thế nào](/vi/docs/passkeys) để biết chi tiết.

### Ký và luồng giao dịch

1. **Dựng** một `UserOperation` ERC-4337 cho Safe của bạn và ước tính gas.
2. **Giải mã** lệnh gọi thành ý định con người đọc được và hiện ra để bạn xem.
3. **Ký** — thiết bị của bạn tạo một xác nhận WebAuthn trên digest của thao tác sau khi
   xác thực sinh trắc học.
4. **Mã hóa** xác nhận đó thành một chữ ký hợp đồng **EIP-1271**.
5. **Chuyển tiếp** thao tác đã ký tới relay, relay gửi nó tới EntryPoint.
6. **Xác minh trên chuỗi** — Safe xác minh chữ ký P-256 ngay trên chuỗi qua precompile
   RIP-7212 trước khi thực thi. Precompile là điều kiện bắt buộc: không có bộ xác minh
   dự phòng, và Vela từ chối bật một mạng thiếu nó.

Relay nhận một thao tác **đã được ký sẵn**. Nó không thể đổi người nhận, số tiền hay bất
kỳ trường nào mà không làm chữ ký mất hiệu lực.

### Relay và mô hình gas

- Gas trả **từ chính số dư ví của bạn** — mặc định bằng token gốc của mạng, hoặc bằng
  một stablecoin được hỗ trợ ở nơi relay có cung cấp. Tempo không có đồng gốc nên gas ở
  đó luôn thanh toán bằng stablecoin USD. Không có **paymaster** và không có bên thứ ba
  nào tài trợ — hay chặn — giao dịch của bạn.
- **Relay là nguồn sự thật duy nhất về giá gas.** Nó báo giá theo điều kiện chuỗi thời
  gian thực; ví hiển thị báo giá đó và ký đúng thứ nó hiển thị.
- Phần thu của Vela cố tình đơn giản: tổng là **chi phí mạng cộng phí dịch vụ của
  relay**, với một mức tối thiểu nhỏ cho những giao dịch rất rẻ. Một phần đi về
  validator của chuỗi; phần còn lại trả cho relay, bên vận hành hạ tầng và giữ cho tài
  khoản gas của bạn có tiền.
- Ví **hiện phí ước tính trước khi bạn xác nhận** — theo tài sản trả phí và theo đơn vị
  tiền hiển thị của bạn — và số tiền báo giá cùng người nhận nó là một phần của thứ bạn
  ký, nên relay được trả đúng bằng con số đã hiện. Không có phần chênh giấu mặt.
- Mỗi Safe có một **tài khoản relay riêng** (tài khoản gas) trên từng chuỗi, kích hoạt
  bằng một khoản đặt cọc **không hoàn lại**. Nó có thể cạn dần, nên về sau có thể cần
  **kích hoạt lại** — không hẳn là một khoản đặt cọc một lần.

Relay là phụ thuộc về **tính sẵn sàng**, không phải về **quyền giữ tiền**: nó có thể
trì hoãn hoặc từ chối chuyển tiếp, nhưng không bao giờ sửa, giả mạo hay trộm được. Nó
là mã nguồn mở và bạn tự chạy được; và vì giá được **báo ra và hiện lên** thay vì giấu
đi, phí của cả một relay tự dựng lẫn của bên thứ ba đều luôn hiện rõ trước khi bạn ký.
Xem [mạng & phí](/vi/docs/networks-and-fees).

### Ký minh bạch (ERC-7730)

Vela giải mã calldata và dữ liệu có kiểu EIP-712 bằng các mô tả **ERC-7730** và vẽ ra
**ý định** (Hoán đổi, Gửi, Duyệt…), **phần cốt lõi** (số tiền, địa chỉ) và **chi tiết**
(nonce, hạn chót, calldata thô) khi được yêu cầu, có mã màu theo rủi ro. Khi không mô tả
nào khớp, Vela hiện cảnh báo ký mù rõ ràng thay vì giả vờ hiểu lệnh gọi đó.

### Mạng

Vela hỗ trợ 12 mạng EVM — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base,
Avalanche, Gnosis, Unichain, Tempo, Monad và World Chain — cộng thêm mạng tùy chọn. Một
mạng tùy chọn chỉ thêm được nếu nó đã có sẵn những hợp đồng Vela dựa vào (EntryPoint,
các hợp đồng Safe, bộ ký WebAuthn) và precompile P-256 RIP-7212; Vela kiểm tra trước
khi bật.

## Mô hình bảo mật

**Những gì Vela không thể làm:**

- Chuyển, tiêu hay sang nhượng tiền của bạn — chỉ passkey của bạn mới ủy quyền được cho
  Safe.
- Đóng băng hay tịch thu tài khoản của bạn — Safe là hợp đồng của bạn trên chuỗi; Vela
  không có vai trò đặc quyền nào trên đó.
- Ký thay bạn — mỗi giao dịch đều cần một xác nhận sinh trắc học mới.
- Thấy khóa riêng tư của bạn — nó không bao giờ tới Vela; chỉ thiết bị của bạn dùng được
  nó để ký.
- Sửa một giao dịch sau khi bạn ký — mọi thay đổi đều làm chữ ký mất hiệu lực.

**Điều mà "không thể đóng băng" không bao gồm:** bản thân *token*. Một stablecoin có
kiểm soát — USDC, USDT và hầu hết token bảo chứng bằng tiền pháp định — mang theo một
hàm danh sách đen mà tổ chức phát hành có thể gọi lên bất kỳ địa chỉ nào, kể cả của bạn.
Quyền đó thuộc về tổ chức phát hành và tồn tại dù bạn giữ token trong ví nào; không ví
tự quản nào, kể cả Vela, lấy nó đi được. Cái mà tự quản mang lại là: **chúng tôi** không
phải một bên thứ hai cũng làm được như vậy.

**Những gì bạn phải tin:**

- **Các hợp đồng Safe** (đã kiểm toán, dùng rộng rãi) và bộ ký WebAuthn xác minh khóa
  P-256 của bạn.
- **Trình cung cấp passkey của hệ điều hành** (Apple / Google) trong việc bảo vệ và đồng
  bộ chứng danh của bạn.
- **Các nhà cung cấp RPC** bạn truy vấn (Vela dùng một nhóm nhiều nguồn có chuyển dự
  phòng; bạn tự đặt được nguồn riêng).
- **Relay**, chỉ về tính sẵn sàng — và bạn tự vận hành được nó.

**Các mối đe dọa đã tính đến:**

- **Mất hoặc bị trộm thiết bị** — kẻ trộm vẫn cần sinh trắc học/mã PIN của bạn để ký.
- **Lừa đảo / dApp độc hại** — được xử lý bằng ký minh bạch.
- **Máy chủ Vela bị chiếm** — không đem lại khả năng ký; thiệt hại tối đa là dịch vụ
  suy giảm, không phải mất tiền.
- **Rủi ro chuỗi cung ứng** — giảm nhẹ bằng mã nguồn mở và tự vận hành.

## Khôi phục

Passkey của bạn được trình cung cấp của hệ điều hành sao lưu; trên thiết bị mới, đăng
nhập bằng chính tài khoản Apple hay Google đó sẽ khôi phục nó, và ví của bạn hiện lại.

<Callout type="warning" title="Bản sao lưu passkey của nền tảng chính là cách khôi phục của bạn">
Khôi phục của Vela chính là passkey của bạn, được Chuỗi khóa iCloud hoặc Trình quản lý
mật khẩu Google đồng bộ. Theo thiết kế, không có cụm từ khôi phục, không có khôi phục
xã hội, không có người giám hộ — không có gì để Vela đánh mất, làm lộ hay bị ép phải
dùng. Mặt trái là có thật: nếu bạn mất <strong>cả</strong> thiết bị
<strong>lẫn</strong> passkey đồng bộ trên đám mây, mà không còn bản sao nào khác, thì
tài khoản không khôi phục được. Hãy bật sao lưu passkey của nền tảng và bảo vệ tài khoản
đó.
</Callout>

Mô hình khôi phục đầy đủ, kể cả những giới hạn thật thà, nằm ở
[khôi phục & đăng nhập](/vi/docs/recovery).

## Nếu Vela biến mất

Tự quản nghĩa là khóa và tiền của bạn không phụ thuộc vào việc Vela còn online. Tiền nằm
trong **hợp đồng Safe của chính bạn trên chuỗi**, còn relay thì mã nguồn mở và thay thế
được.

Một lưu ý thật thà: WebAuthn buộc passkey vào tên miền của bên tin cậy (`getvela.app`).
Nếu tên miền đó mất vĩnh viễn, những passkey gắn với nó sẽ cần trợ giúp để hoạt động ở
nơi khác — một công cụ có thể trình ra bên tin cậy gốc cho bộ xác thực. Vela từng phát
hành một tiện ích trình duyệt mức dành cho lập trình viên cho tình huống đó và đã ngừng
nó vào tháng 9/2026; một đường khôi phục mức người dùng phổ thông cho trường hợp mất tên
miền vẫn là việc còn dang dở, và chúng tôi nói thẳng ra thay vì ngụ ý rằng nó đã có. Việc
truy cập độc lập trên chuỗi còn phụ thuộc vào hỗ trợ P-256 (RIP-7212) của chuỗi đích, vốn
đang dần phổ biến hơn.

## Quyền riêng tư

Không tài khoản, không email, không KYC, không cụm từ khôi phục nào để thu thập. Máy chủ
chỉ lưu **khóa công khai** của bạn và cái tên tài khoản bạn chọn (để khôi phục xuyên
thiết bị), vốn được công bố trên chuỗi theo thiết kế. Nội dung giao dịch không bị ghi
log. Trang web dùng công cụ phân tích tự vận hành, không cookie. Xem
[chính sách riêng tư](/privacy).

## Kiểm chứng được và mã nguồn mở

Mọi thứ đều **mã nguồn mở theo giấy phép MIT** — ứng dụng và cả bốn dịch vụ phía sau (dữ
liệu chuỗi, chỉ mục passkey, relay, tỷ giá), và bạn **tự vận hành được** (Cài đặt →
Nâng cao → Điểm cuối dịch vụ). Đọc mã tại
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet).

## Không có token

Vela **không có token** và không có kế hoạch phát hành. Không có gì để mua, để farm hay
để đầu cơ. Gas trả bằng tài sản gốc của từng mạng.

## Tình trạng kiểm toán và giới hạn

**Các hợp đồng Safe** ở lõi mỗi tài khoản Vela đã được kiểm toán độc lập và tôi luyện
qua thực tế. **Phần tích hợp của chính Vela** quanh chúng **chưa qua kiểm toán độc lập
của bên thứ ba**, và hiện chưa có lịch — một cuộc kiểm toán chuyên nghiệp là mục tiêu
cho lúc dự án đủ sức chi trả, không phải một cam kết có ngày tháng. Cho tới lúc đó, việc
rà soát phần tích hợp là không chính thức: mã nguồn mở, và nó dựa vào những người có
năng lực và quan tâm trong cộng đồng đọc nó, cùng với rà soát có hỗ trợ của AI. Việc đó
có ích, nhưng không tương đương một cuộc kiểm toán chuyên nghiệp. Hãy coi Vela là phần
mềm alpha và chỉ dùng số tiền mà bạn thấy thoải mái khi đặt vào một thứ còn non trẻ như
vậy.

## Tham chiếu

- ERC-4337 — Trừu tượng hóa tài khoản qua EntryPoint
- EIP-1271 — Chuẩn xác thực chữ ký cho hợp đồng
- ERC-7730 — Ký minh bạch / mô tả dữ liệu có cấu trúc
- EIP-5792 — Gộp lô lệnh gọi của ví
- RIP-7212 — Precompile xác minh chữ ký secp256r1 (P-256)
- WebAuthn / FIDO2 — Xác thực bằng passkey
- [Tài khoản thông minh Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
