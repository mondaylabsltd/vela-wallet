---
title: Câu hỏi thường gặp
description: "Những câu trả lời ngắn về quyền lưu ký, khóa, khôi phục, mạng, phí, những gì Vela thấy được, mã nguồn mở, và chuyện gì xảy ra nếu Vela không còn nữa."
source: 0762e55bf54d
---

# Câu hỏi thường gặp

## Vela có phải là ví tự lưu ký không?

Có. Ví của bạn là một tài khoản thông minh Safe chỉ do các khóa của bạn kiểm soát, và các
khóa đó ở lại trong thiết bị, trình quản lý mật khẩu hoặc khóa bảo mật của bạn. Vela không
giữ khóa nào và không có vai trò gì trên ví, nên tự mình không thể chuyển, đóng băng hay khôi
phục tiền của bạn. Nhưng phần mềm yêu cầu các khóa của bạn ký là do Vela viết — xem
[mô hình mối đe dọa](/vi/docs/whitepaper).

## Thật sự không có cụm từ khôi phục sao?

Thật sự không. Khóa của bạn là passkey, và passkey không có bí mật nào để bạn chép ra hay gõ
vào. Xem [passkey hoạt động thế nào](/vi/docs/passkeys).

## Tôi cần gì để tạo ví?

Một thiết bị hỗ trợ passkey (điện thoại hoặc máy tính đời mới có Face ID, vân tay hoặc
Windows Hello), hoặc hai khóa bảo mật phần cứng. Không cần email, không cần tài khoản, không
cần số dư ban đầu. Bạn có thể tạo ví với tối đa bảy khóa; sau đó không thêm khóa được nữa.
Xem [tạo ví của bạn](/vi/docs/create-wallet).

## Mất điện thoại thì sao?

Đăng nhập trên thiết bị mới bằng bất kỳ khóa nào khác: cùng passkey đó được đồng bộ qua
Chuỗi khóa iCloud hoặc Trình quản lý mật khẩu của Google, một điện thoại khác, hoặc khóa bảo
mật của bạn. Nếu chiếc điện thoại đó giữ khóa duy nhất của bạn và khóa ấy không được đồng bộ,
ví sẽ không thể khôi phục. Xem [khôi phục & đăng nhập](/vi/docs/recovery).

## Hỗ trợ những mạng và token nào?

24 mạng EVM tích hợp sẵn, trong đó có Ethereum, Base, Arbitrum, Optimism, Polygon, BNB Chain,
Gnosis và Avalanche, cùng bất kỳ mạng EVM nào bạn tự thêm, miễn là đáp ứng yêu cầu. Coin gốc
và token ERC-20. Địa chỉ giống nhau trên mọi mạng. Xem [mạng & phí](/vi/docs/networks-and-fees).

## Chi phí bao nhiêu?

- **Các ứng dụng:** ví web, tiện ích trình duyệt và ứng dụng máy tính đều miễn phí. Ứng dụng
  iOS và Android sẽ được bán theo hình thức mua một lần trên cửa hàng; bạn cũng có thể tự biên
  dịch bất kỳ ứng dụng nào từ mã nguồn, miễn phí.
- **Mỗi giao dịch:** một khoản phí trả từ ví của bạn cho relay gửi giao dịch đó lên chuỗi. Phí
  bao gồm gas cộng phần lãi của relay, và thường gấp mười lần chi phí trên chuỗi của giao dịch
  trở lên, tối thiểu khoảng 0,01 USD. Số tiền chính xác nằm trên màn hình xác nhận và là một
  phần của thứ bạn ký. Không có khoản đặt cọc và không có phí thuê bao.
  [Cách tính phí](/vi/docs/networks-and-fees).
- **Không có token.** Vela không có token nào và không có kế hoạch phát hành.

## Tôi dùng Vela với dApp được không?

Được, qua tiện ích trình duyệt Vela (Chrome, Edge, Brave) và trình duyệt tích hợp trong ứng
dụng máy tính (macOS, Windows), iOS và Android. Ví web tại wallet.getvela.app không kết nối với
dApp. Xem [cài đặt](/vi/docs/install#dapps).

## Vela thấy được gì, làm được gì?

Vela không đọc được khóa của bạn và tự mình không chuyển được tiền của bạn. Các dịch vụ của nó
thấy địa chỉ IP của bạn và những gì ứng dụng hỏi chúng: chỉ mục thấy khóa công khai và tên ví
của bạn khi đăng ký một ví mới, và thấy những địa chỉ bạn tra cứu; relay thấy địa chỉ của bạn,
các thao tác bạn gửi và điểm cuối RPC mà ứng dụng của bạn dùng; dịch vụ dữ liệu chuỗi thấy ứng
dụng của bạn hỏi về những token và hợp đồng nào. Những gì trở nên công khai trên chuỗi được liệt
kê ở [tạo ví của bạn](/vi/docs/create-wallet#what-is-public).
[Chính sách quyền riêng tư](/privacy) là bản đầy đủ và chính thức.

## Vela có phải mã nguồn mở không?

Các ứng dụng ví, relay và dịch vụ tỷ giá đều theo giấy phép MIT trên
[GitHub](https://github.com/orgs/mondaylabsltd/repositories); danh mục dữ liệu chuỗi cũng theo
MIT. Chỉ mục khóa công khai được công khai nhưng chưa có tệp giấy phép. Bạn có thể tự chạy từng
dịch vụ — xem [hướng dẫn tự triển khai](/vi/docs/self-hosting).

## Vela đã được kiểm toán chưa?

Các hợp đồng giữ tiền của bạn — Safe cùng các mô-đun của nó, và EntryPoint của ERC-4337 — đã
được kiểm toán. Mã của chính Vela thì chưa, và cũng chưa có lịch kiểm toán nào. Xem
[kiểm toán & vấn đề đã biết](/vi/docs/security-audits).

## Nếu Vela đóng cửa thì sao?

Tiền của bạn vẫn nằm trong Safe của bạn trên chuỗi. Với một ví đã có, tiện ích trình duyệt Vela
và các ứng dụng bạn tự biên dịch vẫn tiếp tục hoạt động khi không có getvela.app, và mọi dịch vụ
đều là mã nguồn mở để người khác chạy (riêng relay cần sửa mã để thôi đọc dữ liệu chuỗi từ máy
chủ của Vela). [Hướng dẫn tự triển khai](/vi/docs/self-hosting#if-getvela-app-disappears) liệt
kê các con đường và giới hạn của chúng.

## Tôi có câu hỏi không có ở đây.

Hãy mở một issue trên [GitHub](https://github.com/mondaylabsltd/vela-wallet/issues), hoặc liên
hệ chúng tôi qua [X](https://x.com/realvelawallet) hay [Telegram](https://t.me/velawallet).
