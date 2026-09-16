---
title: Giới thiệu
description: Vela là gì, dành cho ai, và những ý tưởng đằng sau một chiếc ví thông minh tự quản không cần cụm từ khôi phục.
---

# Giới thiệu

Vela là **ví thông minh tự quản** cho các mạng EVM. Khóa là của bạn, nhưng không có
cụm từ khôi phục nào phải chép lại — bạn ký bằng passkey, bằng khuôn mặt hoặc vân
tay của mình.

Tài liệu này nói về cách bắt đầu, cách tạo ví, cách chuyển token và mô hình bảo mật
đằng sau tất cả.

## Bản ngắn

- **Tự quản.** Tiền của bạn do một khóa chỉ mình bạn dùng được kiểm soát. Vela (công
  ty) không thể chuyển, đóng băng hay lấy lại tiền của bạn.
- **Không có cụm từ khôi phục.** Khóa ký của bạn là một passkey nằm trong phần cứng
  bảo mật của thiết bị. Không có mười hai từ nào để mất hay bị lừa lấy.
- **Một tài khoản thông minh Safe.** Mỗi ví là một hợp đồng thông minh
  [Safe](https://github.com/safe-fndn/safe-smart-account) vận hành bằng trừu tượng
  hóa tài khoản ERC-4337 — chính điều đó cho phép bạn ký bằng passkey và đọc từng
  giao dịch trước khi duyệt.
- **12 mạng, một địa chỉ.** Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base,
  Avalanche, Gnosis, Unichain, Tempo, Monad và World Chain — cộng thêm mạng tùy chọn
  — tất cả cùng một địa chỉ.
- **Ký mà đọc được.** Giao dịch được giải mã thành ý định con người đọc được
  (ERC-7730) khi có mô tả; nếu không, Vela giải mã hết mức có thể và hiện cảnh báo.
  Những lệnh gọi nó không đọc được sẽ bị đánh dấu, không bị giấu đi.
- **Mã nguồn mở.** Ví và mọi dịch vụ của nó đều
  [công khai trên GitHub](https://github.com/mondaylabsltd/vela-wallet) để ai cũng
  kiểm tra được chúng thật sự làm gì.
- **Phần mềm alpha.** Vela chạy được và đang giữ tiền thật, nhưng chưa trải qua nhiều
  năm tôi luyện trong môi trường thật. Hãy bắt đầu với số nhỏ.
  [Bài viết về alpha](/blog/vela-is-in-alpha) giải thích điều đó nghĩa là gì.

## Dành cho ai

Vela dành cho những người muốn tự quản thật sự mà không phải ôm quả bom hẹn giờ mang
tên quản lý cụm từ khôi phục — và cho những người từng bỏng tay vì nó. Nếu bạn mở
khóa được điện thoại, bạn dùng được Vela.

## Đi tiếp

- [Cài đặt Vela](/vi/docs/install) — chạy ngay trong trình duyệt, không cần tải gì.
- [Tạo ví của bạn](/vi/docs/create-wallet) — chiếc ví đầu tiên trong khoảng một phút.
- [Passkey hoạt động thế nào](/vi/docs/passkeys) — mô hình bảo mật, nói cho dễ hiểu.
- [Whitepaper](/vi/docs/whitepaper) — toàn bộ kiến trúc và mô hình tin cậy.

Nếu bạn quan tâm cái *vì sao* hơn cái *thế nào*, [blog](/blog) kể lại quá trình
Vela được làm ra.
