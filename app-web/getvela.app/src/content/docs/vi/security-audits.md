---
title: Kiểm toán & vấn đề đã biết
description: Mọi hợp đồng trên chuỗi mà Vela phụ thuộc vào, ai đã kiểm toán, phiên bản được kiểm toán có khớp với phiên bản thật sự đang chạy hay không, và những gì hoàn toàn chưa được kiểm toán.
---

"Đã kiểm toán" là một khẳng định về một phiên bản cụ thể của một đoạn mã cụ thể, nên
trang này không vẫy vẫy cái từ đó — nó dẫn đúng báo cáo, đúng địa chỉ triển khai, và
đúng những khác biệt giữa phiên bản được kiểm toán với phiên bản đang chạy. Nó cũng
liệt kê những gì _chưa_ được kiểm toán, vì danh sách đó gánh trọng lượng không kém
danh sách đầu.

Rà soát lần cuối: tháng 8/2026. Nếu bạn thấy chỗ nào sai ở đây, hãy báo và chúng tôi
sẽ sửa.

## Đường đi của tiền

Có bốn lớp hợp đồng có thể chạm vào tiền của bạn. Cả bốn đều là hợp đồng của bên thứ
ba, có báo cáo kiểm toán công khai, và ở mỗi trường hợp địa chỉ đang chạy chính là bản
triển khai chuẩn chính thức.

### Safe v1.4.1 — bản thân tài khoản

Ví của bạn là một proxy [Safe](https://github.com/safe-global/safe-smart-account):
singleton SafeL2, proxy factory, fallback handler tương thích, và MultiSend để gộp lô.

[Ackee Blockchain đã kiểm toán Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(báo cáo cuối tháng 3/2023): 11 phát hiện, không cái nào ở mức nghiêm trọng hay cao.
Bản v1.4.1 chúng tôi triển khai khác bản v1.4.0 được kiểm toán đúng một sửa đổi một
dòng cho tương thích ERC-4337
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)). Logic của
MultiSend không đổi kể từ
[bản v1.3.0 do G0 Group kiểm toán](https://github.com/safe-global/safe-smart-account/tree/main/docs).
Mọi địa chỉ đều khớp với các bản triển khai chuẩn trong
[safe-deployments](https://github.com/safe-global/safe-deployments), và các hợp đồng
nằm trong
[chương trình thưởng lỗi của Safe Foundation](https://docs.safefoundation.org/security/bug-bounty)
(tới 1.000.000 USD cho phát hiện nghiêm trọng).

Một thứ mà kiểm toán không bao phủ: vụ Bybit năm 2025. Cuộc tấn công đó chiếm dây
chuyền build của giao diện web chính thức của Safe, chứ không phải các hợp đồng —
[kết luận điều tra chính thức](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
không tìm thấy lỗ hổng nào trong hợp đồng thông minh của Safe. Chúng tôi đọc nó như một
bài học về lớp web và vận hành, cũng chính là lớp mà bạn nên soi chúng tôi.

### Safe4337Module v0.3.0 — bộ chuyển đổi ERC-4337

Triển khai tại `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`, địa chỉ chuẩn của v0.3.0
(Sourcify khớp chính xác — bytecode trên chuỗi chính là mã đã được kiểm toán).
[Ackee Blockchain kiểm toán](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)
(báo cáo cuối tháng 3/2024), không còn phát hiện chưa xử lý nào trên mức thông tin. Tổ
hợp v0.3.0 + EntryPoint v0.7 + Safe ≥1.4.1 mà chúng tôi dùng đúng là cấu hình mà bản
kiểm toán và ghi chú phát hành mô tả.

Lịch sử của module có một vấn đề từng được công bố: v0.1.0 (2023) không ký `initCode`
và `paymasterAndData`, một lối quấy rối tiêu tốn gas. Nó đã được
[sửa ở v0.2.0](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module)
và v0.1.0 chưa bao giờ rời khỏi testnet. Chúng tôi dùng v0.3.0, vốn thừa hưởng bản sửa
đó.

### SafeWebAuthnSharedSigner v0.2.1 — bộ ký passkey

Triển khai tại `0x94a4F6affBd8975951142c3999aEAB7ecee555c2`, địa chỉ chuẩn của v0.2.1
(giống nhau trên mọi chuỗi nhờ singleton factory của Safe).

"Shared" (dùng chung) nghĩa là gì — và không nghĩa là gì: thứ được dùng chung là _bản
triển khai hợp đồng_, đúng như singleton của Safe được dùng chung. Khóa của bạn thì
không. Mỗi Safe gọi `configure()` bằng delegatecall và lưu khóa công khai P-256 của
riêng nó trong bộ nhớ của chính nó. Một thể hiện của bộ ký đại diện đúng một passkey
cho một Safe, và Safe của người khác không dùng được khóa của bạn.

Ở đây phiên bản rất quan trọng. Bản kiểm toán v0.2.0
[nói thẳng](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.0/audit.md)
rằng bộ ký dùng chung nằm ngoài phạm vi — lúc đó hợp đồng còn chưa tồn tại. Những bản
kiểm toán bao phủ thứ chúng tôi triển khai là các bản của v0.2.1:
[một cuộc thi kiểm toán của Hats Finance](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
(tháng 6–7/2024: không phát hiện cao, không phát hiện trung bình, ba phát hiện thấp —
đều đã sửa) cùng
[một lượt rà soát của Certora trên commit phát hành](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)
không có phát hiện mới. Từ khi phát hành chưa có lỗ hổng nào ở mức hợp đồng được công
bố; các hợp đồng passkey nằm trong phạm vi chương trình thưởng lỗi của Safe Foundation.

Tài liệu của chính Safe khuyến nghị đi kèm quyền sở hữu bằng passkey với một đường khôi
phục, thay vì coi một chứng danh duy nhất là chiếc khóa duy nhất của tài khoản. Vela xử
lý việc này thế nào thì có ở [Khôi phục & đăng nhập](/vi/docs/recovery).

Việc xác minh P-256 trên chuỗi dùng thẳng precompile RIP-7212, không có bộ xác minh dự
phòng viết bằng Solidity. Trước khi bật bất kỳ mạng nào, ứng dụng thử precompile bằng
một chữ ký thật và từ chối mạng đó nếu xác minh thất bại. Hai lưu ý thật thà: đặc tả
RIP-7212 gốc có những khiếm khuyết ở trường hợp biên mà
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) được viết ra để sửa (chúng không
ảnh hưởng tới các chữ ký WebAuthn đúng dạng), và một phép thử không thể bắt được mọi
cách mà phần cài đặt của một chuỗi có thể lệch đi trong những bối cảnh thực thi bất
thường.

### EntryPoint v0.7 — điểm vào của ERC-4337

Triển khai tại `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, là
[bản triển khai chuẩn v0.7.0](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[OpenZeppelin kiểm toán](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
(do Ethereum Foundation đặt hàng, tháng 1/2024): không phát hiện nghiêm trọng, không
phát hiện cao, năm phát hiện trung bình, tất cả đã xử lý — và commit được kiểm toán
chính là bản phát hành đang chạy. EntryPoint v0.7.0 nằm trong
[chương trình thưởng lỗi ERC-4337](https://docs.erc4337.io/community/bug-bounty) của
Ethereum Foundation (tới 250.000 USD).

## Những vấn đề đã biết mà chúng tôi đang theo dõi

### Lối quấy rối ở EntryPoint

Tháng 2/2026, các nhà nghiên cứu bảo mật tại Trust Security
[công bố](https://erc4337.substack.com/p/improving-useroperation-execution)
một lối quấy rối và kiểm duyệt ảnh hưởng tới mọi EntryPoint trước v0.9, kể cả bản v0.7
chúng tôi dùng. Kẻ tấn công chặn được một UserOperation đã ký trước khi nó lên khối có
thể thực thi nó bên trong một khung gọi do chính hắn kiểm soát và ép phần thực thi bên
trong revert — thao tác thất bại nhưng gas vẫn bị tính. Ethereum Foundation đã trả
50.000 USD tiền thưởng cho phát hiện này; họ xếp nó là lối kiểm duyệt/quấy rối chứ
không phải lối trộm tiền, và nó chưa bao giờ bị khai thác.

Nó làm được gì: đốt một khoản phí và làm chậm một giao dịch. Nó không làm được gì: trộm
tiền hay giả mạo chữ ký. Mức phơi nhiễm của Vela hẹp, vì UserOperation đi thẳng tới
relay chứ không qua mempool công khai, nên chẳng có mấy cơ hội để chặn — và trường hợp
xấu nhất bị giới hạn bởi đúng khoản phí bạn đã đồng ý. Bản sửa chỉ có ở EntryPoint v0.9
(tháng 11/2025); bản v0.7 tự nó không vá được. Chúng tôi dự kiến chuyển sang khi phần
còn lại của hệ thống — đặc biệt là dòng module 4337 của Safe — có hỗ trợ v0.9, và sẽ
ghi lại ở đây khi điều đó xảy ra.

## Những gì chưa được kiểm toán

- **Các hợp đồng của chính Vela.** Hai hợp đồng nhỏ do chúng tôi tự viết, triển khai
  trên Gnosis:
  [chỉ mục khóa công khai passkey](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools)
  (một sổ chỉ-thêm giúp các thiết bị của bạn tìm ra khóa công khai) và hợp đồng phụ trợ
  gộp lô của nó. Chúng chưa được kiểm toán. Theo cấu tạo, chúng không giữ tiền, không
  có chủ sở hữu và không nâng cấp được — đó là lớp tra cứu, không phải lớp cấp quyền.
  Quyền chi tiêu luôn đến từ passkey được cấu hình bên trong Safe của bạn. Sự cố tệ nhất
  có thể hình dung là quấy rối (ai đó chiếm chỗ một mục trong chỉ mục), khiến việc khôi
  phục bất tiện hơn chứ không chuyển được tiền. Một hợp đồng chia tiền thanh toán gas từ
  thiết kế phí cũ nay không còn nằm trong luồng giao dịch.
- **Multicall3.** Chính README của nó
  [nói thẳng](https://github.com/mds1/multicall3): "This contract is unaudited." Chúng
  tôi dùng nó đúng theo cách mà tác giả mô tả là an toàn — gộp lô các lệnh gọi chỉ đọc
  để lấy số dư, metadata token và báo giá. Vela không bao giờ cấp quyền cho nó và nó
  không bao giờ giữ tiền. Hậu quả xấu nhất của một lỗi là đọc ra số liệu sai.
- **Bộ triển khai CREATE2.**
  [Proxy triển khai tất định của Arachnid](https://github.com/Arachnid/deterministic-deployment-proxy)
  là bộ triển khai phi trạng thái tiêu chuẩn của hệ sinh thái; nó không có kiểm toán
  chính thức. Các bước kiểm tra mạng của chúng tôi sẽ thất bại theo hướng an toàn nếu
  nó thiếu hoặc bị sửa trên một chuỗi.
- **Tempo và pathUSD.** Tempo, một trong mười hai mạng có sẵn của chúng tôi, không có
  đồng gốc; gas ở đó thanh toán bằng stablecoin pathUSD. Tính tới tháng 8/2026, cả giao
  thức lõi của Tempo lẫn pathUSD đều chưa có kiểm toán bảo mật công bố hay chương trình
  thưởng lỗi, và một
  [đánh giá tài sản thế chấp độc lập của DefiLlama](https://artifacts.llama.fi/md-exports/pathusd-collateral-assessment-april2026-1776332825042.md)
  (tháng 4/2026) xếp pathUSD ở mức rủi ro cao. Đây là rủi ro ở tầng chuỗi mà không ví
  nào giảm nhẹ được: tiền bạn giữ trên Tempo, và việc thanh toán gas ở đó, đều thừa
  hưởng nó. Hãy coi Tempo là chuỗi mới nhất và ít được kiểm chứng nhất trong danh sách
  và cân đối số dư cho phù hợp. Chúng tôi sẽ cập nhật mục này khi có kiểm toán được công
  bố.
- **Bản thân Vela.** Ứng dụng và các dịch vụ phía sau của chúng tôi chưa qua kiểm toán
  bên thứ ba. Đó là lưu ý lớn nhất trên trang này, chúng tôi ghi nó ngay ở đầu trang
  web, và chi tiết thật thà nằm ở [Vela đang ở giai đoạn alpha](/blog/vela-is-in-alpha).
  Hãy bắt đầu với số nhỏ. Hãy đọc mã nguồn.

## Tự kiểm tra

Mọi địa chỉ ở trên đều là bản triển khai công khai chuẩn mà bạn đối chiếu được với các
sổ đăng ký chính thức —
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
và [ghi chú phát hành EntryPoint](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| Hợp đồng | Địa chỉ |
| ----------------------------------- | -------------------------------------------- |
| SafeL2 singleton v1.4.1 | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1 | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| CompatibilityFallbackHandler v1.4.1 | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| MultiSend v1.4.1 | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| SafeModuleSetup v0.3.0 | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0 | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1 | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| EntryPoint v0.7 | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3 | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Chỉ mục khóa công khai passkey (Gnosis) | `0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3` |
