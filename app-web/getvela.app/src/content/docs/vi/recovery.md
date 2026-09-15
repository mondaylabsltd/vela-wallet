---
title: Khôi phục & đăng nhập
description: Cách Vela cho phép bạn khôi phục ví trên thiết bị mới mà không cần cụm từ khôi phục — và những giới hạn thật thà của mô hình đó.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Khôi phục & đăng nhập

Phần khó nhất của một chiếc ví không có cụm từ khôi phục chính là khôi phục: không có
mười hai từ thì làm sao vào lại trên điện thoại mới? Vela xử lý chính xác như sau.

## Cách hoạt động

Khi bạn tạo ví, hai thứ được ghi lên **chỉ mục passkey** của Vela:

- **Khóa công khai** của passkey (không bao giờ là khóa riêng tư).
- **Cái tên** bạn đặt cho ví.

Khóa công khai được lưu trên blockchain Gnosis qua một hợp đồng thông minh, nên ai
cũng đọc được và nó không phụ thuộc vào việc máy chủ của Vela còn sống hay không.

Trong khi đó, khóa **riêng tư** của bạn là một passkey do chuỗi khóa nền tảng đồng bộ
— **Chuỗi khóa iCloud** trên thiết bị Apple, **Trình quản lý mật khẩu Google** trên
Android.

Để đăng nhập trên thiết bị mới:

1. Đăng nhập cùng tài khoản iCloud hoặc Google, có bật đồng bộ chuỗi khóa.
2. Mở Vela và chọn đăng nhập.
3. Xác thực bằng passkey. Nền tảng đưa ra passkey đã đồng bộ; chỉ mục đưa ra tài khoản
   khớp với nó. Ví của bạn đã trở lại.

Chỉ mục là một bộ nhớ đệm, không phải điểm hỏng duy nhất. Nếu nó không truy cập được
và tài khoản của bạn không có trong bộ nhớ cục bộ, Vela có thể dựng lại khóa công khai
ngay trên thiết bị từ hai chữ ký passkey rồi suy ra lại địa chỉ ví — không cần máy chủ
nào.

<Callout type="info" title="Vì sao lại tách làm hai">
Khóa công khai trong chỉ mục trên chuỗi cho phép bất kỳ ai (kể cả một bản cài mới
tinh) tìm ra tài khoản của bạn. Khóa riêng tư, do chuỗi khóa nền tảng bạn tin cậy đồng
bộ, mới là thứ thật sự cho phép giao dịch. Mọi thứ trong chỉ mục đều là dữ liệu công
khai; không thứ gì trong đó chuyển được tiền của bạn — chỉ chữ ký từ passkey của bạn
mới làm được.
</Callout>

## Những giới hạn thật thà

Tự quản nghĩa là trách nhiệm cũng thật. Đây là những điều cần hiểu.

<Callout type="warning" title="Khôi phục của bạn phụ thuộc vào chuỗi khóa nền tảng">
Việc đăng nhập xuyên thiết bị của Vela dựa vào passkey được đồng bộ qua Chuỗi khóa
iCloud hoặc Trình quản lý mật khẩu Google. Hãy giữ tài khoản đó an toàn và cập nhật
các phương án khôi phục của nó. Nếu bạn mất quyền truy cập <strong>cả</strong> thiết
bị <strong>lẫn</strong> chuỗi khóa của tài khoản nền tảng, Vela không thể tạo lại khóa
riêng tư cho bạn — theo thiết kế, chúng tôi chưa bao giờ có nó.
</Callout>

Lời khuyên thực tế:

- **Bật đồng bộ chuỗi khóa.** Đó là thứ mang passkey của bạn đi giữa các thiết bị.
- **Bảo vệ tài khoản Apple / Google** bằng mật khẩu mạnh và các cách khôi phục riêng
  của nó. Tài khoản đó giờ là một phần an toàn của ví bạn.
- **Đăng nhập trên nhiều hơn một thiết bị** nếu có thể, để mất một chiếc điện thoại
  chỉ là phiền toái chứ không phải khủng hoảng.

## Vela làm được gì và không làm được gì

- **Làm được:** giúp bạn tìm lại tài khoản qua chỉ mục công khai.
- **Không làm được:** chuyển tiền của bạn, đóng băng ví, hay khôi phục một khóa riêng
  tư. Vela chưa bao giờ giữ nó. Đó chính là toàn bộ ý nghĩa của tự quản — và cũng là
  cái giá bạn đánh đổi cho nó.
