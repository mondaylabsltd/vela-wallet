---
title: Vì sao chúng tôi làm Vela
description: Bản dài — mười hai từ đó thì bạn cất ở đâu, passkey đã thay đổi điều gì, chúng tôi không chấp nhận được gì ở những chiếc ví đang dùng, và chúng tôi chọn đánh đổi nào thay vào đó.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Vì sao chúng tôi làm Vela

Chúng tôi không định làm thêm một chiếc ví nữa. Chúng tôi bắt đầu từ một câu hỏi chưa
bao giờ trả lời gọn được:

> Mười hai từ đó thì bạn cất ở đâu?

## Câu trả lời thật thà là một ảnh chụp màn hình

Bỏ vào Ghi chú thì chỉ cách rắc rối đúng một chiếc điện thoại bị mất. Chép ra giấy thì
giờ bạn phải nghĩ về cháy, nước, chuyện chuyển nhà, bạn cùng phòng, túi rác, và liệu
bạn-của-tương-lai có nhớ "chỗ an toàn" là chỗ nào không.

Với rất nhiều người, câu trả lời thật thà là một ảnh chụp màn hình trong thư viện ảnh.
Ai cũng biết thế là sai. Họ vẫn làm — vì câu trả lời "đúng" quá khó để sống cùng.

Cụm từ khôi phục là một bí mật phải sống sót qua hàng chục năm đời thường mà không một
lần bị sao chép, chụp ảnh, gõ nhầm ô, hay đọc to cho một người tốt bụng nào đó qua điện
thoại. Với một người cẩn thận thì đó không phải bài toán khó. Với một con người thì
đó là bài toán khó.

## Rồi passkey thay đổi cảm giác dùng một chiếc ví

Chúng tôi dùng [Base Account](https://account.base.app) mỗi ngày, và ký bằng Face ID
thấy hiển nhiên theo cách mà cụm từ khôi phục chưa bao giờ làm được — bớt giống việc
cầm nắm chất nguy hiểm, giống việc dùng phần còn lại của internet hơn.

Nhưng càng dùng, chúng tôi càng đụng những giới hạn không thể bỏ qua:

- một **khóa khôi phục được tạo ra ngay trong trình duyệt** mà bạn chỉ còn cách tin,
- **không thêm được mạng tùy chọn**,
- **không tự vận hành được phần nào**,
- và vấn đề âm thầm mà lớn nhất: **nếu dịch vụ đóng cửa thì chiếc ví đi theo.**

Nên chúng tôi làm phiên bản mà chính mình sẵn sàng phụ thuộc vào.

## Vela thực ra là gì

Vela là **một chiếc ví passkey mà bạn sở hữu trọn vẹn.**

Passkey của bạn ở nguyên nơi thiết bị đã bảo vệ nó — Chuỗi khóa iCloud, Trình quản lý
mật khẩu Google, hoặc một khóa bảo mật phần cứng bạn cầm trong tay. Khi bạn ký một
giao dịch, Vela gửi một thử thách tới thiết bị của bạn; thiết bị ký và chỉ gửi lại chữ
ký. Vela không bao giờ thấy bản thân chiếc khóa.

Phần lớn ví vẫn có một khoảnh khắc nguy hiểm, dù ngắn: những con chữ trên màn hình,
một cụm từ khôi phục trong bộ nhớ, một khóa khôi phục nằm trong tab trình duyệt. Vela
được thiết kế để khoảnh khắc đó không bao giờ tồn tại.

<Callout type="info" title="Không phải lời hứa — mà là kiến trúc">
Chúng tôi không truy cập được khóa của bạn. Không phải "chúng tôi hứa sẽ không" — mà
là trong Vela không tồn tại đường mã nào làm được điều đó. Chiếc ví là một
<a href="/vi/docs/security-audits">tài khoản thông minh Safe</a> vận hành bằng một chữ
ký do thiết bị của bạn tạo ra và chúng tôi chỉ nhận lại.
</Callout>

Chúng tôi làm Vela **mã nguồn mở** để bạn tự kiểm tra được điều đó, và **tự vận hành
được** để chiếc ví của bạn không bao giờ phụ thuộc vào việc công ty chúng tôi còn
online. Và chúng tôi dựng trên
[hợp đồng Safe](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
không sửa đổi, vì khi dính đến tiền của người khác thì con đường buồn tẻ và đã qua thử
lửa mới là con đường đúng — đúng những hợp đồng đang giữ hàng tỷ đô trên chuỗi.

## Đánh đổi mà chúng tôi chọn

Vẫn có một đánh đổi, và giấu nó đi thì không thật thà.

Với Vela, tài khoản Apple hay Google của bạn có vai trò, vì đó là nơi passkey đã đồng
bộ đang nằm. Mất tài khoản đó, hoặc xóa passkey, thì không có cụm từ khôi phục, không
có nút đặt lại từ hỗ trợ, không có cửa sau.

Nhưng mọi ví tự quản đều bắt bạn chọn xem mình muốn sống cùng rủi ro nào. Cụm từ khôi
phục có thể bị sao chép, bị chụp màn hình, bị lừa lấy, hoặc bị gõ nhầm vào một trang
web lúc một giờ sáng. Passkey thì khác: không có con chữ nào để lộ ra, không có bí mật
nào để dán đi, và không trang giả nào lừa được bạn trao nó. Thiết bị của bạn ký cho
đúng tên miền thật, hoặc không ký gì cả.

Và lựa chọn không phải chỉ có hai đầu. Một chiếc ví có thể được tạo với **tối đa bảy
khóa ký**, bất kỳ khóa nào cũng tự ký được — passkey trên nhiều thiết bị, một chiếc
điện thoại ở gần mà bạn quét mã, hay một khóa bảo mật USB/NFC. Nếu bạn muốn ví của
mình không phụ thuộc chút nào vào tài khoản nền tảng, bạn có thể chọn chính khóa đầu
tiên là một khóa bảo mật phần cứng. Điều kiện duy nhất là thời điểm: địa chỉ của bạn
được suy ra từ toàn bộ tập khóa, nên chúng phải được chọn khi bạn tạo ví.

<Callout type="warning" title="Điều này không mang lại cho bạn thứ gì">
Thêm khóa ký là thêm một lối quay vào, không phải thêm một ổ khóa. Vì bất kỳ khóa đơn
lẻ nào cũng ký được, thêm một khóa phần cứng bảo vệ bạn khỏi việc <em>mất</em> quyền
truy cập — nó không chặn được kẻ đã chiếm được một trong các khóa của bạn. Đó là hình
hài thật thà của 1-of-n.
</Callout>

## Vì thế Vela tồn tại

Một chiếc ví không có cụm từ khôi phục để giấu, không có khóa khôi phục để phải tin,
và không có công ty nào mà bạn buộc phải mong nó sống mãi.

Nếu bạn muốn kiểm chứng thay vì tin lời: [whitepaper](/vi/docs/whitepaper) có phần
kiến trúc, [Kiểm toán & vấn đề đã biết](/vi/docs/security-audits) có mọi hợp đồng
chúng tôi phụ thuộc vào cùng những gì đã và chưa được kiểm toán, và toàn bộ mã nguồn
nằm [trên GitHub](https://github.com/mondaylabsltd/vela-wallet).

Tiếp theo: [cài đặt Vela](/vi/docs/install).
