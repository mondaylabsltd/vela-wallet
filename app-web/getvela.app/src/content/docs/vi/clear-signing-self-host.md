---
title: Trusted Signer
description: "Một trang chỉ gồm một tệp tại sign.getvela.app, tự giải mã yêu cầu và tự ký bằng passkey của bạn — nó kiểm tra những gì, ứng dụng nào dùng nó, và cách build lại hoặc chạy bản sao của riêng bạn."
source: 43a1af6ffdcd
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Trusted Signer

Vela giải mã mọi giao dịch trước khi bạn duyệt, và việc giải mã đó được làm một cách trung
thực — nhưng nó do chính ứng dụng đã dựng giao dịch thực hiện. Nếu ứng dụng, hoặc con đường
nó đến tay bạn, bị can thiệp, nó có thể cho bạn xem một thứ và ký một thứ khác. Đó chính xác
là điều đã xảy ra với [Bybit](/vi/docs/bybit-attack).

Trusted Signer tồn tại để tách việc đó làm hai: ứng dụng chỉ chuyển yêu cầu sang, còn việc
kiểm tra và ký diễn ra trên một trang riêng — một trang bạn có thể đọc từ đầu đến cuối, build
lại giống hệt đến từng byte, hoặc tự chạy.

## Nó chạy ở đâu

Trang chính thức được phục vụ từ **sign.getvela.app**. Các ứng dụng máy tính (macOS, Windows,
Linux), iPhone và Android có thể gửi yêu cầu tới nó: ứng dụng mở trang trong một thẻ trình
duyệt với yêu cầu nằm trong đường liên kết, bạn kiểm tra và ký bằng passkey ngay tại đó, rồi
trang trả chữ ký về cho ứng dụng qua một liên kết `velawallet://`. Ví web không dùng được nó.

Đây là tùy chọn, bạn phải tự chọn dùng. Bạn chọn nó làm cách ký khi tạo ví hoặc đăng nhập, và
từ đó trở đi mọi chữ ký của ví đó trên thiết bị đó đều đi qua nó. Nó cũng có thể tạo khóa cho
ví. Trên `sign.getvela.app`, nó dùng chính các passkey `getvela.app` mà các ứng dụng dùng.

<Callout type="info" title="Đã thử đến đâu">
Các lần chạy trọn vẹn từ đầu đến cuối với trang đã phát hành đã được ghi lại trên: Android, và
Windows 11 (tạo ví và đăng nhập). Các ứng dụng macOS, Linux và iPhone dùng cùng cách kết nối;
chưa ứng dụng nào trong số đó có một lần chạy trọn vẹn được ghi lại.
</Callout>

## Nó làm gì trước khi ký

- **Nó tự giải mã yêu cầu.** Lệnh gọi làm gì, cho ai, bao nhiêu, đọc ra từ calldata — kể cả
  những lệnh gọi lồng bên trong một giao dịch gộp.
- **Nó chỉ ký digest do chính nó tính.** Các digest EIP-191, EIP-712, SafeOp và SafeMessage
  được tính ngay trong trang, không bao giờ lấy từ bên yêu cầu; các bài kiểm thử đối chiếu
  digest SafeOp và SafeMessage với `vela-core`, chính đoạn mã mà ví dùng, và ứng dụng từ chối
  chữ ký trên bất kỳ digest nào khác với digest do chính ứng dụng tính.
- **Nó kiểm tra giao dịch đúng là giao dịch đã được yêu cầu.** Lệnh gọi mà trang web yêu cầu
  phải thực sự nằm bên trong thao tác đang được ký, nếu không trang sẽ từ chối.
- **Nó nói rõ khi một lệnh cấp quyền là không giới hạn.** Nó không thể thay đổi số lượng —
  nó ký đúng những byte đã đến hoặc không ký gì cả — nên một lệnh cấp quyền hay permit không
  giới hạn (từ 2^128 trở lên trên trang này) được hiện màu đỏ kèm lý do đó và có thể được ký
  nguyên trạng; hạn mức trên chuỗi được chọn trên màn hình cấp quyền của chính ví, trước khi
  yêu cầu đến được đây. Lệnh cấp quyền cho cả một bộ sưu tập NFT thì bị từ chối.
- **Nó từ chối những gì nó không thể đứng ra bảo đảm:** `eth_sign`, một phương thức nó không
  biết, một token được gửi tới chính hợp đồng của token đó, một thao tác nó không đọc được, và
  một lần đăng nhập có thử thách (challenge) do bên yêu cầu cung cấp.
- **Nó từ chối những gì sẽ trao tài khoản của bạn cho người khác,** theo đúng quy tắc
  mà các ứng dụng áp dụng: một lệnh gọi từ tài khoản của bạn tới một trong các hàm chủ
  sở hữu, module, guard hoặc fallback của chính nó, kể cả bên trong một lô; một
  `delegatecall`, trừ khi vào hợp đồng MultiSend của Safe, nơi gom các lệnh gọi của một
  thao tác; và một chữ ký `SafeTx`. Nó kiểm tra mọi lệnh gọi trong thao tác mà ứng dụng
  đã lắp ráp, không chỉ những lệnh gọi mà trang web yêu cầu.
- **Nó hiện địa chỉ của tài khoản và một identicon được tính ngay trên trang.** Người nhận và
  hợp đồng không bao giờ được đặt tên theo yêu cầu — chỉ bảng đã được rà soát của chính trang
  mới có thể đặt tên cho một hợp đồng. Tên của chính tài khoản, do ứng dụng gửi để bạn chọn
  đúng passkey, được hiện bên cạnh địa chỉ.
- **Nó yêu cầu xác minh người dùng** (vân tay, khuôn mặt hoặc mã PIN) cho mọi chữ ký.

## Những gì nó cố ý không có

- **Không có trình chỉnh sửa.** Yêu cầu đã cố định khi đến nơi: bạn ký hoặc không ký. Một bộ
  chọn phí hay một trình chỉnh sửa hạn mức sẽ viết lại calldata, chính là căn bệnh mà trang này
  sinh ra để ngăn.
- **Không truy cập mạng.** Trang là một tệp duy nhất, và chính sách bảo mật nội dung
  (`default-src 'none'`) của nó nằm ngay trong các byte của tệp, nên nó không thể tải bất cứ
  thứ gì, mở kết nối hay nạp hình ảnh. Thứ duy nhất rời khỏi trang là câu trả lời của nó, khi
  nó đi theo liên kết callback trong yêu cầu (`velawallet://` khi yêu cầu đến từ một ứng dụng
  Vela). Logo token được vẽ bằng chữ cái.

## Ứng dụng kiểm tra lại những gì

Ứng dụng cũng không tin trang. Nó chỉ chấp nhận một chữ ký khi thử thách (challenge) được ký
là digest **do ứng dụng tính**, bước xác minh người dùng đã được thực hiện, khóa là một trong
các khóa của ví bạn, và chữ ký P-256 được xác minh hợp lệ với khóa đó.

## Mọi phiên bản đã phát hành đều kiểm tra được

Mỗi phiên bản được build từ `app-web/trusted-signer/src/` thành một tệp duy nhất, theo cách
tái lập được — Bun và Node cho ra cùng các byte — và được phát hành tại địa chỉ riêng của nó,
`sign.getvela.app/b/<sha256>/sign.html`, bên cạnh mọi phiên bản trước đó. Danh sách nằm ở
`sign.getvela.app/index.json`.

```sh
cd app-web/trusted-signer
node samples/build-single.mjs --check   # rebuilds a version listed in dist/
curl -sL https://sign.getvela.app/b/<sha256>/sign.html | shasum -a 256
```

Khi khởi động, ứng dụng máy tính tải phiên bản đã phát hành mà nó sẽ mở, băm nó và so sánh với
các phiên bản được tích hợp sẵn trong ứng dụng. Kết quả chỉ được ghi vào nhật ký, và một trang
không khớp vẫn được mở như thường. Các ứng dụng điện thoại chưa kiểm tra việc này.

## Chạy bản sao của riêng bạn

Cài đặt lưu địa chỉ của trang mà các ứng dụng của bạn mở, nên bạn có thể trỏ nó tới bản triển
khai của riêng mình: bất kỳ địa chỉ HTTPS nào, hoặc `localhost` để thử nghiệm. Build bằng
`bun samples/build-single.mjs` (hoặc `node`) rồi chép `dist/` lên máy chủ của bạn.

Một bản sao trên tên miền của riêng bạn ký bằng các passkey được tạo cho **chính** tên miền đó,
không phải bằng các passkey `getvela.app` — nên đó là cách để tạo và dùng một ví có khóa nằm
dưới tên miền của bạn, không phải cách để ký cho một ví `getvela.app` đã có. Mọi khóa của một
ví đều dùng chung một tên miền.

Mã nguồn, cùng các script build và kiểm tra nó, nằm trong `app-web/trusted-signer/`.
