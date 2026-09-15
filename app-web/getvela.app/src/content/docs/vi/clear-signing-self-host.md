---
title: Tự chạy trang ký
description: Trang web và tiện ích Chrome không phụ thuộc thư viện nào, tự giải mã giao dịch và ký bằng passkey của bạn — cách chạy bản sao của riêng bạn, và bản nào ký được cho ví của bạn.
---

# Tự chạy trang ký

Vela giải mã mọi giao dịch trước khi bạn duyệt, và việc giải mã đó là việc làm tử tế —
nhưng nó do chính ứng dụng đã dựng nên giao dịch thực hiện. Nếu ứng dụng, hoặc đường
nó đến tay bạn, bị can thiệp thì nó có thể cho bạn xem một đằng và ký một nẻo. Đó đúng
là chuyện đã xảy ra với [Bybit](/vi/docs/bybit-attack).

Trang ký tồn tại để tách đôi việc đó: giao dịch đến từ một nơi, còn việc kiểm tra và
chữ ký diễn ra ở nơi bạn kiểm soát.

## Nó là gì

Một thư mục duy nhất — `app-web/clearsigning` trong kho mã — vừa là một trang web vừa
là một tiện ích Chrome. HTML, CSS và JavaScript thuần: không framework, không bundler,
không bước build, không phụ thuộc, và không tự phát yêu cầu mạng nào.

Nhận một yêu cầu ký, nó không tin bản tóm tắt đi kèm. Nó tự giải mã calldata thô, tự
tính digest của mình, cho bạn xem chữ ký sẽ thật sự cho phép điều gì, rồi mới hỏi
passkey của bạn.

Vì không có bước build, những tệp bạn đọc chính là những tệp đang chạy. Bạn có thể so
thư mục đó với kho mã và biết chắc mình đang phục vụ cái gì.

## Bản nào ký được cho ví của bạn

Passkey bị ràng buộc vào tên miền nơi nó được tạo ra. Khóa Vela của bạn đăng ký dưới
`getvela.app`, và trình duyệt chỉ đưa chúng cho trang nào có bên tin cậy là
`getvela.app`. Đúng một quy tắc ấy quyết định cách chạy bản sao nào là hữu ích cho
bạn.

**Dạng tiện ích Chrome — đây là cách dùng với chiếc ví hiện có của bạn.** Bên tin cậy
của tiện ích luôn là `getvela.app` bất kể thư mục đến từ đâu, nên khóa hiện tại của bạn
ký được trong đó, còn mã chạy là thư mục bạn đã nạp và đã xem.

1. Mở `chrome://extensions` và bật **Chế độ nhà phát triển**.
2. **Tải tiện ích đã giải nén**, rồi chọn thư mục `app-web/clearsigning`.
3. Biểu tượng trên thanh công cụ mở trang trong một tab.

**Dạng một trang trên tên miền của bạn, hoặc trên localhost.** Khi phục vụ qua
HTTP(S), bên tin cậy của trang chính là tên máy chủ của nó — nên nó ký được bằng khóa
đăng ký dưới _tên miền đó_, không phải khóa đăng ký dưới `getvela.app`. Đó là cách
đúng để thử trọn vẹn toàn bộ nghi thức, để chạy luồng máy bàn, và để ký cho một chiếc
ví mà khóa được tạo trên chính tên miền của bạn. Nó không phải cách ký cho một ví
`getvela.app` đã có.

```sh
cd app-web/clearsigning
python3 -m http.server 8080   # → http://localhost:8080
```

Mọi đường dẫn trong ứng dụng đều là tương đối, nên một thư mục con trên máy chủ sẵn có
cũng chạy được; mở thẳng `index.html` từ đĩa (`file://`) thì đủ để xem cho biết — không
có origin thì không có bên tin cậy, và không ký được gì.

## Nó làm gì trước khi ký

- **Nó tự giải mã giao dịch.** Lệnh gọi làm gì, cho ai, bao nhiêu — đọc từ calldata,
  kể cả những lệnh gọi lồng bên trong một lô.
- **Nó chỉ ký cái digest do chính nó tính.** Các digest EIP-191, EIP-712, SafeOp và
  SafeMessage được tính ngay trong trang và đối chiếu chéo với `vela-core`, đúng đoạn
  mã mà ví đang dùng. Một digest nó không tính được là một lời từ chối, không phải một
  chữ ký.
- **Nó kiểm tra giao dịch đúng là giao dịch đã được yêu cầu.** Lệnh gọi mà trang web
  hỏi phải thật sự nằm trong thao tác đang được ký.
- **Nó từ chối một lệnh duyệt không giới hạn.** Không phải cảnh báo — là từ chối, kèm
  chỉ dẫn nên làm gì thay thế.
- **Nó nói ra khi có thứ nó không đọc được,** thay vì đưa ra một bản tóm tắt thân thiện
  mà nó không đứng sau được.
- **Nó hiện địa chỉ và identicon của tài khoản,** và không hiện tên người nhận do bên
  yêu cầu chữ ký cung cấp. Mọi thứ bên yêu cầu kiểm soát đều bị bỏ đi hoặc gắn nhãn là
  của họ.

## Những thứ nó cố tình không có

- **Không có ô chỉnh sửa.** Yêu cầu được cố định ngay khi đến: bạn ký hoặc không ký.
  Một bộ chọn phí hay một ô sửa hạn mức sẽ viết lại calldata, mà đó đúng là căn bệnh
  trang này sinh ra để ngăn.
- **Không tạo khóa.** Trang ký không tạo được passkey. Tạo một cái mới tức là tạo một
  tài khoản khác.
- **Không có yêu cầu mạng.** Không có gì để tải thì không có gì để chặn giữa đường.

## Một yêu cầu đến với nó bằng cách nào

| Bên yêu cầu | Kênh |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| Một trang trong cùng trình duyệt | `postMessage` |
| Một trang trong cùng trình duyệt, gửi tới tiện ích | Cổng của tiện ích |
| Một ứng dụng máy bàn trên cùng máy | Fragment URL + callback loopback |
| Một điện thoại hoặc máy tính khác | Bluetooth LE (đã cài đặt giao thức; phần sóng chưa thử trên phần cứng thật) |

Định dạng truyền, các digest và bảng liệt kê từng thứ trên màn hình đến từ đâu nằm
trong `PROTOCOL.md` ngay cạnh mã nguồn.

## Khi nào nên dùng

Kể từ ngày tài khoản giữ số tiền mà mất đi bạn sẽ tiếc — và từ đó trở đi, cho mọi chữ
ký. Không chỉ cho số lớn: một lệnh duyệt nhỏ cũng có thể trao đủ quyền để vét sạch tài
khoản. Một thói quen ký chỉ dành cho dịp đặc biệt thì đúng ngày cần đến lại không có
sẵn.
