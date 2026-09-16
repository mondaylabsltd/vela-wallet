---
title: Passkey hoạt động thế nào
description: "Mô hình bảo mật đằng sau Vela: passkey là gì, khóa của bạn nằm ở đâu, và vì sao chẳng có gì để kẻ lừa đảo moi được."
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Passkey hoạt động thế nào

Toàn bộ mô hình bảo mật của Vela nằm ở một ý: khóa kiểm soát ví của bạn là một
**passkey** do thiết bị tạo ra và hệ điều hành giữ — không ứng dụng nào, kể cả Vela,
đọc được — và chỉ dùng được bằng khuôn mặt hoặc vân tay của bạn.

## Passkey thực ra là gì

Passkey là một cặp khóa công khai/riêng tư do thiết bị của bạn tạo ra. **Khóa riêng
tư** nằm ở trình cung cấp passkey của hệ điều hành — thường là Chuỗi khóa iCloud trên
Apple, Trình quản lý mật khẩu Google trên Android — được lưu mã hóa đầu-cuối, nên
không ứng dụng nào đọc hay sao chép được. Ứng dụng không nhận khóa; sau khi bạn xác
thực, chúng chỉ được phép *nhờ thiết bị của bạn ký một thứ gì đó*.

Đây chính là công nghệ bảo vệ Apple Pay và cơ chế mở khóa sinh trắc học của bạn.

<Callout type="info" title="Điểm mấu chốt">
Một ứng dụng — kể cả Vela — có thể yêu cầu chữ ký, nhưng không bao giờ thấy khóa riêng
tư của bạn. Khuôn mặt hoặc vân tay cho phép thiết bị của bạn ký; bản thân khóa vẫn ở
lại với hệ điều hành, mã hóa đầu-cuối.
</Callout>

## Vì sao chẳng có gì để moi

Lừa đảo hoạt động bằng cách khiến bạn tự trao ra một bí mật. Với cụm từ khôi phục, bí
mật đó là mười hai từ mà bạn có thể gõ vào một trang giả. Với passkey, **không có bí
mật nào để gõ cả**. Một trang lừa đảo không thể bảo bạn "nhập passkey của bạn", vì
passkey không phải thứ nhập được — nó là một thao tác phần cứng được chốt bằng sinh
trắc học của bạn.

Điều đó loại bỏ cách phổ biến nhất khiến người ta mất tiền khi tự quản.

## Ký một giao dịch cảm giác thế nào

1. Bạn xác nhận một giao dịch trong Vela.
2. Thiết bị hỏi Face ID / Touch ID.
3. Thiết bị ký giao dịch bằng passkey của bạn.
4. Vela phát giao dịch đã ký lên mạng.

Vẫn là động tác mở khóa điện thoại — vì đó đúng là cơ chế passkey mà thiết bị của bạn
đã dùng ở mọi nơi khác.

<Callout type="warning" title="An toàn thiết bị vẫn quan trọng">
Passkey chống tấn công từ xa và lừa đảo cực tốt. Nó không chống được người đang cầm
thiết bị đã mở khóa của bạn và qua được bước sinh trắc học. Hãy đặt mã khóa máy và
đừng đưa điện thoại đang mở khóa cho người bạn không tin.
</Callout>

## Phần còn lại nằm ở đâu

Khóa **công khai** của passkey được ghi lên một chỉ mục nhỏ trên chuỗi để ví của bạn
có thể khôi phục trên thiết bị mới. Đó là chủ đề của trang kế tiếp:
[khôi phục & đăng nhập](/vi/docs/recovery).
