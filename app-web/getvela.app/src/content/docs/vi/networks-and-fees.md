---
title: Mạng & phí
description: "24 mạng tích hợp sẵn trong Vela, cách thêm mạng khác, phí của một giao dịch được tính chính xác thế nào và ai nhận, và chuyện gì xảy ra khi relay hết gas."
source: 84328d162a3a
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Mạng & phí

## Các mạng tích hợp sẵn

Vela có sẵn **24 mạng**, tất cả đều là mainnet:

| Mạng | Trả gas bằng | Mạng | Trả gas bằng |
| --- | --- | --- | --- |
| Ethereum | ETH | Arc | USDC (coin gốc) |
| BNB Chain | BNB | X Layer | OKB |
| Polygon | POL | Stable | USDT0 (coin gốc) |
| Arbitrum | ETH | Soneium | ETH |
| Optimism | ETH | MegaETH | ETH |
| Base | ETH | Robinhood Chain | ETH |
| Avalanche | AVAX | Mantle | MNT |
| Gnosis | xDAI | Kaia | KAIA |
| Unichain | ETH | Celo | CELO |
| Tempo | pathUSD (không có coin gốc) | Ink | ETH |
| Monad | MON | Plume | PLUME |
| World Chain | ETH | XRPL EVM | XRP |

Trên hầu hết các mạng, bạn cũng có thể trả phí bằng một stablecoin USD mà relay chấp
nhận trên mạng đó (xem bên dưới).

Ví của bạn có **cùng một địa chỉ trên mọi mạng**, vì địa chỉ được tính từ các khóa của
bạn, không phụ thuộc vào chuỗi.

## Thêm mạng khác

Bạn có thể thêm bất kỳ mạng EVM nào trong **Cài đặt → Mạng lưới**, miễn là mạng đó có
đủ những gì một ví Vela cần: mười một hợp đồng tiêu chuẩn (EntryPoint v0.7 của
ERC-4337, các hợp đồng Safe v1.4.1, mô-đun 4337 và mô-đun passkey của Safe, MultiSend,
Multicall3 và hai bộ triển khai tất định) và precompile **RIP-7212** xác minh chữ ký
passkey tại địa chỉ `0x100`. Ví kiểm tra tất cả những thứ đó, bao gồm cả một lần kiểm
tra chữ ký thật với precompile, trước khi cho bạn thêm mạng.

Precompile là yêu cầu bắt buộc. Địa chỉ của nó là một phần trong cách tính mọi địa chỉ
Vela, nên không có bộ xác minh dự phòng và cũng không có cách nào triển khai bù về sau.
Nếu một chuỗi có precompile nhưng thiếu một số hợp đồng, trang
[thiết lập chuỗi](/vi/chain-setup) cho biết thiếu những gì và triển khai những hợp đồng
mà ai cũng triển khai được. Có một chỗ hở trong bước kiểm tra: ví có nhiều hơn một khóa
còn cần hợp đồng factory tạo bộ ký passkey của Safe trên mạng đó, thứ hiện chưa được
kiểm tra; nếu thiếu nó, chỉ khóa đầu tiên ký được trên mạng đó.

## Một giao dịch được trả phí thế nào

Vela là ví ERC-4337: bạn không tự phát giao dịch lên mạng. Ứng dụng dựng một
**UserOperation**, bạn ký nó bằng một trong các khóa của mình, và một **relay** gửi nó
lên chuỗi, trả trước tiền gas. (ERC-4337 gọi vai trò này là bundler.) Relay được hoàn
tiền **ngay bên trong thao tác của bạn**: khoản thanh toán là một lệnh chuyển từ ví của
bạn tới relay, nằm cùng lô với giao dịch của bạn, nên được chữ ký của bạn bảo vệ. Không
có paymaster, không ai tài trợ gas cho bạn, và cũng không ai có thể từ chối giao dịch
của bạn vì một chính sách tài trợ.

### Phí là bao nhiêu

Màn hình xác nhận hiện một con số duy nhất, theo token trả phí và theo tiền tệ hiển
thị của bạn. Nó được tính như sau:

- **Lượng gas ví dự trù.** Ví mô phỏng giao dịch và dự trù nhiều gas hơn mức nó dự
  kiến dùng: ước tính cho phần xác minh và phần thực thi đều được nâng thêm một nửa, kèm
  mức tối thiểu (ví dụ phần xác minh ít nhất là 300.000 gas khi ví đã được triển khai,
  và 2.000.000 cho giao dịch triển khai ví).
- **Giá gas.** Mức cao hơn giữa giá gas của mạng do ví tự đọc được và giá relay báo cho
  tốc độ bạn chọn. Tốc độ mặc định là *nhanh*, được relay định giá khoảng 1,8 × phí cơ
  sở cộng hai lần phí ưu tiên.
- **Phí = 3 × lượng gas dự trù × giá gas**, tối thiểu khoảng 0,01 USD. Trên Tempo, hệ
  số là 2 và phí được trả bằng pathUSD.

Vì lượng dự trù được độn cao hơn hẳn mức giao dịch sẽ dùng, và giá đã chừa sẵn biên độ,
**phí thường gấp mười lần chi phí thực của giao dịch trên chuỗi trở lên**, và còn cao
hơn ở giao dịch đầu tiên trên một mạng. Relay trả chi phí thực và giữ phần còn lại;
không có khoản nào được hoàn lại. Trên các mạng rẻ, đây chỉ là vài xu; trên mainnet
Ethereum, nó có thể là một khoản đáng kể. Số tiền chính xác nằm trên màn hình xác nhận
trước khi bạn ký.

<Callout type="info" title="Thấy bao nhiêu, trả bấy nhiêu">
Số tiền phí và địa chỉ nhận phí là một phần của thao tác bạn ký. Relay mà đổi một trong
hai thứ đó sẽ làm chữ ký của bạn mất hiệu lực, nên bạn trả đúng số tiền đã hiện — không
hơn, kể cả khi gas tăng trước lúc giao dịch được đưa vào khối. Báo giá gas nào của relay
cao hơn ba lần mức ví tự đọc được sẽ bị từ chối.
</Callout>

### Bạn có thể trả phí bằng gì

- **Coin gốc** của mạng, lúc nào cũng được.
- Một **stablecoin USD** trong danh sách của relay cho mạng đó, khi relay định giá được
  coin gốc. Những stablecoin bạn không có chút nào sẽ bị ẩn đi.
- Trên **Tempo**, vốn không có coin gốc, chỉ dùng được **pathUSD**.

Bạn chọn token trả phí, và tốc độ (*chậm*, *tiêu chuẩn* hoặc *nhanh*), trên màn hình
xác nhận và trong phần Cài đặt.

### Giao dịch đầu tiên của bạn trên một mạng

Bạn có thể nhận tiền trên bất kỳ mạng nào trước khi ví của bạn tồn tại ở đó. Lần đầu
bạn gửi đi từ một mạng, giao dịch đó đồng thời triển khai hợp đồng ví của bạn (và một
hợp đồng ký nhỏ cho mỗi khóa thêm vào). Gas triển khai đã được tính vào phí của giao
dịch đó, nên lần gửi đầu tiên trên mỗi mạng tốn hơn những lần sau.

Khi bạn gửi **tối đa** số coin gốc, Vela giữ lại đủ để trả phí.

## Ai vận hành relay — và ai nhận phí

Mặc định, mọi mạng đều dùng **relay của Vela**, và phí thuộc về Vela. Bạn có thể trỏ ví
sang một relay khác trong **Cài đặt → Nâng cao → Điểm cuối dịch vụ**; một địa chỉ phục
vụ tất cả các mạng tích hợp sẵn, còn một mạng tùy chỉnh giữ nguyên địa chỉ relay đã
dùng khi thêm nó. Relay đó phải là
[vela-relay](https://github.com/mondaylabsltd/vela-relay) — của Vela hoặc do bạn tự
chạy — vì ví lấy báo giá phí qua một phương thức riêng của Vela mà các bundler thông
dụng như Pimlico hay Alchemy không hỗ trợ. Ai vận hành relay bạn dùng thì người đó nhận
phí; [hướng dẫn tự triển khai](/vi/docs/self-hosting#relay) giải thích cách tự chạy
một relay.

Relay nhận một thao tác đã được ký sẵn. Nó không thể đổi người nhận, số tiền, mức phí
hay bất cứ thứ gì khác. Nó có thể trì hoãn hoặc từ chối, và nó chọn thời điểm giao dịch
lên chuỗi — nên với một lệnh hoán đổi, về lý thuyết nó có thể giao dịch chen trước bạn
trong phạm vi trượt giá của bạn.

### Khi relay hết gas

Relay trả gas từ **ngân quỹ** của chính nó trên từng mạng. Nếu ngân quỹ đó cạn, màn hình
gửi sẽ báo cho bạn trước khi bạn ký:

- Trên một mạng do relay của Vela phục vụ, bên vận hành relay (Vela) cần nạp thêm; bạn có
  thể báo lỗi này. Nếu không đợi được, bạn có thể **tùy ý** tự gửi một ít coin gốc vào
  ngân quỹ. Khoản đóng góp đó **không được hoàn lại** và **không** dùng để trả cho giao
  dịch của chính bạn.
- Trên một mạng tùy chỉnh, việc nạp tiền cho relay là việc của người vận hành nó — có
  thể chính là bạn.

Không có tài khoản gas riêng cho từng ví, cũng không có khoản đặt cọc kích hoạt nào: một
phiên bản trước đây của Vela từng có cơ chế đó, và giờ nó không còn nữa.

## Vela đọc dữ liệu từng mạng thế nào

Vela đọc số dư và mô phỏng giao dịch qua một **nhóm điểm cuối RPC** cho mỗi mạng — các
điểm cuối tích hợp sẵn, các điểm cuối công khai dự phòng, và mọi khóa nhà cung cấp hay
điểm cuối bạn thêm vào — và chuyển sang điểm cuối kế tiếp khi một điểm cuối chậm hoặc
ngừng hoạt động. Bạn có thể đặt điểm cuối riêng cho từng mạng trong **Cài đặt → Mạng
lưới**. (Ứng dụng Android hiện chỉ dùng một điểm cuối cho mỗi mạng, không có chuyển dự
phòng, và ứng dụng iPhone chưa cho phép thay đổi.)

Tiếp theo: [passkey hoạt động thế nào](/vi/docs/passkeys).
