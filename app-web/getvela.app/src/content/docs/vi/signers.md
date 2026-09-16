---
title: Khóa ký & khóa bảo mật
description: Một ví Vela có thể có tối đa bảy khóa ký — passkey, một thiết bị ở gần, hoặc khóa bảo mật kiểu YubiKey — và bất kỳ khóa nào cũng ký được một mình. Chúng được chọn khi tạo ví, và trang này giải thích vì sao đó không phải một giới hạn mà chúng tôi quên gỡ.
---

# Khóa ký & khóa bảo mật

Ví Vela là một Safe, và Safe thì có chủ sở hữu. Ví của bạn có thể có **tối đa bảy**,
với ngưỡng là **một**: bất kỳ khóa ký đơn lẻ nào cũng tự mình duyệt được một giao
dịch. Người ta viết là `1-of-n`.

## Những gì có thể làm khóa ký

Ba loại, và bạn trộn chúng thoải mái:

| Cách | Là gì | Ví dụ điển hình |
| --- | --- | --- |
| **Nền tảng** | Bộ xác thực tích hợp trong thiết bị bạn đang dùng | Face ID / Touch ID trên chiếc điện thoại hay laptop này, đồng bộ bằng Chuỗi khóa iCloud hoặc Trình quản lý mật khẩu Google |
| **Thiết bị ở gần** | Một thiết bị khác mà bạn chạm tới bằng cách quét mã | Điện thoại ký thay cho máy bàn, qua kênh truyền hybrid của WebAuthn |
| **Khóa bảo mật** | Một bộ xác thực rời cắm USB hoặc chạm NFC | YubiKey và các khóa FIDO2 khác |

Cả ba đều là chứng danh WebAuthn trên đường cong **P-256**. Với Safe thì chúng không
khác gì nhau: mỗi cái là một chủ sở hữu mà bộ xác minh WebAuthn trên chuỗi kiểm tra
chữ ký theo đúng một cách.

Khóa bảo mật có thể là khóa ký **đầu tiên** của bạn, không chỉ là bản dự phòng. Nếu
bạn muốn ví của mình không phụ thuộc chút nào vào tài khoản Apple hay Google, đó chính
là lựa chọn làm được điều đó — đăng ký một YubiKey ngay khi tạo ví và ký bằng nó.

## Vì sao phải chọn ngay khi tạo

Đây là phần khiến nhiều người bất ngờ, nên chúng tôi nói về cơ chế thay vì xin lỗi.

Địa chỉ ví của bạn được **suy ra** từ tập chủ sở hữu. Vela tính nó bằng `CREATE2` từ
dữ liệu thiết lập Safe — trong đó có khóa công khai của từng khóa ký — trước khi bất
cứ thứ gì được triển khai trên chuỗi. Chính điều đó cho phép bạn nhận tiền vào một địa
chỉ còn chưa tồn tại.

Hệ quả là số học, không phải chính sách: **một tập khóa khác là một địa chỉ khác**.
Thêm khóa ký thứ tám về sau sẽ không mở rộng ví của bạn; nó tính ra một chiếc ví mới,
ở một địa chỉ mới, không có đồng nào của bạn trong đó.

Nên câu hỏi "tôi thêm khóa sau được không?" có hai câu trả lời thật thà:

- **Trước khi nạp tiền**: được — địa chỉ chưa ràng buộc với gì cả, cứ tạo lại ví với
  đúng những khóa bạn muốn.
- **Sau khi nạp tiền**: địa chỉ là nơi tiền của bạn đang nằm. Đổi chủ sở hữu của một
  Safe đã triển khai là một thao tác Safe mà hiện Vela không mở ra. Hãy tính tập khóa
  ngay từ lúc tạo.

## Điều này thật sự bảo vệ bạn khỏi cái gì

**Mất thiết bị.** Có nhiều hơn một khóa ký thì mất điện thoại chỉ là phiền toái: khóa
khác ký thay. Chỉ có đúng một khóa ký và lại tắt đồng bộ của hệ điều hành thì mất điện
thoại là mất ví — đó là lý do câu "passkey của bạn tự động đồng bộ" là mô tả một tùy
chọn do bạn kiểm soát, không phải lời bảo đảm chúng tôi thay bạn đưa ra.

**Một tài khoản nền tảng bạn không còn tin.** Nếu passkey của bạn nằm trong Chuỗi khóa
iCloud hay Trình quản lý mật khẩu Google, người kiểm soát tài khoản đó có thể dùng
được nó. Khóa bảo mật thì nằm trong tay bạn và không đồng bộ đi đâu.

Và điều nó **không** bảo vệ bạn, vì `1-of-n` cắt về cả hai phía: thêm khóa thứ hai là
thêm một lối *vào*, không phải thêm một ổ khóa. Ai lấy được bất kỳ khóa ký nào của bạn
đều ký được một mình. Nhiều khóa hơn nghĩa là chống mất mát tốt hơn và bề mặt trộm cắp
rộng hơn; đó là đánh đổi, và quyền quyết định là của bạn.

## Khôi phục khác với thêm khóa

Đây là hai việc khác nhau và tài liệu giữ chúng tách bạch:

- [Khôi phục & đăng nhập](/vi/docs/recovery) — quay lại một chiếc ví đã có trên thiết
  bị mới, bằng một khóa bạn đã có.
- Trang này — quyết định từ đầu rằng những khóa nào sẽ tồn tại.
