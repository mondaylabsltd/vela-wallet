---
title: Hợp đồng tài khoản
description: "Ví Vela của bạn là một Safe v1.4.1 nguyên bản. Không hợp đồng nào trên đường đi tới tiền của bạn do Vela viết — đây là chính xác những hợp đồng đó, chúng mang lại cho bạn điều gì, và cái giá là gì."
source: 17fbc25a3149
---

# Hợp đồng tài khoản

Ví của bạn không phải là một cấu trúc dữ liệu riêng của ứng dụng nào. Nó là một tài
khoản thông minh **Safe v1.4.1** — hợp đồng mà nhiều ngân quỹ lớn trên chuỗi đang dùng —
được triển khai y như Safe công bố, không sửa đổi gì.

## Trên đường đi không có hợp đồng nào của chúng tôi

Mọi hợp đồng có thể chạm tới tiền của bạn đều do Safe hoặc các tác giả ERC-4337 viết:

| Hợp đồng | Vai trò trong ví của bạn | Tác giả |
| --- | --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) (SafeL2, qua một proxy) | Chính tài khoản; chủ sở hữu, ngưỡng, thực thi | Safe |
| [Safe 4337 Module v0.3.0](https://github.com/safe-global/safe-modules/tree/4337/v0.3.0/modules/4337) | Cho phép EntryPoint vận hành Safe; đồng thời là fallback handler của nó | Safe |
| [SafeWebAuthnSharedSigner v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) | Xác minh chữ ký P-256 của khóa đầu tiên | Safe |
| [SafeWebAuthnSignerFactory v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) và các bộ ký nó tạo ra | Mỗi khóa thêm vào có một hợp đồng ký nhỏ | Safe |
| [ERC-4337 EntryPoint v0.7](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0) | Chạy thao tác đã ký của bạn | Các tác giả ERC-4337 |

Các hợp đồng của chính Vela không nằm trong danh sách này: **sổ đăng ký khóa công khai**
ghi lại các khóa của từng ví để thiết bị mới tìm được ví ([khôi phục](/vi/docs/recovery)),
sổ đăng ký tên miền đi kèm với nó, và chỉ mục cũ mà chúng đã thay thế. Chúng không giữ
tiền và không có vai trò gì trong Safe của bạn.

Kho mã của ví hoàn toàn không có dòng Solidity nào — bạn có thể kiểm tra chỉ bằng một
lệnh:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # không in ra gì cả
```

Vela không giữ vai trò đặc quyền nào trên tài khoản của bạn: không khóa quản trị, không
đường nâng cấp, không mô-đun nào nó có thể thêm vào. Chỉ các khóa của bạn mới thay đổi
được Safe của bạn.

## Vì sao "nguyên bản" là chữ quan trọng

Rất nhiều ví được xây trên "một Safe", một bản fork của Safe, hoặc một tài khoản lấy cảm
hứng từ Safe. Sự khác biệt quan trọng ở ba điểm.

**Các bản kiểm toán áp dụng đúng vào thứ bạn đang dùng.** Các bản kiểm toán của Safe bao
phủ chính những bản phát hành này, hoặc những bản trước đó chỉ khác chúng vài thay đổi
nhỏ đã được ghi lại ([trang kiểm toán](/vi/docs/security-audits) có chi tiết). Kiểm toán
của một bản fork chỉ bao phủ phần mã trước khi fork; phần sửa đổi chính là phần không ai
kiểm toán.

**Hệ sinh thái coi tài khoản của bạn là một Safe, vì nó đúng là một Safe.** Các trình
khám phá khối giải mã được nó, và công cụ của Safe đọc được nó và dựng được giao dịch cho
nó. Tuy nhiên, để *ký* những giao dịch đó, một chương trình phải có khả năng xin khóa của
bạn một chữ ký cho `getvela.app`, tên miền mà passkey của bạn thuộc về — nên chính ứng
dụng web của Safe, vốn chạy trên một tên miền khác, không ký thay bạn được.
[Hướng dẫn tự triển khai](/vi/docs/self-hosting#if-getvela-app-disappears) liệt kê những
gì ký được.

**Bề mặt tấn công là thứ nhiều người khác cũng đang theo dõi.** Một hợp đồng tài khoản tự
viết chủ yếu chỉ có tác giả của nó để mắt tới. Các hợp đồng lõi của Safe được theo dõi bởi
tất cả những ai giữ tiền trong một Safe; mô-đun 4337 và mô-đun passkey có ít người theo
dõi hơn, nhưng vẫn có thật.

## Cái giá phải trả

Dùng chuẩn không phải là miễn phí:

- **Gas.** Chữ ký của bạn được xác minh trên chuỗi và giao dịch chạy qua EntryPoint. Theo
  đo đạc của chúng tôi trên Gnosis (tháng 9/2026), một lần gửi đơn giản từ ví Vela đã
  triển khai tốn khoảng 140.000–170.000 gas trên chuỗi; một lần chuyển ETH thông thường
  từ tài khoản thường tốn 21.000. Ngoài gas, relay còn tính phí của nó — xem
  [mạng & phí](/vi/docs/networks-and-fees).
- **Tài khoản phải được triển khai.** Địa chỉ của bạn được tính bằng `CREATE2` trước khi
  có bất cứ thứ gì trên chuỗi, nên bạn nhận tiền vào đó được ngay; giao dịch gửi đi đầu
  tiên của bạn trên mỗi mạng sẽ trả chi phí triển khai hợp đồng.
- **Không phải chuỗi nào cũng đủ điều kiện.** Chữ ký passkey được xác minh bằng precompile
  **EIP-7951 / RIP-7212**, và địa chỉ của nó là một phần trong dữ liệu thiết lập của mọi ví, nên một
  mạng không có nó hoàn toàn không chạy được Vela.
- **Rủi ro của Safe giờ là rủi ro của bạn.** Tin một hợp đồng được dùng rộng rãi thì vẫn
  là tin một hợp đồng. Vela không thêm hợp đồng thứ hai nào của riêng mình trên đường đi
  tới tiền của bạn để bạn phải tin.

## Những gì đã và chưa được kiểm toán

Các hợp đồng của Safe, mô-đun 4337 và mô-đun passkey của Safe, cùng EntryPoint v0.7 đều có
báo cáo kiểm toán công khai của bên thứ ba. **Mã của chính Vela — các ứng dụng, các dịch
vụ phía sau và hợp đồng sổ đăng ký — chưa được bên thứ ba kiểm toán, và cũng chưa có lịch
kiểm toán nào**; đó là mục tiêu cho lúc dự án có kinh phí, không phải một cam kết có ngày
cụ thể. Mọi hợp đồng, báo cáo kiểm toán của nó và những vấn đề chúng tôi đang theo dõi đều
nằm trong [kiểm toán & vấn đề đã biết](/vi/docs/security-audits).

## Tự mình xem

Tài khoản của bạn nằm trên chuỗi. Khi ví đã được triển khai, hãy mở địa chỉ của bạn trên
một trình khám phá khối: đó là một proxy Safe có phần triển khai là bản SafeL2 v1.4.1 chính
thức của Safe, trên mọi mạng.

Tiếp theo: [kiểm toán & vấn đề đã biết](/vi/docs/security-audits).
