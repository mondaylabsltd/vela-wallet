---
title: Ký minh bạch
description: Vela giải mã giao dịch thành ngôn ngữ đời thường trước khi bạn duyệt — ý định, số tiền, địa chỉ và rủi ro — thay vì hex khó hiểu. Khi không giải mã được, nó cảnh báo chứ không giả vờ.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Ký minh bạch

Phần lớn ví bảo bạn duyệt một bức tường số thập lục phân rồi cầu may. "Ký mù" — duyệt
những lệnh gọi bạn thực ra không đọc được — đứng sau một tỷ lệ lớn các vụ ví bị rút
sạch. Câu trả lời của Vela là **ký minh bạch**: trước khi bạn ký, giao dịch được giải
mã thành thứ bạn hiểu được.

## Bạn nhìn thấy gì

Thay vì calldata thô, Vela hiện:

- **Ý định** — giao dịch làm gì: *Gửi*, *Duyệt*, *Hoán đổi*, v.v.
- **Phần cốt lõi** — số tiền và các địa chỉ liên quan, với số token hiển thị theo đơn
  vị thật và người nhận được phân giải thành tên nếu có.
- **Chi tiết** — nonce, hạn chót và calldata thô, hiện ra khi bạn muốn xem chứ không
  dí vào mặt bạn.
- **Một chỉ báo rủi ro**, có màu, để thứ đáng sợ trông đúng là đáng sợ.

## Cách hoạt động (ERC-7730)

Vela giải mã cả **lệnh gọi hợp đồng** lẫn **dữ liệu có kiểu EIP-712** bằng các mô tả
[ERC-7730](https://github.com/LedgerHQ/clear-signing-erc7730-registry) — những định
nghĩa nhỏ, chia sẻ được, nói rõ các hàm của một hợp đồng nghĩa là gì.

- Khi có **mô tả riêng cho hợp đồng**, giao dịch được đánh dấu **đã xác minh** và gắn
  tên hợp đồng.
- Khi không có, Vela lùi về **các mô tả chuẩn** cho những dạng phổ biến — token
  ERC-20, NFT ERC-721, vault ERC-4626 và permit ERC-2612 — nên phần lớn thao tác
  thường ngày vẫn giải mã được.

Số lượng token được định dạng theo **số thập phân thật trên chuỗi** của token đó. Vela
không bao giờ mặc định coi là 18; nếu không xác nhận được số thập phân, nó vẫn hiện
giá trị nhưng **đánh dấu là chưa xác minh** thay vì đoán bừa.

## Mức rủi ro

Mỗi giao dịch đã giải mã đều có một mức rủi ro để những mẫu nguy hiểm nổi lên:

- **Thận trọng** cho các lệnh duyệt và permit — bạn đang trao quyền chi tiêu.
- **Nguy hiểm** cho những thứ thật sự rủi ro, như **duyệt chi token không giới hạn**.
- Rủi ro thấp hơn cho các thao tác thường ngày như stake hay gửi vào.

<Callout type="warning" title="Duyệt chi không giới hạn bị chặn">
Một lệnh "approve" trao hạn mức không giới hạn là một trong những cách phổ biến nhất
khiến tiền bị rút sạch về sau. Vela làm nhiều hơn là đánh dấu: nó viết lại yêu cầu
thành một số hữu hạn do bạn chọn, và một lần kiểm tra cuối trước khi gửi sẽ từ chối
mọi lệnh duyệt vẫn còn không giới hạn. Lớp bảo vệ đó đọc thẳng calldata thô, nên nó
hoạt động cả khi hợp đồng chẳng có mô tả nào.
</Callout>

## Khi Vela không giải mã được một lệnh gọi

Thật thà quan trọng hơn một màn hình sạch sẽ. Khi không có mô tả ERC-7730 nhưng hàm đó
xuất hiện trong cơ sở dữ liệu selector công khai, Vela giải mã chung chung và gắn nhãn
**cố hết sức** — đã giải mã nhưng chưa xác minh — dưới một dải cảnh báo. Nếu đến cả
thế cũng thất bại, hoặc Vela chỉ giải mã được một phần giao dịch, nó **không** giả vờ
đã hiểu.

<Callout type="danger" title="Cảnh báo ký mù rõ ràng">
Nếu một lệnh gọi không giải mã được, Vela hiện cảnh báo ký mù rõ ràng thay vì một bản
tóm tắt thân thiện giả tạo. Nếu nó chỉ phân giải được một số trường, nó nói cho bạn
biết rằng bức tranh còn thiếu và giữ mức rủi ro ở mức cao. Bạn luôn biết Vela thực sự
đọc được bao nhiêu phần của thứ bạn đang ký.
</Callout>

## Vì sao điều này quan trọng

Tự quản nghĩa là không ai đảo ngược được một giao dịch tồi giúp bạn. Hàng phòng thủ
không phải là bộ phận hỗ trợ — mà là việc hiểu thứ bạn duyệt **trước khi** duyệt. Ký
minh bạch biến "hãy tin cái khối khó hiểu này" thành "đây chính xác là việc nó làm".
Xem [whitepaper](/vi/docs/whitepaper) để biết nó nằm ở đâu trong mô hình bảo mật tổng
thể của Vela.
