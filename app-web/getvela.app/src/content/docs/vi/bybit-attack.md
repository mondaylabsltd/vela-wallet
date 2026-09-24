---
title: Vụ tấn công Bybit, và con đường nó đã dùng
description: "Tháng 2/2025, Bybit mất khoảng 1,5 tỷ USD. Hợp đồng Safe không bị phá — giao diện mới bị phá. Trang này giải thích con đường tấn công đó, và những gì trong thiết kế của Vela chặn nó lại."
source: 14ae76da6694
---

# Vụ tấn công Bybit, và con đường nó đã dùng

Ngày 21/2/2025, Bybit mất khoảng **1,5 tỷ USD** từ một ví lạnh đa chữ ký Safe. Đó là vụ
trộm lớn nhất trong lịch sử ngành, và đáng đọc kỹ, vì gần như mọi thứ trong vụ này đều
*đúng*, trừ một điều.

## Chuyện đã xảy ra

Bản ngắn, theo các báo cáo phân tích sau sự cố được công bố:

1. Kẻ tấn công chiếm được **máy của một lập trình viên `Safe{Wallet}`** và chèn
   JavaScript độc hại vào bucket AWS S3 đang phục vụ giao diện `Safe{Wallet}`. Mã được
   cài vào ngày 19/2 và kích hoạt ngày 21/2, nhắm đúng vào chiếc Safe cụ thể của Bybit.
2. Những người ký của Bybit mở giao diện và xem một giao dịch trông rất bình thường.
3. Dữ liệu thực sự được gửi tới **ví cứng** của họ không phải giao dịch đó. Đó là một
   `delegatecall` ghi đè `masterCopy` của proxy Safe — ô nhớ số 0 — thay toàn bộ phần
   triển khai (implementation) của tài khoản bằng phần của kẻ tấn công.
4. Những người ký đã duyệt. Chữ ký hợp lệ. Hợp đồng làm đúng những gì nó được bảo.

Vụ việc được công khai quy cho hoạt động liên quan tới Triều Tiên (FBI gọi tên cụm
TraderTraitor).

## Cái gì *không* bị phá

- **Không phải hợp đồng Safe.** Chúng thực thi một chỉ thị được ký hợp lệ. Không lỗi
  nào của Safe bị khai thác.
- **Không phải mật mã học.** Mọi chữ ký đều là thật.
- **Không phải ví cứng.** Thiết bị Ledger có mặt trong quy trình và vẫn ký — vì ví cứng
  hiện cho bạn thứ nó được đưa, và thứ nó được đưa chính là dữ liệu độc hại. Một thiết bị
  không giải mã nổi `delegatecall` thành thứ con người đánh giá được thì nó bảo vệ
  *khóa*, chứ không bảo vệ *quyết định*.

Cái bị phá là giả định nằm bên dưới mọi giao diện ví: **rằng màn hình mô tả giao dịch
và những byte đang được ký là cùng một thứ.**

## Vì sao đây là trường hợp chung, không phải tai nạn hy hữu

Phần lớn chữ ký được tạo ra trong một ví web đều dựa trên giả định ấy. Giao diện dựng
dữ liệu, giao diện hiển thị bản tóm tắt, và không có gì độc lập kiểm tra rằng hai thứ
khớp nhau. Nếu mã phục vụ giao diện đó bị thay — bởi một quy trình build bị chiếm, một
CDN bị chiếm quyền, một thư viện phụ thuộc độc hại, một thông tin đăng nhập triển khai bị
đánh cắp — thì bản tóm tắt sẽ thành bất cứ thứ gì kẻ tấn công muốn, còn chữ ký của bạn là
thật.

Đó là rủi ro mà thiết kế ký của Vela nhắm tới. Không phải lừa đảo. Không phải lộ khóa.
**Mà là một màn hình ký đang nói dối bạn.**

## Vela làm gì trước chuyện đó

**Ký minh bạch, tới tận calldata.** Mọi giao dịch đều được giải mã thành ý định con người
đọc được trước khi bạn duyệt — số tiền, người nhận, lệnh gọi thực sự làm gì
([ERC-7730](/vi/docs/clear-signing)). Một lệnh gọi chúng tôi không giải mã được sẽ **bị
đánh dấu là không giải mã được**, chứ không lặng lẽ hiện ra như thể mọi thứ vẫn ổn. Dữ
liệu trong vụ Bybit là một `delegatecall` tráo địa chỉ implementation; đó đúng là loại
việc phải khiến người ký khựng lại ngay, và giấu nó sau một bản tóm tắt thân thiện chính
là lý do họ đã không khựng lại.

Có hai điều cần nói cho chính xác. Một dApp không thể trực tiếp yêu cầu Vela thực
hiện `delegatecall` — các yêu cầu mà một trang web gửi được chỉ tạo ra lệnh gọi thông
thường — nên chính dữ liệu kiểu Bybit không thể đi vào bằng đường đó. Còn một trang web
yêu cầu lệnh gọi từ Safe của bạn tới chính nó thì **bị từ chối, chứ không chỉ được giải
mã**: `enableModule`, `addOwnerWithThreshold`, `swapOwner`, `setFallbackHandler`,
`setGuard` và những lệnh còn lại trong họ đó đều bị chặn khi đích đến là chính ví của
bạn, kể cả khi nằm trong một giao dịch gộp hay trong một `MultiSend`; mọi nhánh mang
`delegatecall` dù nhắm tới đâu, và chữ ký dữ liệu có cấu trúc `SafeTx`, cũng vậy. Ví nói
rõ nó đã từ chối lệnh gọi nào và không đưa ra thứ gì để ký. Bất kỳ lệnh nào trong số đó,
chỉ cần ký một lần, cũng sẽ trao tài khoản đi trọn vẹn như dữ liệu trong vụ Bybit — một
mô-đun đã được bật sau đó có thể tự chạy `delegatecall` của riêng nó. Và nếu chính mã của
Vela bị thay, như mã của `Safe{Wallet}` đã bị, thì phần giải mã cũng sẽ là của kẻ tấn
công — đó là lý do có điểm tiếp theo.

**Một đường độc lập có thể kiểm tra lại giao diện.** Vela đã làm một
[trang ký](/vi/docs/clear-signing-self-host) không cần build, không có thư viện phụ
thuộc, tự giải mã yêu cầu và tự thực hiện chữ ký WebAuthn — một thư mục tệp tĩnh duy nhất
mà bạn đọc được từ đầu đến cuối, tự phục vụ, hoặc nạp như một tiện ích trình duyệt. Mục
đích của nó là làm ý kiến thứ hai không dùng chung chuỗi cung ứng với ứng dụng chính.
*Tình trạng: đã làm xong và đã thử nghiệm; chưa phát hành, và chưa có ứng dụng Vela nào
gửi yêu cầu tới nó.* Trang này sẽ nói rõ khi điều đó thay đổi.

**Chúng tôi không có vai trò quản trị nào để bị mất.** Tài khoản Vela là
[Safe v1.4.1 nguyên bản](/vi/docs/account-contract), và Vela không giữ vai trò đặc quyền
nào trên chúng — không có khóa quản trị, không có đường nâng cấp nào của chúng tôi mà
chúng tôi có thể bị ép buộc hoặc bị chiếm quyền để dùng. Nhưng cần nói rõ điều này *không*
loại bỏ được gì: cơ chế mà kẻ tấn công Bybit đã dùng — một `delegatecall` do chủ sở hữu ký
để viết lại phần triển khai của tài khoản — vẫn tồn tại trong mọi Safe, kể cả của Vela
(chính các giao dịch gộp của Vela cũng dùng `delegatecall` vào MultiSend của Safe). Nó cần
một chữ ký hợp lệ từ một trong các khóa của bạn. Thứ bảo vệ bạn khỏi bị dụ ký như vậy là
phần giải mã ở trên và lớp kiểm tra độc lập.

**Xác nhận mới cho mỗi chữ ký.** Mọi chữ ký đều cần chính khóa của bạn xác nhận — Face
ID, vân tay, mã PIN, hoặc chạm và nhập PIN trên khóa bảo mật. Không có khóa phiên tồn tại
lâu dài, nên không có khoảng thời gian nào để một thứ gì đó ký thay bạn khi bạn không có
mặt.

**Tự triển khai là chốt chặn cuối.** Các ứng dụng và dịch vụ phía sau đều là mã nguồn mở.
Nếu bạn hoàn toàn không muốn tin quy trình build của chúng tôi, hãy tự biên dịch tiện ích
hoặc một ứng dụng và tự chạy những dịch vụ bạn cần —
[hướng dẫn tự triển khai](/vi/docs/self-hosting) đi qua từng bước. Cách đó đưa quy trình
build của chúng tôi ra khỏi chuỗi tin cậy; bạn vẫn phải tin phần mã mà bạn biên dịch, nên
hãy đọc nó.

## Vela không tuyên bố điều gì

Giao diện của Vela hoàn toàn có thể bị chiếm theo đúng cách giao diện `Safe{Wallet}` đã
bị. Mã của chúng tôi chưa được kiểm toán. Nói khác đi thì chính là kiểu bảo đảm mà vụ này
lẽ ra phải chấm dứt.

Điều thiết kế cố làm là thu hẹp con đường đó: làm dữ liệu trở nên đọc được thay vì khó
hiểu, không giữ vai trò quản trị nào có thể bị lạm dụng nhân danh bạn, và cho bạn một cách
kiểm chứng bằng thứ không phải là chúng tôi. Tóm lại một cách thật thà: **lớp tấn công này
được giảm nhẹ bằng thiết kế, chứ không bị loại bỏ**, và những phần có thể khiến nó chắc
hơn nữa đang được liệt kê, còn dang dở, trong
[kiểm toán & vấn đề đã biết](/vi/docs/security-audits).

## Nguồn

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
