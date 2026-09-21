---
title: Tạo ví của bạn
description: "Tạo một ví Vela với từ một đến bảy khóa — mỗi bước làm gì, vì sao khóa được cố định từ lúc tạo ví, những gì trở nên công khai, và ví của bạn thực chất là gì."
source: a2edda21a075
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Tạo ví của bạn

Tạo ví mất một, hai phút. Mở ví web tại
[wallet.getvela.app](https://wallet.getvela.app/) — hoặc tiện ích, ứng dụng máy tính
hay ứng dụng điện thoại — rồi chọn **Tạo ví**.

## Các bước

1. **Đặt tên cho ví.** Cái tên giúp bạn nhận ra ví, và nó được ghi vào một sổ đăng ký
   công khai cùng với các khóa của bạn — hãy coi nó là công khai, đừng đặt gì riêng tư
   vào đó.
2. **Xác nhận những gì sẽ xảy ra.** Bạn đánh dấu xác nhận rằng khóa công khai và tên ví
   của bạn sẽ được ghi lên chuỗi, rằng khóa riêng tư ở lại trong thiết bị hoặc khóa bảo
   mật của bạn, và rằng bạn đồng ý với [điều khoản](/terms) và
   [chính sách quyền riêng tư](/privacy).
3. **Tạo khóa đầu tiên.** Chọn cách tạo: **thiết bị này** (Face ID, Touch ID, vân tay,
   Windows Hello), **điện thoại hoặc máy tính bảng** (quét mã QR và tạo khóa trên thiết
   bị đó, ở những ứng dụng có tùy chọn này), hoặc **khóa bảo mật USB**. Thiết bị tạo
   passkey rồi dùng nó ký thử một lần, để ứng dụng biết khóa thực sự hoạt động trước khi
   đi tiếp.
4. **Thêm khóa nếu bạn muốn.** Tổng cộng tối đa bảy khóa, loại nào cũng được. Bất kỳ
   khóa nào trong số đó cũng sẽ tự ký được một mình. Nếu khóa duy nhất của bạn không được
   đồng bộ đi đâu cả — một khóa bảo mật, hoặc Windows Hello — ứng dụng sẽ yêu cầu thêm
   khóa thứ hai, vì chỉ có một khóa không đồng bộ thì mất một thiết bị là mất luôn ví.
5. **Tạo.** Ứng dụng tính địa chỉ ví từ toàn bộ tập khóa và công bố tập khóa đó lên sổ
   đăng ký công khai trên Gnosis Chain. Khi bản ghi đã nằm trên chuỗi, ví của bạn sẽ mở
   ra.

<Callout type="warning" title="Hãy chọn khóa ngay bây giờ">
Địa chỉ của bạn được tính từ tập khóa cuối cùng bạn chốt, nên về sau không thể thêm,
gỡ bỏ hay thay khóa. [Khóa ký & khóa bảo mật](/vi/docs/signers) giải thích vì sao, và
cách chọn khóa.
</Callout>

## Ví của bạn là gì

Ví của bạn là một **tài khoản thông minh Safe** — một hợp đồng, không phải một tài
khoản thường chỉ có một khóa riêng tư. Các khóa của bạn là chủ sở hữu của nó, và bất
kỳ khóa nào trong số đó cũng có thể phê duyệt một giao dịch.
[Hợp đồng tài khoản](/vi/docs/account-contract) liệt kê mọi hợp đồng liên quan.

Địa chỉ **giống nhau trên mọi mạng**, và mang tính **phản thực** (counterfactual): nó
được tính trước khi có bất cứ thứ gì được triển khai, nên bạn nhận tiền được ngay trên
bất kỳ mạng nào. Hợp đồng tự triển khai vào lần đầu bạn gửi đi từ một mạng, và phí của
giao dịch đầu tiên đó đã bao gồm chi phí triển khai. Việc tạo ví không tốn của bạn đồng
nào.

## Những gì công khai

<span id="what-is-public"></span>

Tạo ví sẽ ghi một bản ghi vĩnh viễn vào một hợp đồng sổ đăng ký công khai trên Gnosis
Chain; ai cũng đọc được, và không thể sửa hay xóa:

- **khóa công khai** của từng khóa (không bao giờ là khóa riêng tư) và **ID thông tin
  xác thực** của nó;
- **mẫu trình xác thực** của từng khóa (trình quản lý mật khẩu hay khóa bảo mật nào đã
  tạo ra nó) cùng các cờ cho biết bạn có được xác minh hay không và khóa có được đồng bộ
  hay không;
- **tên ví** và **nhãn của từng khóa**;
- **địa chỉ ví** và thời điểm tạo;
- chính **dữ liệu đăng ký đã ký**.

Chỉ mục khóa công khai của Vela gửi bản ghi này lên và trả gas cho nó, nên nó là bên
thấy bản ghi trước tiên. Không có gì trong đó chuyển được tiền của bạn; chính nó cho
phép bất kỳ khóa nào của bạn tìm lại ví trên thiết bị mới
([khôi phục](/vi/docs/recovery)). [Chính sách quyền riêng tư](/privacy) có danh sách
đầy đủ, và [trang sổ đăng ký](/registry) hiển thị mọi bản ghi.

## Bước tiếp theo

- [Nhận những token đầu tiên](/vi/docs/send-and-receive)
- [Hiểu về mạng và phí](/vi/docs/networks-and-fees)
- [Làm gì nếu bạn mất một thiết bị](/vi/docs/recovery)
