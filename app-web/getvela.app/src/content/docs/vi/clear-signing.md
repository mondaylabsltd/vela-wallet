---
title: Ký minh bạch
description: "Vela giải mã giao dịch thành ngôn ngữ dễ hiểu trước khi bạn duyệt — ý định, số tiền, địa chỉ và rủi ro — thay vì một chuỗi hex khó hiểu. Khi không giải mã được một lệnh gọi, nó cảnh báo bạn chứ không giả vờ đã hiểu."
source: 7232328b724e
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Ký minh bạch

Nhiều ví vẫn chỉ hiện dữ liệu thô với bất kỳ hợp đồng nào chúng không nhận ra, và "ký
mù" — duyệt những lệnh gọi mà bạn thực ra không đọc được — là một trong những con đường
khiến ví bị rút sạch. Câu trả lời của Vela là **ký minh bạch**: trước khi bạn ký, giao
dịch được giải mã thành thứ bạn hiểu được, trong phạm vi có thể.

## Bạn nhìn thấy gì

Thay vì calldata thô, Vela hiện:

- **Ý định** — giao dịch làm gì: *Gửi*, *Cấp quyền*, *Hoán đổi*, v.v.
- **Nội dung chính** — số tiền và các địa chỉ liên quan, với số token hiển thị theo đơn
  vị thật và người nhận được hiện bằng tên nếu có.
- **Chi tiết** — nonce, hạn chót và calldata thô, mở ra khi bạn cần chứ không dí vào
  mặt bạn.
- **Một chỉ báo rủi ro**, phân màu để các thao tác nguy hiểm nổi bật lên.

## Cách hoạt động (ERC-7730)

Vela giải mã cả **lệnh gọi hợp đồng** lẫn **dữ liệu có cấu trúc EIP-712** bằng các bộ
mô tả [ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry) — những
định nghĩa nhỏ, chia sẻ được, cho biết các hàm của một hợp đồng có ý nghĩa gì.

Vela tìm bộ mô tả theo thứ tự sau:

1. **Có sẵn trong ứng dụng** — bộ mô tả cho những hợp đồng được dùng rộng rãi: router
   của Uniswap, PancakeSwap và SushiSwap, WETH, pool Aave v3, 1inch, Lido và wstETH, và
   Seaport.
2. **Lấy từ máy chủ dữ liệu chuỗi của Vela**, nơi đăng lại registry ERC-7730 công khai.
3. **Các dạng tiêu chuẩn** — token ERC-20, NFT ERC-721 và ERC-1155, vault ERC-4626 và
   permit ERC-2612 — nên phần lớn thao tác hằng ngày vẫn giải mã được.

**Đã xác minh** chỉ dành cho nguồn thứ nhất. Một giao dịch chỉ được gắn nhãn đã xác minh
khi phần mô tả đến từ một bộ mô tả có sẵn trong chính ứng dụng bạn đang chạy — hoặc đến
từ máy chủ dữ liệu chuỗi và giống hệt bản có sẵn, điều này chứng minh không có gì bị thay
đổi trên đường truyền. Mọi thứ khác mà máy chủ gửi về vẫn được giải mã và vẫn được hiển
thị, kèm một dòng nói rằng nó đến từ dịch vụ bộ mô tả và không có gì xác thực nó. Dịch vụ
đó không được ký, nên nó chỉ đáng tin bằng chính người vận hành nó — một trong những lý do
bạn có thể [tự chạy máy chủ của mình](/vi/docs/self-hosting#chain-data).

Số lượng token được định dạng theo **số chữ số thập phân thật trên chuỗi** của token đó.
Nếu Vela không xác nhận được số chữ số thập phân của một token, nó hiện số lượng như thể
token có 18 chữ số và **đánh dấu là chưa xác minh**, để một con số sai không bao giờ
trông giống một con số đã được kiểm tra.

## Mức rủi ro

Mỗi giao dịch đã giải mã đều có một mức rủi ro để những mẫu nguy hiểm nổi bật lên:

- **Thận trọng** cho các lệnh cấp quyền và permit — bạn đang trao quyền chi tiêu.
- **Nguy hiểm** cho những gì thật sự rủi ro, như **cấp quyền token không giới hạn**.
- Rủi ro thấp hơn cho các thao tác thường ngày như staking hay gửi tiền vào.

<Callout type="warning" title="Lệnh cấp quyền “không giới hạn” trên chuỗi không thể gửi đi">
Một lệnh cấp quyền trên chuỗi với số lượng không giới hạn là một trong những cách phổ
biến nhất khiến tiền bị rút sạch về sau. Khi một dApp yêu cầu lệnh như vậy
(<code>approve</code>, <code>increaseAllowance</code>, hoặc <code>approve</code> của
Permit2) ở mức "không giới hạn" — từ 2^200 trở lên (2^152 với Permit2), tức là con số mà
dApp dùng cho "không giới hạn" — Vela sẽ không gửi nó đi cho đến khi bạn đổi thành một số
lượng cụ thể, bằng số dư của bạn, hoặc thu hồi; một lần kiểm tra cuối trước khi gửi đọc
thẳng calldata thô, nên nó hoạt động dù có bộ mô tả hay không. Những gì nó không chặn:
một <strong>lệnh cấp quyền lớn nhưng hữu hạn</strong> (kể cả khi vượt xa số dư của bạn),
<strong>permit dạng chữ ký</strong> (chữ ký EIP-2612 và Permit2), và
<code>setApprovalForAll</code> cho NFT — mỗi thứ đều hiện kèm cảnh báo thận trọng, và
quyết định là của bạn.
</Callout>

## Khi Vela không giải mã được một lệnh gọi

Khi không có bộ mô tả ERC-7730 nào nhưng hàm đó có trong một cơ sở dữ liệu selector công
khai, Vela giải mã lệnh gọi theo cách chung chung và gắn nhãn **giải mã tốt nhất có thể**
— đã giải mã, nhưng chưa xác minh — dưới một dải cảnh báo thận trọng. Nếu đến cả cách đó
cũng thất bại, hoặc Vela chỉ giải mã được một phần giao dịch, nó **không** giả vờ đã
hiểu.

<Callout type="danger" title="Cảnh báo ký mù rõ ràng">
Nếu một lệnh gọi không giải mã được, Vela hiện cảnh báo ký mù rõ ràng thay vì một bản tóm
tắt thân thiện giả tạo. Nếu nó chỉ đọc được một số trường, nó cho bạn biết phần hiển thị
chưa đầy đủ và giữ mức rủi ro ở mức cao. Bạn luôn biết Vela thực sự đọc được bao nhiêu
phần của thứ bạn đang ký.
</Callout>

## Vì sao điều này quan trọng

Tự lưu ký nghĩa là không ai đảo ngược được một giao dịch tồi giúp bạn. Tuyến phòng thủ
không phải là bộ phận hỗ trợ — mà là hiểu thứ bạn duyệt **trước khi** duyệt. Ký minh bạch
là cách Vela cố gắng cho bạn thấy điều đó, và nó có giới hạn: nó chỉ trung thực được đến
mức ứng dụng hiển thị nó trung thực, đó là lý do một
[lớp kiểm tra độc lập](/vi/docs/clear-signing-self-host) lại quan trọng. Xem
[whitepaper](/vi/docs/whitepaper) để biết nó nằm ở đâu trong mô hình bảo mật của Vela.
