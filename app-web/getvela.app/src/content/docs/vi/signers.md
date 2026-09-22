---
title: Khóa ký & khóa bảo mật
description: "Một ví Vela có thể có tối đa bảy khóa ký — passkey, một điện thoại ở gần, hoặc khóa bảo mật kiểu YubiKey — và bất kỳ khóa nào cũng ký được một mình. Chúng được chọn khi tạo ví; trang này giải thích vì sao, và làm gì khi một khóa có thể đã bị lộ."
source: 072891bd4768
---

# Khóa ký & khóa bảo mật

Một ví Vela là một Safe, và Safe thì có chủ sở hữu. Ví của bạn có thể có **tối đa
bảy**, với ngưỡng là **một**: bất kỳ khóa ký đơn lẻ nào cũng tự mình phê duyệt được
một giao dịch. Cách viết là `1-of-n`.

## Những gì có thể làm khóa ký

Ba loại, và bạn có thể kết hợp tùy ý:

| Cách | Là gì | Ví dụ điển hình |
| --- | --- | --- |
| **Nền tảng** | Trình xác thực tích hợp trong thiết bị bạn đang dùng | Face ID / Touch ID trên điện thoại hoặc laptop này, đồng bộ qua Chuỗi khóa iCloud hoặc Trình quản lý mật khẩu của Google |
| **Thiết bị ở gần** | Một thiết bị khác mà bạn kết nối bằng cách quét mã | Điện thoại của bạn ký thay cho máy tính, qua kênh truyền hybrid của WebAuthn |
| **Khóa bảo mật** | Một trình xác thực rời, qua USB hoặc NFC | YubiKey và các khóa FIDO2 khác |

Cả ba đều là thông tin xác thực WebAuthn trên đường cong **P-256**. Với Safe, chúng
không khác gì nhau: mỗi loại là một chủ sở hữu mà mô-đun passkey của Safe kiểm tra chữ
ký trên chuỗi theo cùng một cách. (Khóa đầu tiên được bộ ký dùng chung của Safe xác
minh; mỗi khóa thêm vào có một hợp đồng ký nhỏ của riêng nó, do factory của Safe tạo ra
vào lần đầu ví được triển khai trên một chuỗi.)

Mỗi ứng dụng dùng được những loại nào:

| Ứng dụng | Thiết bị này | Điện thoại ở gần (QR) | Khóa bảo mật |
| --- | --- | --- | --- |
| Ví web, tiện ích trình duyệt | Có | Có | USB hoặc NFC, qua trình duyệt |
| Máy tính (macOS, Windows, Linux) | macOS và Windows | Có | USB |
| Android | Có (cần dịch vụ Google Play) | Có | USB |
| iOS | Có | Có | YubiKey cổng USB-C hoặc Lightning, firmware 5.8 trở lên |

Tùy chọn "thiết bị này" của ứng dụng máy tính (Touch ID, Windows Hello) và việc hỗ trợ
Windows nói chung đều còn mới và được thử nghiệm ít hơn các cách khác; hiện tại, trên
máy tính, điện thoại hoặc khóa bảo mật là lựa chọn đáng tin cậy hơn.

Khóa bảo mật có thể là khóa ký **đầu tiên** của bạn, không chỉ là khóa dự phòng. Nếu
bạn muốn ví của mình không bao giờ phụ thuộc vào tài khoản Apple hay Google nào, hãy
tạo ví bằng **hai** khóa bảo mật và cất một chiếc ở nơi an toàn. (Không thể tạo một ví
mà khóa duy nhất của nó không đồng bộ đi đâu cả: ứng dụng sẽ yêu cầu thêm khóa thứ hai,
vì mất thiết bị đó là mất luôn ví.)

## Vì sao khóa được chọn ngay khi tạo ví

Đây là phần khiến nhiều người bất ngờ, nên chúng tôi giải thích cơ chế thay vì xin lỗi.

Địa chỉ ví của bạn được **suy ra** từ tập chủ sở hữu của nó. Vela tính địa chỉ bằng
`CREATE2` từ dữ liệu thiết lập Safe — vốn chứa khóa công khai của mọi khóa ký — trước khi
có bất cứ thứ gì được triển khai trên chuỗi. Chính điều đó cho phép bạn nhận tiền vào
một địa chỉ chưa tồn tại.

Với địa chỉ, hệ quả chỉ là số học: **tập khóa khác thì địa chỉ khác**. Thêm một khóa ký
về sau sẽ không mở rộng ví của bạn; nó sẽ tính ra một ví mới, ở một địa chỉ mới, trong
đó không có đồng nào của bạn.

Nên câu hỏi "sau này tôi có thêm khóa được không?" có hai câu trả lời thật thà:

- **Trước khi bạn nạp tiền**: được — địa chỉ chưa gắn với thứ gì, nên hãy tạo lại ví
  với những khóa bạn muốn.
- **Sau khi bạn nạp tiền**: địa chỉ đó là nơi tiền của bạn đang nằm. Bản thân Safe có
  thể đổi chủ sở hữu trên một chuỗi mà ví đã được triển khai — nhưng trên mọi chuỗi mà
  ví chưa được triển khai, cùng địa chỉ đó vẫn đại diện cho tập khóa ban đầu, nên tập
  chủ sở hữu sẽ lệch nhau giữa các chuỗi. Giữ chúng đồng bộ giữa các chuỗi là làm được —
  một số ví thông minh có làm — nhưng Vela chưa xây tính năng đó, nên không cho phép đổi
  chủ sở hữu. Hãy lên kế hoạch cho tập khóa ngay khi tạo ví.

## Điều này thực sự bảo vệ bạn khỏi gì

**Mất thiết bị.** Có nhiều hơn một khóa ký, mất điện thoại chỉ là chuyện phiền: một
khóa khác vẫn ký được. Chỉ có đúng một khóa ký mà đồng bộ của hệ điều hành lại tắt, thì
mất điện thoại là mất ví — đó là lý do câu "passkey của bạn tự động đồng bộ" là mô tả
một thiết lập do bạn kiểm soát, không phải một bảo đảm mà chúng tôi có thể thay bạn đưa
ra.

**Một tài khoản nền tảng bạn không còn tin.** Nếu passkey của bạn nằm trong Chuỗi khóa
iCloud hoặc Trình quản lý mật khẩu của Google, người kiểm soát tài khoản đó có khả năng
dùng được nó. Khóa bảo mật thì do chính bạn cầm và không đồng bộ đi đâu cả.

Và điều nó **không** bảo vệ bạn, vì `1-of-n` là con dao hai lưỡi: thêm khóa thứ hai là
thêm một lối *vào*, không phải thêm một ổ khóa. Bất kỳ ai lấy được bất kỳ khóa ký nào
của bạn cũng tự ký được một mình. Nhiều khóa hơn nghĩa là chống mất tốt hơn và dễ bị
trộm hơn; đó là sự đánh đổi, và người quyết định là bạn.

## Nếu một khóa có thể đã bị lộ

Không thể gỡ bỏ khóa. Nếu một trong các khóa của bạn có thể đã rơi vào tay người khác
— một chiếc điện thoại chưa khóa bị mất, một mã khóa bị ai đó nhìn thấy, một tài khoản
Apple hay Google bạn không còn kiểm soát — hãy **chuyển toàn bộ sang một ví mới** được
tạo bằng những khóa bạn tin tưởng. Địa chỉ cũ vẫn tiêu được bằng khóa đó trên mọi mạng,
kể cả những khoản tiền ai đó gửi vào sau này.

## Khôi phục, khác với thêm khóa

Đây là hai việc khác nhau và tài liệu tách riêng chúng:

- [Khôi phục & đăng nhập](/vi/docs/recovery) — quay lại một ví đã có trên thiết bị mới
  bằng một khóa bạn đang có.
- Trang này — quyết định ngay từ đầu xem sẽ có những khóa nào.
