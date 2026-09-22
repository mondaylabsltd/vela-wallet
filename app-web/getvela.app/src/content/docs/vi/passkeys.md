---
title: Passkey hoạt động thế nào
description: "Passkey là gì, khóa riêng tư nằm ở đâu với từng loại khóa, vì sao không có bí mật nào để kẻ lừa đảo moi được, và passkey không bảo vệ bạn khỏi điều gì."
source: b23999b2ed69
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Passkey hoạt động thế nào

Các khóa kiểm soát một ví Vela là **passkey**: thông tin xác thực WebAuthn trên đường
cong P-256. Thiết bị hoặc khóa bảo mật của bạn tạo ra từng passkey, giữ khóa riêng tư,
và chỉ dùng nó sau khi bạn xác nhận bằng Face ID, vân tay, mã PIN của thiết bị, hoặc
chạm và nhập PIN trên khóa bảo mật. Vela không bao giờ nhận được khóa riêng tư; nếu một
trình quản lý mật khẩu đồng bộ nó, thì trình quản lý đó giữ nó thay bạn, ở dạng mã hóa.

## Passkey là gì

Passkey là một cặp khóa công khai/riêng tư được tạo ra cho một trang web duy nhất — với
Vela là `getvela.app`. Ứng dụng không bao giờ nhận được khóa riêng tư; nó chỉ có thể nhờ
trình xác thực ký một thứ gì đó, và trình xác thực sẽ hỏi bạn trước.

Khóa riêng tư nằm ở đâu tùy vào loại khóa:

| Loại khóa | Khóa riêng tư nằm ở đâu | Có đồng bộ sang thiết bị khác không? |
| --- | --- | --- |
| **Thiết bị này** — Face ID, Touch ID, vân tay, Windows Hello | Trình quản lý mật khẩu của nền tảng (Chuỗi khóa iCloud, Trình quản lý mật khẩu của Google) hoặc một trình quản lý mật khẩu như 1Password | Thường là có, mã hóa đầu cuối, nếu bật đồng bộ. Khóa Windows Hello chỉ nằm trên chiếc PC đó |
| **Một điện thoại khác**, kết nối bằng cách quét mã QR | Trình quản lý mật khẩu của điện thoại đó | Như trên |
| **Một khóa bảo mật phần cứng** (YubiKey và các khóa FIDO2 khác, qua USB hoặc NFC) | Bên trong khóa bảo mật | Không bao giờ |

Một ví Vela có thể dùng tối đa bảy khóa, kết hợp loại nào cũng được, và được chọn khi
bạn tạo ví; [khóa ký & khóa bảo mật](/vi/docs/signers) nói về lựa chọn đó.

## Không có bí mật nào để moi

Lừa đảo (phishing) hoạt động bằng cách khiến bạn tự trao ra một bí mật. Cụm từ khôi phục
là mười hai từ mà người khác có thể dụ bạn gõ vào đâu đó. Passkey **không có bí mật nào
để gõ**: không có gì để lộ, không có gì để dán, và một trang giả không thể đòi nó. Hơn
nữa, vì passkey được tạo cho một trang web duy nhất, trình duyệt chỉ đưa passkey của
`getvela.app` cho các trang trên getvela.app và các tên miền con của nó.

Điều đó loại bỏ cả một kiểu mất tiền — bị đánh cắp cụm từ khôi phục — vốn rất phổ biến
khi tự lưu ký.

## Passkey không bảo vệ bạn khỏi điều gì

<Callout type="warning" title="Bạn duyệt gì, passkey ký nấy">
Lời nhắc từ điện thoại hay trình duyệt cho biết khóa <em>nào</em> đang được dùng, không
cho biết <em>cái gì</em> đang được ký. Nếu bạn duyệt, passkey sẽ ký một giao dịch có hại
dễ dàng như ký một giao dịch bình thường. Đó là lý do Vela giải mã mọi giao dịch trước
khi bạn ký (<a href="/vi/docs/clear-signing">ký minh bạch</a>), và vì sao trang hiển thị
giao dịch lại quan trọng (<a href="/vi/docs/bybit-attack">vụ tấn công Bybit</a>).
</Callout>

Passkey cũng không bảo vệ bạn trước người đang cầm điện thoại đã mở khóa của bạn và
vượt qua được bước kiểm tra của nó, hoặc người kiểm soát tài khoản mà passkey của bạn
đồng bộ qua. Hãy đặt mã khóa cho thiết bị, bảo vệ tài khoản Apple hoặc Google của bạn,
và cân nhắc dùng một khóa bảo mật phần cứng không đồng bộ đi đâu cả.

## Ký một giao dịch diễn ra thế nào

1. Bạn xác nhận một giao dịch trong Vela, sau khi đọc xem nó làm gì.
2. Thiết bị hoặc khóa bảo mật hỏi Face ID, vân tay, mã PIN, hoặc chạm kèm PIN.
3. Nó ký, và chỉ có chữ ký được gửi lại cho ứng dụng.
4. Ứng dụng chuyển thao tác đã ký cho relay để gửi lên chuỗi; hợp đồng ví của bạn kiểm
   tra chữ ký passkey trên chuỗi trước khi làm bất cứ điều gì.

## Khóa công khai đi đâu

Phần **công khai** của các khóa được ghi vào một sổ đăng ký công khai trên Gnosis Chain,
để thiết bị mới tìm được ví của bạn. Đó là chủ đề của
[khôi phục & đăng nhập](/vi/docs/recovery).

Tiếp theo: [khóa ký & khóa bảo mật](/vi/docs/signers).
