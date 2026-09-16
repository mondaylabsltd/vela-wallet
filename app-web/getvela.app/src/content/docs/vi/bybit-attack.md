---
title: Vụ tấn công Bybit
description: "Tháng 2/2025 Bybit mất khoảng 1,5 tỷ USD. Hợp đồng Safe không hỏng — giao diện mới hỏng. Trang này giải thích lối tấn công đó, và thiết kế của Vela chặn nó ở đâu."
---

# Vụ tấn công Bybit và lối nó đã đi

Ngày 21/2/2025, Bybit mất khoảng **1,5 tỷ USD** từ một ví lạnh đa chữ ký Safe. Đó là
vụ trộm lớn nhất lịch sử ngành, và đáng đọc kỹ, vì gần như mọi thứ trong đó đều *đúng*
trừ một điều.

## Chuyện đã xảy ra

Bản ngắn, theo các báo cáo điều tra công khai:

1. Kẻ tấn công chiếm được **máy của một lập trình viên `Safe{Wallet}`** và chèn
   JavaScript độc hại vào bucket AWS S3 đang phục vụ giao diện `Safe{Wallet}`. Mã được
   đưa vào ngày 19/2 và kích hoạt ngày 21/2, nhắm thẳng vào chiếc Safe cụ thể của
   Bybit.
2. Những người ký của Bybit mở giao diện và xem một giao dịch trông rất bình thường.
3. Dữ liệu thật sự gửi tới **ví cứng** của họ không phải giao dịch đó. Đó là một
   `delegatecall` ghi đè `masterCopy` của proxy Safe — ô nhớ số 0 — thay toàn bộ phần
   cài đặt của tài khoản bằng cài đặt của kẻ tấn công.
4. Những người ký đã duyệt. Chữ ký hợp lệ. Hợp đồng làm đúng những gì được bảo.

Hành vi này được công khai quy cho các nhóm liên quan tới Triều Tiên (FBI gọi tên cụm
TraderTraitor).

## Cái gì *không* hỏng

- **Không phải hợp đồng Safe.** Chúng thực thi một chỉ dẫn đã ký hợp lệ. Không lỗi nào
  của Safe bị khai thác.
- **Không phải mật mã học.** Mọi chữ ký đều thật.
- **Không phải ví cứng.** Thiết bị Ledger có trong quy trình và vẫn ký — vì ví cứng
  hiện cho bạn cái nó được đưa, và cái nó được đưa là dữ liệu độc hại. Một thiết bị
  không giải mã nổi `delegatecall` thành thứ con người đánh giá được thì nó bảo vệ
  *khóa*, chứ không bảo vệ *quyết định*.

Cái hỏng là giả định nằm dưới mọi giao diện ví: **rằng màn hình mô tả giao dịch và
những byte đang được ký là cùng một thứ.**

## Vì sao đây là trường hợp phổ quát chứ không phải tai nạn hy hữu

Mọi chữ ký bạn từng tạo ra trong một ví web đều dựa trên giả định ấy. Giao diện dựng
dữ liệu, giao diện vẽ bản tóm tắt, và không có gì độc lập kiểm tra rằng hai thứ khớp
nhau. Nếu mã phục vụ giao diện đó bị thay — bởi một dây chuyền build bị chiếm, một CDN
bị cướp, một thư viện độc hại, một chứng danh triển khai bị trộm — thì bản tóm tắt trở
thành bất cứ thứ gì kẻ tấn công muốn, còn chữ ký của bạn là thật.

Đó là rủi ro mà thiết kế ký của Vela nhắm tới. Không phải lừa đảo. Không phải lộ khóa.
**Mà là một màn hình ký đang nói dối bạn.**

## Vela làm gì trước chuyện đó

**Ký minh bạch, tới tận calldata.** Mọi giao dịch đều được giải mã thành ý định con
người đọc được trước khi bạn duyệt — số tiền, người nhận, lệnh gọi thật sự làm gì
([ERC-7730](/vi/docs/clear-signing)). Một lệnh gọi chúng tôi không giải mã được sẽ
**bị đánh dấu là không giải mã được**, chứ không lặng lẽ vẽ ra như thể mọi thứ ổn. Dữ
liệu của vụ Bybit là một `delegatecall` tráo địa chỉ implementation; đó đúng là dạng
việc phải làm người ký khựng lại, và giấu nó sau một bản tóm tắt thân thiện chính là
cách nó đã không khựng lại.

**Một đường độc lập có thể kiểm tra giao diện.** Vela đang làm một trang ký không cần
build, không phụ thuộc thư viện, tự vẽ ý định và tự thực hiện chữ ký WebAuthn — một
thư mục tệp tĩnh duy nhất mà bạn đọc được từ đầu đến cuối, tự phục vụ, hoặc chạy như
một tiện ích trình duyệt. Mục đích duy nhất của nó là làm ý kiến thứ hai không dùng
chung chuỗi cung ứng với ứng dụng chính. *Trạng thái: đã làm xong và đã thử, chưa
triển khai.* Khi phát hành, nó là tùy chọn, và trang này sẽ nói rõ khi điều đó thay
đổi.

**Không có hợp đồng nào chúng tôi nâng cấp được.** Dữ liệu tấn công Bybit hoạt động
bằng cách thay phần cài đặt của tài khoản. Tài khoản Vela là
[Safe v1.4.1 không sửa đổi](/vi/docs/account-contract) và Vela không giữ vai trò đặc
quyền nào trên chúng — không khóa quản trị, không đường nâng cấp nào chúng tôi có thể
bị ép hoặc bị chiếm để dùng.

**Sinh trắc học mới cho mỗi chữ ký.** Không có khóa phiên sống lâu, nên không có
khoảng thời gian nào để thứ gì đó ký thay bạn khi bạn không có mặt.

**Tự vận hành là chốt chặn cuối.** Ứng dụng và mọi dịch vụ phía sau đều mã nguồn mở.
Nếu bạn không muốn tin dây chuyền build của chúng tôi chút nào, hãy chạy của riêng bạn
— đó là câu trả lời duy nhất cho lớp tấn công này mà không đòi hỏi phải tin ai.

## Vela không tuyên bố điều gì

Giao diện của Vela hoàn toàn có thể bị chiếm theo đúng cách `Safe{Wallet}` đã bị. Mã
của chúng tôi chưa được kiểm toán. Nói khác đi thì chính là kiểu bảo đảm mà vụ này lẽ
ra phải chấm dứt.

Điều thiết kế cố làm là thu hẹp lối đi: làm dữ liệu trở nên đọc được thay vì khó hiểu,
bỏ đi cái khả năng nâng cấp mà cuộc tấn công dựa vào, và cho bạn một cách kiểm chứng
bằng thứ không phải chúng tôi. Tóm tắt thật thà là **lớp tấn công này được giảm nhẹ
bằng thiết kế, chứ không bị loại bỏ**, và những phần sẽ làm nó chắc hơn nữa đang nằm,
còn dang dở, trong [kiểm toán & vấn đề đã biết](/vi/docs/security-audits).

## Nguồn

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
