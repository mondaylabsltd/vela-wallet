---
title: Vì sao chúng tôi làm Vela
description: "Bản dài — mười hai từ đó bạn phải cất ở đâu, passkey đã thay đổi điều gì, những gì chúng tôi không chấp nhận được ở những chiếc ví mình đang dùng, và sự đánh đổi mà chúng tôi chọn thay vào đó."
source: 06307425f631
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Vì sao chúng tôi làm Vela

Chúng tôi không định làm thêm một chiếc ví nữa. Chúng tôi bắt đầu từ một câu hỏi mà
mình chưa bao giờ trả lời được cho gọn:

> Mười hai từ đó bạn phải cất ở đâu?

## Câu trả lời thật thà là một ảnh chụp màn hình

Bỏ vào Ghi chú thì chỉ cần mất điện thoại là gặp rắc rối. Chép ra giấy thì giờ bạn
phải lo chuyện cháy, nước, chuyển nhà, bạn cùng phòng, túi rác, và liệu bạn của sau
này có còn nhớ "chỗ an toàn" là chỗ nào.

Với rất nhiều người, câu trả lời thật thà là một ảnh chụp màn hình trong thư viện ảnh.
Ai cũng biết như thế là sai. Họ vẫn làm — vì câu trả lời "đúng" quá khó để duy trì
trong đời sống hằng ngày.

Cụm từ khôi phục là một bí mật phải sống sót qua hàng chục năm đời thường mà không một
lần bị sao chép, chụp ảnh, gõ nhầm ô, hay đọc to cho một người "tốt bụng" nào đó qua
điện thoại. Với một người cẩn thận, đó không phải bài toán khó. Với một con người, đó
là bài toán khó.

## Rồi passkey thay đổi cảm giác dùng một chiếc ví

Chúng tôi dùng [Base Account](https://account.base.app) mỗi ngày, và ký bằng Face ID
thấy hiển nhiên theo cách mà cụm từ khôi phục chưa bao giờ có được — bớt giống việc
cầm nắm vật liệu nguy hiểm, giống việc dùng phần còn lại của internet hơn.

Nhưng càng dùng, chúng tôi càng đụng phải những giới hạn không thể làm ngơ:

- một **khóa khôi phục được tạo ra trong trình duyệt** mà bạn chỉ còn cách tin,
- **không thêm được mạng tùy chỉnh**,
- **không thể tự triển khai**,
- và vấn đề lặng lẽ mà lớn nhất: **nếu dịch vụ biến mất, chiếc ví cũng biến mất theo.**

Nên chúng tôi làm ra phiên bản mà chính mình muốn dựa vào.

## Vela thực ra là gì

Vela là **một chiếc ví passkey mà bạn sở hữu trọn vẹn.**

Passkey của bạn ở nguyên nơi thiết bị vốn đã bảo vệ nó — Chuỗi khóa iCloud, Trình
quản lý mật khẩu của Google, hoặc một khóa bảo mật phần cứng bạn tự cầm. Khi bạn ký một
giao dịch, Vela nhờ thiết bị của bạn ký; thiết bị ký và chỉ gửi lại chữ ký. Vela không
bao giờ thấy bản thân chiếc khóa.

Phần lớn ví vẫn có một khoảnh khắc nguy hiểm, dù ngắn ngủi: những con chữ trên màn
hình, một cụm từ khôi phục trong bộ nhớ, một khóa khôi phục nằm trong tab trình duyệt.
Vela được thiết kế để khoảnh khắc đó không bao giờ tồn tại.

<Callout type="info" title="Không phải lời hứa — mà là kiến trúc">
Chúng tôi không truy cập được khóa của bạn. Không phải "chúng tôi hứa sẽ không" — mà là
trong Vela không có đường mã nào làm được điều đó; WebAuthn không cho phép. Chiếc ví là
một <a href="/vi/docs/account-contract">tài khoản thông minh Safe</a> được vận hành bằng
chữ ký do thiết bị của bạn tạo ra, còn chúng tôi chỉ nhận lại chữ ký ấy. Điều mà ứng
dụng bạn dùng để ký thực sự quyết định là khóa của bạn được yêu cầu ký <em>cái gì</em> —
và đó là lý do phần <a href="/vi/docs/whitepaper">mô hình mối đe dọa</a> dành nhiều
trang cho chuyện này.
</Callout>

Chúng tôi làm Vela **mã nguồn mở** để bạn tự kiểm chứng được điều đó, và **tự triển
khai được** để một chiếc ví đã có vẫn tiếp tục hoạt động mà không cần máy chủ của công
ty chúng tôi — với một giới hạn: tên miền mà passkey của bạn thuộc về, điều được
[hướng dẫn tự triển khai](/vi/docs/self-hosting) giải thích cùng các cách vượt qua.
Và chúng tôi xây trên
[hợp đồng Safe](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
nguyên bản, vì khi dính đến tiền của người khác thì con đường nhàm chán nhưng đã qua
thử lửa mới là con đường đúng — chính những hợp đồng đang bảo vệ hàng tỷ đô la trên
chuỗi.

## Sự đánh đổi mà chúng tôi chọn

Vẫn có một sự đánh đổi, và giấu nó đi thì không thật thà.

Với Vela, tài khoản Apple hay Google của bạn có vai trò quan trọng, vì đó là nơi passkey
đã đồng bộ nằm. Mất tài khoản đó, hoặc xóa passkey, thì không có cụm từ khôi phục, không
có bộ phận hỗ trợ đặt lại giúp, không có cửa sau.

Nhưng ví tự lưu ký nào cũng bắt bạn chọn rủi ro mà mình sẵn lòng sống chung. Cụm từ
khôi phục có thể bị sao chép, bị chụp màn hình, bị lừa lấy, hoặc bị gõ nhầm vào một
trang web lúc một giờ sáng. Passkey thì khác: không có con chữ nào để lộ, không có bí
mật nào để dán, và không trang giả nào lừa được bạn trao nó ra. Trình duyệt chỉ đưa nó
cho các trang trên đúng tên miền thật.

Và lựa chọn không chỉ có hai đầu. Một chiếc ví có thể được tạo với **tối đa bảy khóa
ký**, bất kỳ khóa nào cũng tự ký được một mình — passkey trên nhiều thiết bị, một điện
thoại ở gần mà bạn quét mã, hoặc một khóa bảo mật USB/NFC. Nếu bạn muốn ví của mình hoàn
toàn không phụ thuộc vào tài khoản nền tảng nào, bạn có thể chỉ dùng khóa bảo mật phần
cứng — hai chiếc, vì một chiếc ví không thể chỉ dựa vào một khóa duy nhất không đồng bộ
đi đâu cả. Điều kiện duy nhất là thời điểm: địa chỉ của bạn được suy ra từ toàn bộ tập
khóa, nên chúng phải được chọn ngay khi bạn tạo ví.

<Callout type="warning" title="Điều mà khóa thêm không làm được">
Khóa ký thêm vào là một lối quay lại, không phải một ổ khóa thứ hai. Vì bất kỳ khóa đơn
lẻ nào cũng ký được, thêm một khóa phần cứng bảo vệ bạn khỏi việc <em>mất</em> quyền truy
cập — nó không ngăn được kẻ đã chiếm được một trong các khóa của bạn. Đó là hình hài thật
thà của 1-of-n.
</Callout>

## Vì thế Vela tồn tại

Một chiếc ví không có cụm từ khôi phục phải giấu, không có khóa khôi phục phải tin, và
không có công ty nào mà bạn phải cầu cho nó tồn tại mãi.

Nếu bạn muốn kiểm chứng thay vì chỉ tin lời: [whitepaper](/vi/docs/whitepaper) có phần
kiến trúc, [Kiểm toán & vấn đề đã biết](/vi/docs/security-audits) có mọi hợp đồng chúng
tôi phụ thuộc vào cùng những gì đã và chưa được kiểm toán, và toàn bộ mã nguồn nằm
[trên GitHub](https://github.com/mondaylabsltd/vela-wallet).

Tiếp theo: [cài đặt Vela](/vi/docs/install).
