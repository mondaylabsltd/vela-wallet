---
title: Tạo ví của bạn
description: Tạo một chiếc ví Vela tự quản trong khoảng một phút bằng passkey — không cụm từ khôi phục. Ví của bạn là một tài khoản thông minh Safe với cùng một địa chỉ trên mọi mạng.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Tạo ví của bạn

Tạo ví mất khoảng một phút và một lần xác thực sinh trắc học. Mở ví web tại
[wallet.getvela.app](https://wallet.getvela.app/) rồi chọn **Tạo ví**.

## Các bước

1. **Đặt tên cho ví.** Chọn một cái tên để sau này bạn nhận ra tài khoản, kể cả khi
   đăng nhập trên thiết bị khác. Tên được lưu cạnh khóa công khai của bạn, nên hãy coi
   nó là công khai — đừng đặt gì riêng tư vào đó.
2. **Xác nhận vài điều cơ bản.** Một danh sách ngắn xác nhận bạn hiểu rằng Vela là ví
   tự quản và vẫn còn là phần mềm alpha, kèm liên kết tới
   [chính sách riêng tư](/privacy) và [điều khoản](/terms).
3. **Tạo passkey.** Khi được hỏi, hãy xác thực bằng **Face ID, Touch ID hoặc vân
   tay**. Thao tác này tạo ra một passkey WebAuthn (P-256) do thiết bị của bạn giữ và
   Vela không bao giờ nhìn thấy. Không có bước cụm từ khôi phục, vì không có cụm từ
   khôi phục nào cả.
4. **Xong.** Vela hiện địa chỉ ví và bạn đã vào. Bạn có thể kiểm tra lại rồi đăng nhập
   để vào ví của mình.

## Ví của bạn thực ra là gì

Đây là phần mà hầu hết ví không giải thích — và nó quan trọng với cách Vela hoạt động.

Ví Vela của bạn là một **tài khoản thông minh Safe** (một hợp đồng thông minh), không
phải "tài khoản do khóa ngoài sở hữu" thông thường. Passkey của bạn là chủ sở hữu tài
khoản; thiết lập ERC-4337 cho phép bạn vận hành nó chỉ bằng khuôn mặt hoặc vân tay.

<Callout type="info" title="Địa chỉ của bạn giống nhau trên mọi mạng">
Vela suy ra địa chỉ từ khóa công khai của passkey, nên nó y hệt nhau trên Ethereum,
Base, Arbitrum, Gnosis và mọi mạng được hỗ trợ khác. Bạn chỉ cần đưa một địa chỉ cho
tất cả.
</Callout>

Một hệ quả hữu ích: địa chỉ mang tính **phản thực**. Nó được tính trước khi có bất cứ
thứ gì được triển khai trên chuỗi, nên **bạn có thể nhận tiền vào đó trước khi hợp
đồng ví tồn tại**. Hợp đồng tự triển khai — trả bằng chính số dư của nó — trong lần
đầu bạn gửi giao dịch trên mạng đó.

## Vừa rồi đã xảy ra gì với khóa của bạn

- Thiết bị của bạn tạo ra một **cặp khóa passkey**.
- **Khóa riêng tư** nằm ở trình cung cấp passkey của hệ điều hành (Chuỗi khóa iCloud
  hoặc Trình quản lý mật khẩu Google), được lưu mã hóa đầu-cuối và đồng bộ giữa các
  thiết bị của bạn — không ứng dụng nào, kể cả Vela, nhìn thấy nó.
- **Khóa công khai và cái tên bạn chọn** được gửi lên Chỉ mục Passkey của Vela; chỉ
  mục này cũng ghi khóa vào một bản ghi công khai trên Gnosis Chain, để tài khoản của
  bạn có thể được tìm lại trên thiết bị mới. Xem
  [khôi phục & đăng nhập](/vi/docs/recovery).

## Bước tiếp theo

- [Nhận những token đầu tiên](/vi/docs/send-and-receive)
- [Hiểu về mạng và phí](/vi/docs/networks-and-fees)
- [Đọc xem passkey giữ an toàn bằng cách nào](/vi/docs/passkeys)
