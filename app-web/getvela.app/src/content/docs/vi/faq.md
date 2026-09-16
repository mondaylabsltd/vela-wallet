---
title: Câu hỏi thường gặp
description: Những câu hỏi hay gặp về Vela — quyền giữ tiền, passkey, tài khoản thông minh, khôi phục, mạng được hỗ trợ, phí và quyền riêng tư.
---

# Câu hỏi thường gặp

## Vela có phải ví tự quản không?

Đúng. Ví của bạn là một tài khoản thông minh do một khóa chỉ mình bạn dùng được kiểm
soát; khóa đó do hệ điều hành của thiết bị giữ và Vela không bao giờ thấy. Vela không
thể chuyển, đóng băng hay khôi phục tiền của bạn.

## Ví của tôi là tài khoản thường hay hợp đồng?

Nó là một **tài khoản thông minh Safe** (một hợp đồng thông minh), vận hành bằng trừu
tượng hóa tài khoản ERC-4337. Chính điều đó cho phép bạn ký bằng passkey, đọc từng giao
dịch trước khi duyệt, và dùng cùng một địa chỉ trên mọi mạng. Xem
[whitepaper](/vi/docs/whitepaper) để biết kiến trúc.

## Thật sự không có cụm từ khôi phục sao?

Thật. Khóa ký của bạn là một passkey do hệ điều hành của thiết bị giữ, và Vela không
bao giờ thấy nó. Không có mười hai từ nào để chép lại, để mất hay để bị lừa lấy. Đọc
[passkey hoạt động thế nào](/vi/docs/passkeys) để biết vì sao như thế là an toàn.

## Mất điện thoại thì sao?

Nếu passkey của bạn được đồng bộ qua Chuỗi khóa iCloud hoặc Trình quản lý mật khẩu
Google, bạn đăng nhập trên thiết bị mới bằng cùng tài khoản và ví trở lại. Xem
[khôi phục & đăng nhập](/vi/docs/recovery) để biết toàn bộ mô hình và giới hạn của nó.

## Hỗ trợ những mạng và token nào?

Vela có sẵn **12 mạng EVM** — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base,
Avalanche, Gnosis, Unichain, Tempo, Monad và World Chain — cộng thêm mạng tùy chọn, giữ
token gốc và token ERC-20. Địa chỉ của bạn giống nhau trên tất cả. Xem
[mạng & phí](/vi/docs/networks-and-fees).

## Chi phí bao nhiêu?

Ví miễn phí và Vela **không có token**. Bạn trả **gas** của mạng từ chính số dư ví của
mình, cộng một khoản phí relay. Giá do relay báo và được hiện thành _phí mạng / phí
relay / tổng_ **trước khi bạn ký** — chi phí chính xác của mọi giao dịch nằm trên màn
hình xác nhận, và số tiền báo giá là một phần của thứ bạn ký nên nó không đổi được về
sau. Giao dịch rất rẻ có thể chạm mức phí tối thiểu nhỏ. Trên Tempo, vốn không có đồng
gốc, gas trả bằng stablecoin USD. Mỗi mạng còn cần một khoản **đặt cọc nhỏ không hoàn
lại để kích hoạt tài khoản relay gas** (Vela có thể tài trợ cho người dùng mới); vì tài
khoản đó có thể cạn, về sau bạn có thể phải nạp lại — nên nó không hẳn là một lần duy
nhất. Chi tiết ở [mạng & phí](/vi/docs/networks-and-fees).

## Vela (công ty) thấy được gì, làm được gì?

Vela lưu khóa **công khai** của passkey và **cái tên** bạn chọn, để cho phép đăng nhập
xuyên thiết bị. Nó không thấy khóa riêng tư của bạn, số dư được đọc từ các chuỗi công
khai, và không có chuyện đăng ký bằng email. Bản có hiệu lực là
[chính sách riêng tư](/privacy).

## Vela có mã nguồn mở không?

Có — ví và bốn dịch vụ phía sau (dữ liệu chuỗi, chỉ mục passkey, relay, tỷ giá) đều
[công khai trên GitHub](https://github.com/mondaylabsltd/vela-wallet) theo giấy phép
MIT, và bạn tự vận hành được.

## Tôi có câu hỏi không nằm ở đây.

Hãy mở một issue trên [GitHub](https://github.com/mondaylabsltd/vela-wallet) hoặc liên
hệ chúng tôi qua [X](https://x.com/realvelawallet) hay
[Telegram](https://t.me/velawallet).
