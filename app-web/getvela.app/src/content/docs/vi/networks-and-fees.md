---
title: Mạng & phí
description: 12 mạng Vela hỗ trợ, cách phí gas hoạt động trong trừu tượng hóa tài khoản, ai vận hành relay và thu phí, khi nào bạn tự trả tiền kích hoạt tài khoản gas, và cách Vela chọn điểm cuối RPC.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Mạng & phí

## Các mạng được hỗ trợ

Vela có sẵn **12 mạng EVM**:

| Mạng | Token phí gốc |
| ----------- | ---------------- |
| Ethereum | ETH |
| BNB Chain | BNB |
| Polygon | POL |
| Arbitrum | ETH |
| Optimism | ETH |
| Base | ETH |
| Avalanche | AVAX |
| Gnosis | xDAI |
| Unichain | ETH |
| Tempo | USD |
| Monad | MON |
| World Chain | ETH |

Ví của bạn có **cùng một địa chỉ trên tất cả**, nên chỉ có một địa chỉ để chia sẻ ở
mọi nơi.

Bạn cũng có thể **thêm mạng tùy chọn** (Cài đặt → Mạng). Vì Vela là ví tài khoản
thông minh, một mạng phải có sẵn những hợp đồng Vela dựa vào — EntryPoint của
ERC-4337, các hợp đồng Safe, và precompile chữ ký **P-256 (RIP-7212)** dùng để xác
minh passkey của bạn trên chuỗi. Vela kiểm tra tự động trước khi cho bạn thêm mạng.

<Callout type="info" title="Vì sao Gnosis xuất hiện nhiều thế">
Ngoài việc là một trong 12 mạng, Gnosis Chain còn lưu <strong>Chỉ mục Passkey</strong>
của Vela — hợp đồng giữ khóa công khai và tên tài khoản của bạn để khôi phục xuyên
thiết bị. Việc đó tách biệt với chuyện bạn giao dịch trên mạng nào.
</Callout>

## Phí hoạt động thế nào (trừu tượng hóa tài khoản)

Vela dùng **trừu tượng hóa tài khoản ERC-4337**, nên giao dịch không do bạn phát trực
tiếp — nó là một **UserOperation** giao cho một **relay**, relay gửi lên chuỗi và được
hoàn lại tiền gas. (Đặc tả ERC-4337 gọi vai trò đó là *bundler*. Của Vela gọi là relay
vì nó làm nhiều hơn việc gộp lô: nó báo phí ngay trong kênh và chạy giao thức tài khoản
gas nói ở dưới, cả hai đều không thuộc chuẩn.) Từ đó suy ra vài điều:

- **Gas trả từ chính số dư ví của bạn** — mặc định bằng token gốc của mạng (ETH, BNB,
  xDAI…), hoặc bằng một stablecoin được hỗ trợ ở nơi relay có cung cấp; bạn chọn tài
  sản trả phí ngay trên màn hình xác nhận. Tempo không có đồng gốc nên gas ở đó luôn
  được thanh toán bằng stablecoin USD. Không có **paymaster** ERC-4337 nào tài trợ —
  hay chặn — từng giao dịch. (Vela có thể tài trợ bước _kích hoạt tài khoản gas_ một
  lần cho người dùng mới; đó là chuyện riêng, nói ở dưới.)
- **Relay báo giá gas** — nó là nguồn sự thật duy nhất, còn ví hiển thị đúng báo giá đó
  và ký đúng thứ nó hiển thị. Không có nút chọn tốc độ: mọi giao dịch đều gửi ở mức ưu
  tiên cao.
- Tổng phí là **chi phí mạng cộng phí dịch vụ của relay**, với một mức tối thiểu nhỏ
  cho những giao dịch rất rẻ. Báo giá của relay chính là giá — không có bảng phí riêng
  nào để tra. Một phần đi về các validator của chuỗi; phần còn lại trả cho relay, bên
  ứng trước tiền gas và vận hành hạ tầng.
- Màn hình xác nhận hiện **phí ước tính** theo tài sản trả phí và theo đơn vị tiền hiển
  thị của bạn trước khi bạn ký. Số tiền báo giá và người nhận nó là một phần của thứ
  bạn ký, nên relay được trả đúng bằng con số đã hiện — đổi số là chữ ký của bạn mất
  hiệu lực.

## Ai vận hành relay — và ai nhận phí

Mỗi mạng trỏ tới một relay. Mặc định đó là **relay của chính Vela**, và bạn có thể thay
điểm cuối ở _Cài đặt → Nâng cao → Điểm cuối dịch vụ_. Một điểm cuối áp dụng cho mọi
mạng có sẵn; một mạng tùy chọn thì giữ địa chỉ relay bạn đã nhập khi thêm nó.

Một lưu ý thật thà về tương thích: ứng dụng lấy báo giá phí qua một phương thức RPC
riêng của Vela (`vela_getInBandGasQuote`), và luồng gửi không chạy nếu thiếu nó. Vậy
nên điểm cuối bạn trỏ tới phải đang chạy
[vela-relay](https://github.com/mondaylabsltd/vela-relay) — bản của Vela hoặc bản bạn
tự dựng. Một bundler ERC-4337 thông thường như **Pimlico** hay **Alchemy** không cài
đặt phương thức đó, nên ở bản phát hành hiện tại nó sẽ không chạy trọn luồng.

Ai vận hành relay cho một mạng thì **thu phí của mạng đó** — phần chênh của relay trên
mỗi giao dịch và khoản đặt cọc kích hoạt tài khoản gas. Hãy chạy vela-relay của riêng
bạn và những khoản phí ấy nuôi hạ tầng của bạn thay vì của Vela; Vela không lấy phần
nào từ lưu lượng bạn dẫn đi nơi khác.

<Callout type="warning" title="Tài khoản gas là một phần của giao thức vela-relay">
Bước <strong>kích hoạt tài khoản gas</strong> nạp tiền cho một tài khoản relay dành
riêng cho ví của bạn trên mỗi mạng. Nếu bạn trỏ điểm cuối tới một vela-relay tự dựng,
khoản đặt cọc nạp cho tài khoản của relay của chính bạn, không phải của Vela.
</Callout>

### Kích hoạt tài khoản gas (Vela Relay)

Trên relay của Vela, giao dịch đầu tiên của bạn ở mỗi mạng sẽ **kích hoạt một tài
khoản gas riêng**. Ứng dụng trước hết xin ngân quỹ của relay chi trả giúp bạn — việc
này diễn ra lặng lẽ bên trong luồng gửi, và một chiếc ví được tài trợ sẽ không bao giờ
thấy màn hình nạp tiền. Chỉ khi việc tài trợ bị từ chối, ứng dụng mới hiện yêu cầu nạp:
bạn gửi một lượng nhỏ token gốc tới địa chỉ tài khoản gas mà nó hiển thị, và nó nói cho
bạn biết vì sao không được tài trợ.

**Bạn tự trả phí kích hoạt** mỗi khi không được tài trợ miễn phí, cụ thể là khi:

- **Ngân quỹ của Vela cho mạng đó cạn hoặc gần cạn** — quỹ miễn phí trên chuỗi đó tạm
  hết.
- **Bạn đã dùng hết hạn mức miễn phí** — việc tài trợ có giới hạn theo từng ví, quá vài
  lần đầu thì bạn tự trả.
- **Relay của Vela không tài trợ mạng đó** — ví dụ **mạng tùy chọn hoặc mạng thử nghiệm
  do bạn tự thêm**, Vela không giữ ngân quỹ cho chúng. (Hãy trỏ chúng sang relay của
  riêng bạn nếu muốn bỏ hẳn bước kích hoạt.)

Khoản đặt cọc kích hoạt **không hoàn lại** — đó là số dư khởi đầu của relay và tự bù
lại từ tiền hoàn gas theo thời gian, dù vẫn có thể cạn và cần **kích hoạt lại** về sau.
Địa chỉ relay cũng có thể đổi khi nâng cấp dịch vụ, và khi đó cần kích hoạt mới.

Phí trừ vào số dư của bạn ở **tài sản trả phí** bạn chọn — mặc định là token gốc. Nếu
một lệnh gửi bị chặn vì gas, nghĩa là số dư của bạn ở tài sản trả phí đó không đủ trả
phí; ở nơi relay có cung cấp gas bằng stablecoin, đổi tài sản trả phí trên màn hình xác
nhận có thể gỡ chặn.

Khi bạn gửi **toàn bộ** số token gốc, Vela tự động giữ lại đủ cho gas để giao dịch
không thất bại.

## Vela nói chuyện với từng mạng thế nào

Vela đọc số dư và gửi giao dịch qua một **nhóm điểm cuối RPC**, không phải một nhà cung
cấp duy nhất. Nó gom điểm cuối từ nhiều nguồn, chấm điểm theo độ trễ và độ tin cậy, và
**tự chuyển dự phòng** khi một cái chậm hoặc chết — tạm cho những điểm cuối tệ ra ngoài
— để một node chập chờn không bao giờ làm cả ứng dụng ngưng chạy.

Tiếp theo: [passkey hoạt động thế nào](/vi/docs/passkeys).
