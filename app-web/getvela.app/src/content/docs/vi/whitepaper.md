---
title: Whitepaper
description: "Vela hoạt động thế nào và bạn phải — cũng như không phải — tin những gì khi dùng nó: tài khoản, khóa, phí, mô hình mối đe dọa, khôi phục, và chuyện gì xảy ra nếu Vela biến mất."
source: 60d297b650ac
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Tình trạng: alpha · sửa đổi lần cuối tháng 9/2026">
Trang này mô tả Vela hoạt động thế nào hiện nay và bạn phải hay không phải tin những gì
khi dùng nó. Vela đang ở giai đoạn <a href="/blog/vela-is-in-alpha">alpha</a> — hãy bắt
đầu với số tiền nhỏ. Vela không có token. Mọi điều ở đây đều đối chiếu được với mã nguồn
mở; chỗ nào mã nguồn và trang này không khớp, mã nguồn là đúng và trang này là lỗi.
</Callout>

## Tóm tắt

Vela là một **ví hợp đồng thông minh tự lưu ký** cho Ethereum và các mạng EVM khác. Mỗi
ví là một tài khoản **Safe v1.4.1** nguyên bản, vận hành qua **ERC-4337**, và được kiểm
soát bởi tối đa bảy **passkey** — các khóa WebAuthn P-256 do thiết bị của bạn, trình quản
lý mật khẩu của bạn, hoặc khóa bảo mật phần cứng nắm giữ. Không có cụm từ khôi phục.

Vela, với tư cách công ty, không bao giờ giữ khóa của bạn và không có vai trò gì trên Safe
của bạn, nên tự mình nó **không thể chuyển, đóng băng hay chiếm đoạt tiền của bạn**. Nhưng
Vela có viết và cung cấp phần mềm yêu cầu các khóa của bạn ký — đó là lý do mô hình mối đe
dọa bên dưới quan trọng. Các ứng dụng, relay gửi giao dịch lên chuỗi, và các dịch vụ hỗ
trợ đều là mã nguồn mở, và bạn có thể tự chạy bản sao của từng thứ. Tóm lại, những gì bạn
phải tin: các hợp đồng, các trình xác thực giữ khóa của bạn, mã của ứng dụng bạn dùng để ký,
tên miền mà passkey của bạn thuộc về, và các dịch vụ mà bạn trỏ ứng dụng tới.

## Vì sao có Vela

- **Ví dùng cụm từ khôi phục** đặt một bí mật 12–24 từ trước mặt mọi người dùng: một điểm
  hỏng duy nhất và một mục tiêu lừa đảo thường trực.
- **Ví lưu ký** bỏ được cụm từ khôi phục bằng cách giữ luôn tiền của bạn.
- **Ví passkey** phụ thuộc vào máy chủ và mã đóng của một công ty thì bỏ được cụm từ khôi
  phục, nhưng bỏ rơi bạn nếu công ty đó biến mất.
- **Ký mù** — duyệt dữ liệu khó hiểu mà bạn không đọc được — vẫn còn phổ biến, và là một
  trong những con đường khiến ví bị rút sạch.

Vela hướng tới sự tiện lợi của passkey mà không kèm theo bất kỳ sự phụ thuộc nào ở trên:
một tài khoản tiêu chuẩn, mã nguồn mở, dịch vụ thay thế được, và giao dịch bạn đọc được
trước khi ký.

## Nguyên tắc thiết kế

1. **Tự lưu ký, không ngoại lệ.** Khóa do trình xác thực của bạn tạo và giữ. Các dịch vụ
   của Vela không bao giờ thấy chúng; những gì các dịch vụ đó thấy được liệt kê trong phần
   Quyền riêng tư.
2. **Hợp đồng tiêu chuẩn, không sửa đổi.** Không hợp đồng nào trên đường đi tới tiền của
   bạn do Vela viết.
3. **Kiểm chứng, đừng tin suông.** Các ứng dụng và dịch vụ đều công khai; các dịch vụ có
   thể tự triển khai.
4. **Giải mã trước khi ký.** Những gì không giải mã được đều kèm cảnh báo ký mù rõ ràng.
5. **Làm ít hơn.** Chiếc ví gửi, nhận, và ký cho những dApp bạn chọn.

## Kiến trúc

```text
Các ứng dụng Vela — web, tiện ích trình duyệt, máy tính (macOS/Windows/Linux), iOS, Android
  một lõi Rust dùng chung (quy tắc, mật mã, ABI, ký minh bạch) + vỏ gốc cho từng nền tảng
  • dựng UserOperation và cho bạn thấy nó làm gì
  • xin khóa của bạn một xác nhận WebAuthn
        │  UserOperation đã ký (đã gồm phí)
        ▼
Relay (vela-relay, tự triển khai được)
  • báo giá phí, trả trước gas, gửi handleOps
  • không thể thay đổi thao tác
        ▼
Chuỗi EVM
  EntryPoint v0.7 → Safe v1.4.1 của bạn → mô-đun 4337 của Safe
  Mô-đun passkey của Safe xác minh P-256 qua precompile EIP-7951 / RIP-7212
```

Các dịch vụ hỗ trợ, tất cả đều mã nguồn mở: một **chỉ mục khóa công khai** đăng ký ví mới
vào một sổ đăng ký trên chuỗi và trả lời các yêu cầu tra cứu, một danh mục **dữ liệu
chuỗi**, và một nguồn **tỷ giá**. Xem [hướng dẫn tự triển khai](/vi/docs/self-hosting).

### Tài khoản

Ví của bạn là một proxy **Safe v1.4.1** (singleton SafeL2), với **mô-đun 4337 v0.3.0** của
Safe được bật làm mô-đun và fallback handler, vận hành qua **EntryPoint v0.7**. Các chủ sở
hữu của nó là các bộ ký passkey từ **mô-đun passkey v0.2.1** của Safe: khóa đầu tiên được
bộ ký dùng chung xác minh, và mỗi khóa thêm vào được xác minh bởi hợp đồng ký riêng do
factory của Safe tạo ra. Ngưỡng là **1**.

Địa chỉ mang tính **tất định và phản thực**: nó được tính bằng `CREATE2` từ dữ liệu thiết
lập Safe, vốn chứa mọi khóa ban đầu, trước khi có bất cứ thứ gì được triển khai. Địa chỉ
giống nhau trên mọi mạng. Bạn nhận tiền vào đó được ngay; giao dịch đầu tiên của bạn trên
mỗi mạng sẽ triển khai ví và trả chi phí đó ngay trong phí của giao dịch.

### Khóa

Một ví có **từ một đến bảy khóa**, cố định khi bạn tạo ví. Bất kỳ khóa nào cũng tự ký được
một mình (1-of-n). Một khóa có thể là:

- một passkey trên thiết bị bạn đang dùng — được Chuỗi khóa iCloud, Trình quản lý mật khẩu
  của Google hoặc một trình quản lý mật khẩu khác đồng bộ nếu bạn cho phép;
- một điện thoại khác, kết nối bằng cách quét mã QR (kênh truyền hybrid của WebAuthn);
- một khóa bảo mật phần cứng qua USB hoặc NFC, không đồng bộ đi đâu cả.

Mọi chữ ký đều cần bước xác minh người dùng của chính trình xác thực — sinh trắc học hoặc mã
PIN thiết bị, hoặc mã PIN và thao tác chạm trên khóa bảo mật. Không có khóa phiên. Không thể
thêm, gỡ bỏ hay thay khóa về sau: trên mọi chuỗi mà ví chưa được triển khai, địa chỉ vẫn đại
diện cho tập khóa ban đầu, nên đổi chủ sở hữu trên một chuỗi sẽ khiến tài khoản khác nhau
giữa các chuỗi.

Passkey thuộc về một bên phụ thuộc (relying party) — passkey của Vela được tạo cho
**`getvela.app`**. Trình duyệt chỉ đưa chúng cho các trang trên getvela.app hoặc các tên miền
con của nó, điều khiến chúng chống được lừa đảo; đó cũng là một sự phụ thuộc mà tài liệu này
sẽ quay lại bên dưới.

### Luồng ký

1. **Dựng** một UserOperation cho Safe của bạn — bao gồm một lệnh chuyển trả phí cho relay
   — và mô phỏng nó.
2. **Giải mã** nó thành ý định con người đọc được và hiện cho bạn.
3. **Ký**: sau khi xác minh bạn, trình xác thực tạo một xác nhận WebAuthn trên hash của thao
   tác.
4. **Mã hóa** xác nhận đó thành dạng chữ ký Safe mà mô-đun passkey mong đợi.
5. **Gửi** thao tác đã ký tới relay, relay gọi EntryPoint.
6. **Xác minh trên chuỗi**: mô-đun passkey kiểm tra chữ ký P-256 bằng precompile EIP-7951 / RIP-7212
   trước khi Safe thực thi bất cứ điều gì. Không có bộ xác minh dự phòng; một mạng không có
   precompile thì không thể thêm vào.

### Phí

- Relay được trả **ngay trong thao tác** (in band): thao tác khai báo phí EntryPoint bằng
  không và kèm một lệnh chuyển từ Safe của bạn tới địa chỉ của relay. Số tiền và người nhận
  là một phần của thứ bạn ký, nên bạn trả đúng số tiền màn hình xác nhận đã hiện.
- Phí bằng **ba lần lượng gas ví dự trù cho thao tác** (các ước tính mô phỏng được nâng thêm
  một nửa, kèm mức tối thiểu), **tính theo mức cao hơn giữa giá gas ví tự đọc được và giá
  relay báo cho tốc độ đã chọn**, tối thiểu khoảng 0,01 USD. Trên Tempo, hệ số là hai. Phần
  độn thêm và biên độ trong giá khiến phí cao hơn chi phí thực của thao tác trên chuỗi, nhất
  là ở giao dịch đầu tiên trên một mạng; relay giữ phần chênh lệch. Số tiền chính xác nằm
  trên màn hình xác nhận trước khi bạn ký.
- Phí được trả cho relay mà ví đang dùng: mặc định là relay của Vela, hoặc bất kỳ bản triển
  khai vela-relay nào, kể cả bản do bạn tự chạy.
- Phí được trả bằng coin của mạng hoặc bằng một stablecoin USD mà relay chấp nhận (pathUSD
  trên Tempo, mạng không có coin gốc). **Không có paymaster**: không ai tài trợ gas, và cũng
  không ai có thể lọc giao dịch qua một chính sách tài trợ.
- Nếu ngân quỹ gas của relay trên một mạng đã cạn, ví sẽ báo trước khi bạn ký. Không có khoản
  đặt cọc nào cho từng người dùng.

Chi tiết: [mạng & phí](/vi/docs/networks-and-fees).

### Ký minh bạch

Các lệnh gọi và thông điệp EIP-712 được giải mã bằng bộ mô tả **ERC-7730** — có sẵn trong
ứng dụng cho các hợp đồng phổ biến, lấy từ dịch vụ dữ liệu chuỗi, hoặc khớp với các dạng
token tiêu chuẩn — rồi, như phương án cuối cùng, một cơ sở dữ liệu selector công khai, được
gắn nhãn giải mã tốt nhất có thể. Những gì còn lại đều nhận cảnh báo ký mù rõ ràng. Một bộ
mô tả lấy về không bao giờ được gắn nhãn đã xác minh — chỉ bộ mô tả có sẵn trong ứng dụng,
hoặc bộ lấy về mà giống hệt bản có sẵn, mới xứng với chữ đó. Một lệnh cấp quyền trên chuỗi
ở mức "không giới hạn" (từ 2^200 trở lên) không thể gửi đi cho đến khi bạn giảm nó xuống;
một lệnh cấp quyền lớn nhưng hữu hạn và các permit dạng chữ ký thì hiện kèm cảnh báo thận
trọng nhưng không bị chặn. Chi tiết: [ký minh bạch](/vi/docs/clear-signing).

### Mạng

Vela có 24 mạng tích hợp sẵn — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base,
Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain, Arc, X Layer, Stable, Soneium,
MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume và XRPL EVM — và chấp nhận bất kỳ
mạng EVM nào có mười hai hợp đồng mà nó kiểm tra cùng precompile EIP-7951 / RIP-7212. Hai trong số
mười hai hợp đồng đó là factory tạo bộ ký passkey của Safe và mã bộ ký mà factory này
triển khai; chỉ ví có nhiều hơn một khóa mới cần chúng, và bước kiểm tra báo riêng hai
hợp đồng này.

## Mô hình bảo mật

**Những gì Vela không thể làm**

- Tự mình chuyển, tiêu hay đóng băng tiền của bạn — chỉ các khóa của bạn mới phê duyệt được
  cho Safe của bạn, và Vela không có vai trò gì trên nó. (Điều Vela làm được là phát hành
  phần mềm yêu cầu bạn ký; xem các mối đe dọa bên dưới.)
- Sửa một giao dịch sau khi bạn đã ký — mọi thay đổi đều làm chữ ký mất hiệu lực.
- Đọc khóa riêng tư của bạn — chúng ở lại trong các trình xác thực của bạn.
- Thêm một khóa vào ví của bạn, hoặc gỡ bỏ một khóa.

**Điều mà "không thể đóng băng" không bao gồm: chính token.** USDC, USDT và hầu hết các token
bảo chứng bằng tiền pháp định cho phép bên phát hành đưa bất kỳ địa chỉ nào vào danh sách đen,
kể cả địa chỉ của bạn. Quyền đó thuộc về bên phát hành và tồn tại dù bạn dùng ví nào. Điều tự
lưu ký mang lại cho bạn là Vela không phải một bên thứ hai có được quyền đó.

**Những gì bạn phải tin**

- **Các hợp đồng**: Safe, mô-đun 4337 và mô-đun passkey của nó, EntryPoint v0.7, và precompile
  EIP-7951 / RIP-7212 của chuỗi.
- **Tên miền**: bất kỳ trang nào được phục vụ từ getvela.app hoặc một tên miền con của nó đều
  có thể xin các khóa của bạn một chữ ký.
- **Các trình xác thực** giữ khóa của bạn, và — với passkey được đồng bộ — tài khoản Apple,
  Google hoặc trình quản lý mật khẩu đứng sau chúng.
- **Mã của ứng dụng bạn dùng để ký.** Nó dựng giao dịch và cho bạn thấy giao dịch làm gì. Một
  ứng dụng bị xâm nhập có thể cho bạn xem một thứ và yêu cầu bạn ký một thứ khác; lời nhắc của
  trình xác thực sẽ không cho bạn biết sự khác biệt.
- **Các điểm cuối RPC** bạn đọc dữ liệu từ đó: một nút nói dối có thể hiện sai số dư hoặc hiện
  sai bản xem trước mô phỏng. Bạn có thể đặt điểm cuối của riêng mình.
- **Các dịch vụ dữ liệu chuỗi và tỷ giá**: chúng cung cấp danh sách token, bộ mô tả, danh sách
  token trả phí và tỷ giá dùng để đổi một số tiền pháp định thành số lượng token.
- **Relay**: nó không thể sửa những gì bạn đã ký, nhưng có thể trì hoãn hoặc từ chối, chọn thời
  điểm giao dịch lên chuỗi (nên nó có thể chạy trước một lệnh hoán đổi trong phạm vi trượt giá
  của bạn), và đặt giá gas làm cơ sở tính phí của bạn, tối đa gấp ba mức ví tự đọc được.

**Các mối đe dọa đã được xem xét**

- **Mất hoặc bị trộm thiết bị** — kẻ trộm vẫn phải vượt qua bước kiểm tra của trình xác thực;
  một khóa khác giúp bạn lấy lại quyền truy cập. Nhưng không thể gỡ bỏ khóa: nếu một khóa có
  thể đã rơi vào tay người khác, hãy chuyển tiền sang một ví mới, vì địa chỉ cũ vẫn tiêu được
  bằng khóa đó trên mọi mạng.
- **Lừa đảo** — không thể gõ passkey vào một trang giả, và trình duyệt chỉ đưa nó cho các trang
  trên getvela.app và các tên miền con của nó.
- **dApp độc hại** — được xử lý bằng ký minh bạch, lớp chặn cấp quyền và một lần từ chối
  thẳng: yêu cầu một lệnh gọi từ Safe của bạn tới chính nó — `enableModule`,
  `addOwnerWithThreshold`, `swapOwner`, `setFallbackHandler`, `setGuard` và những lệnh còn lại
  trong họ đó — đều bị chặn, kể cả khi nằm trong một giao dịch gộp hay một `MultiSend`; mọi
  nhánh mang `delegatecall` và chữ ký dữ liệu có cấu trúc `SafeTx` cũng vậy. Bất kỳ lệnh nào
  trong số đó, chỉ cần ký một lần, cũng sẽ trao tài khoản đi trọn vẹn như dữ liệu trong vụ
  Bybit, nên ví không hề đưa chúng ra để ký.
- **Dịch vụ phía sau bị xâm nhập** (relay, chỉ mục, dữ liệu chuỗi, tỷ giá) — không có quyền ký,
  nhưng có ảnh hưởng thật: từ chối phục vụ, bộ mô tả hoặc danh sách token gây hiểu lầm, tỷ giá
  sai làm thay đổi số tiền thực gửi đi khi bạn nhập theo tiền pháp định, và (với relay) thời
  điểm cùng giá gas đã nêu ở trên. Một phần mô tả lấy từ dịch vụ dữ liệu chuỗi không bao giờ
  được gọi là đã xác minh — chỉ phần mô tả có sẵn trong ứng dụng, hoặc phần lấy về mà giống
  hệt bản có sẵn, mới xứng với chữ đó; phần còn lại vẫn hiện kèm một dòng nói rằng không có
  gì xác thực chúng. Mỗi dịch vụ đều thay được.
- **Kênh phân phối ứng dụng bị xâm nhập** — một bản triển khai web, bản cập nhật tiện ích hay
  bản build ứng dụng bị sửa đổi có thể đưa ra một giao dịch độc hại để bạn ký. Đây là lớp tấn
  công kiểu [Bybit](/vi/docs/bybit-attack). Biện pháp giảm nhẹ hiện nay còn hạn chế: phần giải
  mã và lớp chặn cấp quyền trong chính ứng dụng, các bản build macOS đã công chứng, và việc tự
  biên dịch tiện ích hoặc ứng dụng từ mã nguồn (các gói phát hành có checksum SHA-256 và các
  attestation nguồn gốc bản dựng của GitHub nêu rõ commit và lần chạy workflow; trình cài
  đặt Windows vẫn chưa được ký mã). Một trang ký độc lập không dùng chung mã với ứng dụng đã được làm xong nhưng
  chưa được kết nối.
- **Bất cứ thứ gì được phục vụ từ tên miền** — bất kỳ trang nào trên getvela.app hoặc các tên
  miền con của nó, kể cả một script mà trang đó tải, đều có thể xin chữ ký từ passkey của Vela,
  và lời nhắc chỉ hiện "getvela.app". Vì vậy trang web cấm chính các trang của mình dùng
  passkey, và không tải script phân tích trên trang đang giữ khóa. Nếu tên miền đổi chủ, chủ
  mới cũng sẽ kiểm soát những ứng dụng nào được dùng passkey. Tiện ích và ứng dụng tự biên dịch
  mang theo mã của riêng mình, dù theo mặc định chúng vẫn lấy bộ mô tả và dùng các dịch vụ dưới
  getvela.app.

## Khôi phục

Tạo ví sẽ công bố khóa công khai và địa chỉ của ví lên một **hợp đồng sổ đăng ký** công khai
trên Gnosis (có thể sao chép sang Ethereum). Trên thiết bị mới, bạn đăng nhập bằng **bất kỳ
khóa nào**; ứng dụng tìm ví qua chỉ mục hoặc, nếu không được, đọc thẳng từ sổ đăng ký, và
kiểm tra rằng các khóa tính lại ra đúng địa chỉ đã ghi. Một ví chỉ có một khóa còn có thể
được dựng lại từ hai chữ ký mà hoàn toàn không cần sổ đăng ký.

<Callout type="warning" title="Khóa của bạn chính là cách khôi phục">
Không có cụm từ khôi phục, không có khôi phục xã hội và không có người giám hộ — không có gì
mà Vela có thể làm mất, làm lộ, hay bị ép phải dùng. Nếu mọi khóa ban đầu đều mất, ví không
thể khôi phục. Hãy tạo ví với nhiều hơn một khóa, giữ đồng bộ passkey luôn bật nếu bạn dựa vào
nó, và bảo vệ tài khoản đứng sau nó.
</Callout>

Chi tiết: [khôi phục & đăng nhập](/vi/docs/recovery).

## Nếu Vela biến mất

Tiền của bạn vẫn nằm trong Safe của bạn trên chuỗi. Các hợp đồng không phụ thuộc vào Vela, và
mọi dịch vụ Vela vận hành đều là mã nguồn mở để người khác chạy. Thứ duy nhất không thể dời
đi là bên phụ thuộc của passkey, `getvela.app`: một bản sao ví web trên tên miền khác sẽ tạo
ra một ví khác. Với các ví
đã có, tiện ích trình duyệt Vela (được phép dùng passkey của `getvela.app`) và các ứng dụng bạn
tự biên dịch (với điện thoại hoặc khóa bảo mật) vẫn tiếp tục hoạt động khi không có
getvela.app. [Hướng dẫn tự triển khai](/vi/docs/self-hosting#if-getvela-app-disappears) trình
bày rõ từng con đường và giới hạn của nó. Truy cập độc lập trên một chuỗi cũng đòi hỏi chuỗi đó
hỗ trợ EIP-7951 / RIP-7212.

## Quyền riêng tư

Không tài khoản, không email, không KYC. Những gì trở nên công khai được ghi vào sổ đăng ký khi
bạn tạo ví: khóa công khai và ID thông tin xác thực của từng khóa, mẫu trình xác thực, tên ví và
nhãn các khóa của bạn, địa chỉ, và dữ liệu đăng ký đã ký. Chỉ mục của Vela thấy bản ghi đó trước
khi gửi lên, và thấy những địa chỉ bạn tra tên; relay của Vela thấy địa chỉ của bạn, các thao
tác bạn gửi và điểm cuối RPC mà ứng dụng của bạn dùng (kể cả khóa API trong URL của nó), và lưu
các thao tác trong một thời gian giới hạn để thử lại và chẩn đoán lỗi. Mọi dịch vụ đều thấy địa
chỉ IP của bạn. Trang web dùng công cụ phân tích không dùng cookie.
[Chính sách quyền riêng tư](/privacy) là danh sách chính thức.

## Mã nguồn mở

Mọi thứ đều theo giấy phép MIT: chiếc ví (mọi ứng dụng và phần lõi), relay, chỉ mục khóa
công khai, dịch vụ tỷ giá và danh mục dữ liệu chuỗi. Mã nguồn: [github.com/mondaylabsltd](https://github.com/orgs/mondaylabsltd/repositories).

## Không có token

Vela không có token và không có kế hoạch phát hành token. Không có gì để mua, để farm hay để
đầu cơ. Phí được trả bằng coin của từng mạng hoặc bằng stablecoin.

## Tình trạng kiểm toán và giới hạn

Các hợp đồng của Safe, mô-đun 4337 và mô-đun passkey của Safe, cùng EntryPoint v0.7 đã được
kiểm toán độc lập và được dùng rộng rãi. **Mã của chính Vela — các ứng dụng, các dịch vụ phía
sau và hợp đồng sổ đăng ký — chưa qua một cuộc kiểm toán độc lập của bên thứ ba, và cũng chưa
có lịch kiểm toán nào**; một cuộc kiểm toán chuyên nghiệp là mục tiêu cho lúc dự án có kinh phí,
không phải một cam kết có ngày cụ thể. Cho đến lúc đó, việc rà soát là không chính thức: mã
nguồn công khai, những thành viên có năng lực trong cộng đồng đọc nó, và nó được rà soát bằng
công cụ AI. Điều đó có ích, nhưng không tương đương một cuộc kiểm toán chuyên nghiệp. Hãy coi
Vela là phần mềm alpha. Chi tiết: [kiểm toán & vấn đề đã biết](/vi/docs/security-audits).

## Tài liệu tham khảo

- ERC-4337 — Trừu tượng hóa tài khoản qua EntryPoint
- EIP-1271 — Xác thực chữ ký cho hợp đồng
- ERC-7730 — Bộ mô tả cho ký minh bạch
- EIP-5792 — Gộp lệnh gọi của ví (`wallet_sendCalls`)
- EIP-7951 / RIP-7212 — Precompile xác minh chữ ký P-256
- WebAuthn / FIDO2 — Passkey
- [Tài khoản thông minh Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
