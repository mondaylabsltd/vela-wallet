---
title: Khôi phục & đăng nhập
description: "Cách quay lại ví trên thiết bị mới bằng bất kỳ khóa nào của bạn, ví được tìm ở đâu, và những giới hạn thật thà của việc khôi phục khi không có cụm từ khôi phục."
source: 77cf3f24c7c7
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Khôi phục & đăng nhập

Không có cụm từ khôi phục, việc khôi phục dựa vào hai thứ: **một khóa bạn vẫn còn giữ**,
và **một bản ghi công khai cho biết những khóa nào thuộc về ví của bạn**.

## Những gì được ghi lại khi bạn tạo ví

Địa chỉ ví của bạn được tính từ tất cả các khóa bạn dùng khi tạo ví. Để về sau bất kỳ
khóa nào trong số đó cũng tìm lại được ví, việc tạo ví sẽ ghi một bản ghi vào một
**hợp đồng sổ đăng ký** công khai trên Gnosis Chain: khóa công khai của từng khóa, địa
chỉ ví, tên ví, và dữ liệu đăng ký đã ký. Sổ đăng ký không có chủ sở hữu và bản ghi không
thể sửa hay xóa. (Danh sách đầy đủ những gì công khai nằm ở
[tạo ví của bạn](/vi/docs/create-wallet#what-is-public).)

Dịch vụ chỉ mục khóa công khai của Vela gửi bản ghi đó lên và trả gas cho nó; còn bản
thân bản ghi nằm trên chuỗi, và bất kỳ ứng dụng nào cũng đọc trực tiếp được.

## Đăng nhập trên thiết bị mới

1. Mở Vela và chọn đăng nhập.
2. Dùng **bất kỳ khóa nào** của bạn: một passkey đã đồng bộ sang thiết bị này, một điện
   thoại ở gần (quét mã QR), hoặc khóa bảo mật của bạn.
3. Vela tính ra khóa công khai của khóa đó từ chữ ký, rồi tra nó — trước hết trong chỉ
   mục của Vela, sau đó, nếu chỉ mục không phản hồi, trong hợp đồng sổ đăng ký trên
   Gnosis rồi trên Ethereum — và dựng lại ví. Nó kiểm tra rằng các khóa tìm được thực sự
   tính ra đúng địa chỉ đã ghi trước khi hiện ví cho bạn.

Danh sách tài khoản không được đồng bộ giữa các thiết bị; đăng nhập sẽ dựng lại chúng.

<Callout type="info" title="Nếu không chỉ mục hay sổ đăng ký nào trả lời được">
Một ví chỉ có <strong>một khóa</strong> có thể được dựng lại ngay trên thiết bị mà không
cần máy chủ nào: hai chữ ký từ khóa đó là đủ để khôi phục khóa công khai và tính lại địa
chỉ. Một ví có nhiều khóa thì cần bản ghi trong sổ đăng ký, vì một khóa không thể cho ứng
dụng biết các khóa kia là gì.
</Callout>

## Các bản sao của bản ghi

Sổ đăng ký trên Gnosis là nơi các ứng dụng đọc trước tiên. Trong **Cài đặt**, bạn còn
có thể sao chép bản ghi của ví sang cùng hợp đồng sổ đăng ký đó trên **Ethereum**, tự
trả gas, để bản ghi tồn tại trên một chuỗi thứ hai. Ai cũng có thể tạo bản sao như vậy;
trong đó không có gì chuyển được tiền.

## Những giới hạn thật thà

<Callout type="warning" title="Mất khóa là mất">
Nếu mọi khóa bạn dùng khi tạo ví đều không còn — các passkey đã đồng bộ, các điện thoại,
các khóa bảo mật — thì không ai khôi phục được ví: Vela không, Apple hay Google không,
không ai cả. Không có cụm từ khôi phục, không có bộ phận hỗ trợ đặt lại giúp, và không có
cửa sau.
</Callout>

Điều khiến chuyện đó khó xảy ra là có nhiều hơn một lối vào:

- **Giữ đồng bộ passkey luôn bật** nếu bạn dùng passkey của thiết bị này. Chính nó mang
  khóa sang điện thoại hoặc máy tính mới.
- **Bảo vệ tài khoản đứng sau nó.** Người kiểm soát tài khoản Apple hoặc Google của bạn
  có thể dùng được passkey đã đồng bộ; hãy đặt mật khẩu mạnh và các phương án khôi phục
  riêng cho tài khoản đó.
- **Tạo ví với nhiều hơn một khóa**, ví dụ passkey trên điện thoại cộng một khóa bảo mật
  phần cứng cất ở nơi an toàn. Chỉ có thể thêm khóa khi tạo ví
  ([vì sao](/vi/docs/signers)). Hãy nhớ rằng bất kỳ khóa đơn lẻ nào cũng tự ký được — và
  không thể gỡ bỏ, nên nếu một khóa từng bị lộ, hãy chuyển tiền sang một ví mới
  ([cần làm gì](/vi/docs/signers)).

## Vela làm được gì và không làm được gì

- **Làm được:** duy trì chỉ mục hoạt động, để ví của bạn được tìm thấy nhanh trên thiết
  bị mới.
- **Không làm được:** chuyển tiền của bạn, đóng băng ví, thêm hay gỡ bỏ khóa, hoặc khôi
  phục một khóa bạn đã mất. Vela không bao giờ giữ khóa của bạn.

Tiếp theo: [ký minh bạch](/vi/docs/clear-signing).
