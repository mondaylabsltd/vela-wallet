---
title: Gửi & nhận
description: Cách nhận và gửi token trong Vela — một địa chỉ cho mọi mạng, giao dịch được ký minh bạch, và trừu tượng hóa tài khoản thực sự chuyển tiền của bạn ra sao.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Gửi & nhận

## Nhận

1. Mở ví và chạm **Nhận**.
2. Chia sẻ địa chỉ của bạn — sao chép, hoặc để người gửi quét mã QR.
3. Khi giao dịch được xác nhận trên chuỗi, số dư xuất hiện trong ví bạn.

Hai điều đáng biết:

- Địa chỉ của bạn **giống nhau trên mọi mạng được hỗ trợ**, nên bạn chỉ chia sẻ một
  địa chỉ ở khắp nơi — chỉ cần chắc rằng người gửi dùng đúng mạng.
- Bạn **nhận được tiền trước khi ví được triển khai**. Tài khoản Vela là tài khoản
  thông minh phản thực, nên tiền có thể tới địa chỉ của bạn trước khi hợp đồng tồn tại
  trên một chuỗi; nó tự triển khai trong lần gửi đầu tiên của bạn ở đó.

## Gửi

1. Chạm **Gửi** và chọn **token**.
2. Nhập **số tiền** (bạn có thể đổi qua lại giữa token và đơn vị tiền hiển thị) và
   **người nhận**. Vela phân giải những người nhận đã biết thành tên khi có thể — một
   tài khoản Vela, một tên ENS, một Basename, v.v.
3. **Xem lại và xác nhận.** Vela hiện giao dịch, rồi hỏi passkey của bạn (Face ID /
   Touch ID / vân tay).

### Điều gì xảy ra khi bạn xác nhận

Vela không chỉ "phát" một giao dịch. Bên dưới:

1. Nó dựng một **UserOperation** ERC-4337 cho tài khoản Safe của bạn.
2. Sau bước sinh trắc học, thiết bị của bạn ký nó bằng một xác nhận **WebAuthn
   (P-256)**.
3. Giao dịch đã ký đi tới **relay**, relay gửi nó tới EntryPoint; Safe của bạn xác
   minh chữ ký P-256 **trên chuỗi** rồi thực thi.

<Callout type="info" title="Relay không thể sửa giao dịch của bạn">
Relay nhận một UserOperation <strong>đã được ký</strong>. Nó có thể trì hoãn hoặc từ
chối chuyển tiếp, nhưng không thể đổi người nhận, số tiền hay bất kỳ trường nào — mọi
thay đổi đều làm chữ ký của bạn mất hiệu lực. Nó là người giúp việc cho tính sẵn sàng,
không phải bên giữ tiền, và vì là mã nguồn mở nên bạn tự chạy được.
</Callout>

### Ký minh bạch — không duyệt mù

Trước khi bạn ký, Vela giải mã giao dịch bằng các mô tả **ERC-7730** và hiện **ý
định** (Gửi, Duyệt, Hoán đổi…), **số tiền và địa chỉ**, cùng một chỉ báo rủi ro —
không phải một đống hex khó hiểu. Khi không giải mã trọn vẹn được một lệnh gọi, nó
hiện **cảnh báo ký mù** rõ ràng thay vì giả vờ đã hiểu. Một lệnh duyệt chi không giới
hạn không chỉ bị đánh dấu — Vela viết lại nó thành một số hữu hạn và từ chối gửi bất
kỳ lệnh duyệt nào vẫn còn không giới hạn.

## Trước khi bấm gửi

- **Kiểm tra vài ký tự đầu và cuối của địa chỉ.** Mã độc tráo địa chỉ là chuyện có
  thật.
- **Xác nhận đúng mạng.** Gửi nhầm mạng là sai lầm đắt đỏ phổ biến nhất. Xem
  [mạng & phí](/vi/docs/networks-and-fees).
- **Với người nhận mới, hãy thử số nhỏ.** Một giao dịch thử tí xíu là khoản bảo hiểm
  rẻ.

Giao dịch không thể đảo ngược. Không có bộ phận hỗ trợ nào kéo lại được số tiền gửi
nhầm địa chỉ — đó là bản chất của tự quản.

## Đọc lịch sử của bạn

Số dư và lịch sử được đọc trực tiếp từ một nhóm điểm cuối RPC công khai có tự động
chuyển dự phòng. Nếu mạng chậm, lịch sử có thể mất một lúc — vòng quay nghĩa là "vẫn
đang tải", không phải "tiền đã bay".
