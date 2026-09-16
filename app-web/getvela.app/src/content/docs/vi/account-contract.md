---
title: Hợp đồng tài khoản
description: Ví Vela của bạn là một Safe v1.4.1 không sửa đổi. Không dòng nào trên đường đi của hợp đồng do chúng tôi viết — đây là những gì bạn được, và cái giá phải trả.
---

# Hợp đồng tài khoản

Ví của bạn không phải cấu trúc dữ liệu riêng của một ứng dụng. Nó là một tài khoản
thông minh **Safe v1.4.1** — đúng hợp đồng đang giữ những ngân quỹ lớn hơn bất cứ thứ
gì Vela từng thấy — được triển khai đúng như Safe công bố, không sửa một dòng.

Câu đó ngắn nhưng hệ quả thì không, nên trang này viết rõ ra.

## Không thứ gì trên đường đi là của chúng tôi

Có bốn hợp đồng đứng giữa bạn và tiền của bạn. Vela không viết cái nào:

| Hợp đồng | Ai viết |
| --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1) (chính tài khoản, một proxy) | Safe |
| [Safe 4337 Module](https://github.com/safe-global/safe-modules/tree/main/modules/4337) | Safe |
| [SafeWebAuthnSharedSigner](https://github.com/safe-global/safe-modules/tree/main/modules/passkey) (xác minh khóa P-256 của bạn) | Safe |
| [ERC-4337 EntryPoint v0.7](https://eips.ethereum.org/EIPS/eip-4337) | Nhóm tác giả ERC-4337 |

Không có hợp đồng nào của Vela. Kho mã hoàn toàn không chứa Solidity — bạn kiểm tra
bằng một câu lệnh:

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # không in ra gì
```

Khi Vela thêm một mạng, nó triển khai **chính những** hợp đồng ấy ở địa chỉ chuẩn của
chúng. Nó không triển khai hợp đồng do mình thiết kế, và nó không giữ vai trò đặc
quyền nào trên ví bạn: không khóa quản trị, không đường nâng cấp, không module nào
chúng tôi thêm được.

## Vì sao "không sửa đổi" mới là từ quan trọng

Rất nhiều ví được dựng trên "một cái Safe", "một bản fork của Safe" hoặc "một tài
khoản lấy cảm hứng từ Safe". Fork là một hợp đồng mới đội danh tiếng cũ. Khác biệt,
một cách thực tế:

**Các bản kiểm toán áp dụng đúng vào thứ bạn đang dùng.** Báo cáo kiểm toán của Safe
bao phủ bytecode ở đúng những bản phát hành này. Kiểm toán của một bản fork chỉ bao
phủ phần mã trước khi fork. Nếu một chiếc ví đã sửa hợp đồng tài khoản, mọi kiểm toán
nó viện dẫn đều là kiểm toán của một thứ khác, còn phần sửa đổi chính là phần không ai
xem.

**Hệ sinh thái coi tài khoản của bạn là một Safe, vì nó đúng là vậy.** Các trình duyệt
khối giải mã được nó. Công cụ giao dịch của chính Safe hiểu nó. Nếu mai Vela biến mất,
ví của bạn không phải một định dạng mồ côi — nó là tài khoản thông minh có nhiều công
cụ hỗ trợ nhất trên Ethereum, và bất kỳ giao diện tương thích Safe nào cũng điều khiển
được. Đó là điều làm cho câu
["Vela biến mất thì ví của bạn vẫn còn"](/vi/docs/why-vela) là một phát biểu về hợp
đồng chứ không phải về ý định của chúng tôi.

**Bề mặt tấn công là bề mặt mà tất cả mọi người cũng đang nhìn.** Một hợp đồng tài
khoản đóng riêng chỉ có tác giả của nó nhìn. Cái này thì mọi người đang giữ tiền trong
Safe đều nhìn.

## Cái giá phải trả

Theo chuẩn không miễn phí, và các đánh đổi là thật:

- **Gas.** Tài khoản thông minh xác minh chữ ký trên chuỗi. Hãy chuẩn bị tinh thần tốn
  khoảng 1,5–3× gas của một giao dịch EOA thông thường, tùy chuỗi. Xem
  [mạng & phí](/vi/docs/networks-and-fees).
- **Tài khoản phải được triển khai.** Địa chỉ của bạn được tính bằng `CREATE2` trước
  khi có gì trên chuỗi, nên bạn nhận tiền được ngay; nhưng giao dịch đi đầu tiên sẽ trả
  phí triển khai hợp đồng.
- **Không phải chuỗi nào cũng đủ điều kiện.** Bộ ký WebAuthn xác minh chữ ký P-256
  trên chuỗi, việc này cần precompile **RIP-7212**. Vela thà từ chối bật một mạng
  thiếu nó còn hơn lùi về một bộ xác minh yếu hơn.
- **Rủi ro của Safe giờ là rủi ro của bạn.** Tin vào một hợp đồng được dùng rộng rãi
  thì vẫn là tin vào một hợp đồng. Điều Vela nói được là chúng tôi không chồng thêm
  một thứ thứ hai để bạn phải tin.

## Cái gì được kiểm toán, cái gì không

Các hợp đồng của Safe và module ký WebAuthn đã được bên thứ ba kiểm toán, và những báo
cáo đó công khai. **Mã ứng dụng của chính Vela chưa được kiểm toán độc lập**, và cũng
chưa có lịch kiểm toán — đó là mục tiêu cho lúc dự án đủ sức chi trả, không phải một
cam kết có ngày tháng. Mọi hợp đồng Vela phụ thuộc vào, báo cáo kiểm toán của nó và
những vấn đề chúng tôi đang theo dõi đều nằm ở
[kiểm toán & vấn đề đã biết](/vi/docs/security-audits).

## Tự mình xem

Tài khoản của bạn nằm trên chuỗi. Mở nó trong một trình duyệt khối và đọc địa chỉ
implementation: đó sẽ là bản triển khai chuẩn v1.4.1 của Safe, từng byte một, trên mọi
mạng Vela hỗ trợ.
