---
title: Tự triển khai trang ký
description: "Trang web và tiện ích Chrome không phụ thuộc thư viện nào, tự giải mã giao dịch và ký bằng passkey của bạn — cách chạy bản sao của riêng bạn, và bản sao nào ký được cho ví của bạn."
source: c81389ef7aa2
---

# Tự triển khai trang ký

Vela giải mã mọi giao dịch trước khi bạn duyệt, và việc giải mã đó được làm một cách trung
thực — nhưng nó do chính ứng dụng đã dựng giao dịch thực hiện. Nếu ứng dụng, hoặc con đường
nó đến tay bạn, bị can thiệp, nó có thể cho bạn xem một thứ và ký một thứ khác. Đó chính xác
là điều đã xảy ra với [Bybit](/vi/docs/bybit-attack).

Trang ký tồn tại để tách việc đó làm hai: giao dịch đến từ một nơi, còn việc kiểm tra và ký
diễn ra ở một nơi khác do bạn kiểm soát.


Khi nhận một yêu cầu ký, nó không tin bản tóm tắt đi kèm. Nó tự giải mã calldata thô, tự tính
digest của riêng nó, cho bạn thấy chữ ký thực sự sẽ cho phép điều gì, rồi mới hỏi passkey của
bạn.

Vì không có bước build, các tệp bạn đọc chính là các tệp đang chạy. Bạn có thể so sánh thư mục
với kho mã và biết chính xác mình đang phục vụ cái gì.

**Dưới dạng một trang trên tên miền của bạn, hoặc trên localhost.** Khi được phục vụ qua HTTPS
(hoặc từ localhost), bên phụ thuộc của trang là chính tên máy chủ của nó — nên nó ký được bằng
các khóa được đăng ký dưới _tên máy chủ đó_, không phải các khóa được đăng ký dưới
`getvela.app`. Điều đó khiến đây là cách phù hợp để thử toàn bộ quy trình từ đầu đến cuối, để
chạy luồng máy tính, và để ký cho một ví có khóa được tạo trên tên miền của riêng bạn. Đây
không phải là cách ký cho một ví `getvela.app` đã có.

```sh
cd app-web/trusted-signer
python3 -m http.server 8080   # → http://localhost:8080
```

Mọi đường dẫn trong ứng dụng đều là đường dẫn tương đối, nên một thư mục con trên một máy chủ
có sẵn cũng chạy được, và mở thẳng `index.html` từ ổ đĩa (`file://`) thì xem thử được — khi
không có origin, sẽ không có bên phụ thuộc và không ký được gì.

## Nó làm gì trước khi ký

- **Nó tự giải mã giao dịch.** Lệnh gọi làm gì, cho ai, bao nhiêu, đọc ra từ calldata — kể cả
  những lệnh gọi lồng bên trong một giao dịch gộp.
- **Nó chỉ ký digest do chính nó tính.** Các digest EIP-191, EIP-712, SafeOp và SafeMessage
  được tính ngay trong trang và đối chiếu chéo với `vela-core`, chính đoạn mã mà ví dùng. Một
  digest nó không tính được thì là từ chối, không phải một chữ ký.
- **Nó kiểm tra giao dịch đúng là giao dịch đã được yêu cầu.** Lệnh gọi mà trang web yêu cầu
  phải thực sự nằm bên trong thao tác đang được ký.
- **Nó nói rõ khi một lệnh cấp quyền là không giới hạn.** Nó không thể thay đổi số lượng —
  nó ký đúng những byte đã đến hoặc không ký gì cả — nên một lệnh cấp quyền hay permit không
  giới hạn (từ 2^128 trở lên trên trang này) được hiện màu đỏ kèm lý do đó và có thể được ký
  nguyên trạng; hạn mức trên chuỗi được chọn trên màn hình cấp quyền của chính ví, trước khi
  yêu cầu đến được đây. Lệnh cấp quyền cho cả một bộ sưu tập NFT thì bị từ chối.
- **Nó nói rõ khi không đọc được thứ gì đó,** thay vì hiện một bản tóm tắt thân thiện mà nó
  không thể đứng ra bảo đảm.
- **Nó hiện địa chỉ và identicon của tài khoản,** và không hiện tên người nhận do bên xin chữ
  ký cung cấp. Bất cứ thứ gì bên yêu cầu kiểm soát đều bị bỏ đi hoặc được ghi rõ là của họ.

## Những gì nó cố ý không có

- **Không có trình chỉnh sửa.** Yêu cầu đã cố định khi đến nơi: bạn ký hoặc không ký. Một bộ
  chọn phí hay một trình chỉnh sửa hạn mức sẽ viết lại calldata, chính là căn bệnh mà trang này
  sinh ra để ngăn.
- **Không tạo khóa.** Trang ký không thể tạo passkey. Tạo một passkey mới đồng nghĩa với tạo
  một tài khoản khác.
- **Không lấy dữ liệu từ mạng.** Không thứ gì nó hiển thị hay ký là được tải về. Thứ duy nhất
  nó tải là logo token, dưới dạng hình ảnh, từ máy chủ dữ liệu chuỗi của Vela; nếu tải lỗi, một
  chữ cái sẽ thế chỗ.

## Một yêu cầu đến được nó bằng cách nào

| Bên yêu cầu                                  | Kênh                                                                       |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| Một trang trong cùng trình duyệt             | `postMessage`                                                              |
| Một trang trong cùng trình duyệt, gửi tới tiện ích | Cổng kết nối của tiện ích (extension port)                           |
| Một ứng dụng máy tính trên cùng máy          | Phân đoạn URL + callback qua loopback (có bản demo trong `samples/`; ứng dụng máy tính Vela chưa dùng) |
| Một điện thoại hoặc máy tính khác            | Bluetooth LE (giao thức đã cài đặt; phần sóng radio chưa được thử trên phần cứng thật) |

Định dạng truyền tin, cách tính digest, và một bảng cho biết từng mục trên màn hình đến từ đâu
đều nằm trong `PROTOCOL.md` cạnh mã nguồn.

## Nó nằm ở đâu trong bức tranh chung

Khi các ứng dụng đã chuyển được yêu cầu sang cho nó, cách dùng dự kiến rất đơn giản: kể từ ngày
tài khoản giữ số tiền mà bạn không muốn mất, mọi chữ ký đều đi qua một trang có mã do chính bạn
nạp. Không chỉ với số tiền lớn — một lệnh cấp quyền nhỏ cũng có thể trao đủ quyền để rút sạch
một tài khoản. Cho đến lúc đó, trang này là cách để đọc và thử nghiệm chính xác cách "ý kiến thứ
hai" ấy sẽ hoạt động.
