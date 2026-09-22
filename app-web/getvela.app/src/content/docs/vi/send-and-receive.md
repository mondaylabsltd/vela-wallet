---
title: Gửi & nhận
description: "Cách nhận và gửi bằng Vela — một địa chỉ trên mọi mạng, gửi cho một hay nhiều người, tên người nhận lấy từ đâu, bạn xác nhận những gì, và relay chuyển tiền của bạn ra sao."
source: 9e280dfc853b
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Gửi & nhận

## Nhận

1. Mở ví và chạm **Nhận**.
2. Chia sẻ địa chỉ của bạn — sao chép hoặc cho xem mã QR. Ví web còn tạo được yêu cầu
   thanh toán kèm số tiền.
3. Khi giao dịch chuyển được xác nhận trên chuỗi, số tiền sẽ hiện trong số dư của bạn.

- Địa chỉ của bạn **giống nhau trên mọi mạng**, nên bạn chỉ cần đưa một địa chỉ — nhưng
  người gửi vẫn phải dùng một mạng mà Vela hỗ trợ, hoặc một mạng bạn đã thêm.
- Bạn có thể **nhận tiền trước khi ví được triển khai** trên một mạng. Ví tự triển khai
  vào lần đầu bạn gửi đi trên mạng đó.

## Gửi

1. Chạm **Gửi** và chọn **token**.
2. Nhập **số tiền** (theo token hoặc theo tiền tệ hiển thị của bạn) và **địa chỉ người
   nhận**, bằng cách dán, quét mã QR hoặc chọn từ danh bạ.
3. **Xem lại.** Vela cho thấy điều gì sẽ xảy ra, mức phí, và tên nó tìm được cho người
   nhận, nếu có.
4. **Xác nhận** bằng một trong các khóa của bạn — Face ID, vân tay, mã PIN, hoặc chạm
   và nhập PIN trên khóa bảo mật.

### Gửi cho nhiều người, hoặc gom về một chỗ

- **Chia** — gửi một token cho nhiều người trong một giao dịch. Bạn có thể dán danh
  sách hoặc nhập từ bảng tính, và nhập số tiền theo tiền tệ của bạn.
- **Gom** — gửi nhiều token về một địa chỉ trong một giao dịch.

Dù theo cách nào, bạn cũng chỉ ký một lần, và giao dịch chỉ trả một lần phí.

### Tên cho địa chỉ

Khi bạn nhập một địa chỉ, Vela tra tên cho nó: trước hết trong sổ đăng ký của chính
Vela (tên của một ví Vela khác), sau đó trong bản ghi ngược của `.bnb`, `.arb`, `.g`,
Basename và ENS, đọc trực tiếp từ từng chuỗi. Việc tra chỉ đi một chiều — nó đặt tên
cho địa chỉ bạn đã nhập. Gõ một cái tên như `alice.eth` sẽ không tra ra địa chỉ.
**Danh bạ** bạn đã lưu cũng hiện tên.

Tên lấy từ những bản ghi ngược đó chỉ được hiện nếu nó **phân giải xuôi trở lại đúng địa
chỉ ấy**. Ai cũng có thể đặt bản ghi ngược của mình thành bất kỳ chuỗi ký tự nào, nên
riêng bản ghi ấy không chứng minh được gì; ví hỏi dịch vụ tên xem cái tên đó trỏ tới địa
chỉ nào, và chỉ hiện tên khi hai bên khớp nhau. Nếu không kiểm tra được — điểm cuối
không trả lời, resolver lỗi — bạn sẽ thấy địa chỉ và không có tên, chứ không bao giờ
thấy một cái tên chưa được kiểm tra.

### Token trả phí và tốc độ

Màn hình xác nhận hiện mức phí theo token trả phí và theo tiền tệ của bạn. Bạn có thể
trả bằng coin gốc của mạng hoặc, ở nơi relay chấp nhận, bằng một stablecoin USD, và
chọn tốc độ (mặc định: nhanh). Khi bạn gửi **tối đa** số coin gốc, Vela giữ lại đủ để
trả phí. [Cách tính phí](/vi/docs/networks-and-fees).

### Điều gì xảy ra khi bạn xác nhận

1. Vela dựng một **UserOperation** ERC-4337 cho Safe của bạn, bao gồm cả khoản trả
   phí cho relay.
2. Sau khi xác minh bạn, khóa của bạn ký nó bằng một xác nhận **WebAuthn (P-256)**.
3. Thao tác đã ký được gửi tới **relay**, relay chuyển nó cho EntryPoint; Safe của bạn
   kiểm tra chữ ký P-256 trên chuỗi rồi thực thi.

<Callout type="info" title="Relay không thể sửa giao dịch của bạn">
Relay nhận một thao tác đã được ký sẵn. Nó không thể đổi người nhận, số tiền hay mức
phí — mọi thay đổi đều làm chữ ký của bạn mất hiệu lực. Nó có thể trì hoãn hoặc từ
chối, và nó quyết định khi nào giao dịch lên chuỗi. Relay là mã nguồn mở, và bạn có thể
[tự chạy relay của mình](/vi/docs/self-hosting#relay).
</Callout>

Trước khi bạn ký, Vela giải mã xem giao dịch làm gì và cảnh báo bạn về những phần nó
không giải mã được; xem [ký minh bạch](/vi/docs/clear-signing).

## Trước khi bấm gửi

- **Kiểm tra phần đầu và phần cuối của địa chỉ.** Phần mềm độc hại tráo địa chỉ là có
  thật, và những địa chỉ trông na ná được cài vào lịch sử giao dịch của bạn cũng vậy.
- **Xác nhận đúng mạng.** Gửi nhầm mạng là lỗi phổ biến và đắt giá.
- **Gửi thử số nhỏ cho người nhận mới.** Một giao dịch thử thật nhỏ là khoản bảo hiểm
  rẻ.

Giao dịch không thể đảo ngược. Không ai đòi lại được một lần gửi nhầm địa chỉ — đó là
bản chất của tự lưu ký.

## Hoạt động của bạn

Phần hoạt động gộp những gì bạn đã gửi từ thiết bị này với các giao dịch chuyển token
đọc từ log của từng chuỗi. Một giao dịch chuyển coin gốc thông thường tới bạn mà đi qua
một hợp đồng khác (ví dụ một số lệnh rút từ sàn) có thể không tạo log trên một số mạng,
nên nó có thể hiện trong số dư mà không xuất hiện trong phần hoạt động. Số dư được đọc
trực tiếp qua một nhóm điểm cuối RPC có tự động chuyển dự phòng; biểu tượng đang xoay
nghĩa là "vẫn đang tải", không phải "tiền đã mất".

Tiếp theo: [mạng & phí](/vi/docs/networks-and-fees).
