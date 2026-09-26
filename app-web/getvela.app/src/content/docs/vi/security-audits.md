---
title: Kiểm toán & vấn đề đã biết
description: "Mọi hợp đồng Vela phụ thuộc vào, ai đã kiểm toán phiên bản nào, phiên bản được kiểm toán có phải là phiên bản đang được triển khai không, những phát hiện còn mở mà chúng tôi đang theo dõi, và những gì hoàn toàn chưa được kiểm toán."
source: a9c5e58e7ed3
---

"Đã kiểm toán" là một khẳng định về một đoạn mã cụ thể ở một phiên bản cụ thể, nên trang
này dẫn ra các báo cáo, các commit và các địa chỉ triển khai — và liệt kê cả những gì
**chưa** được kiểm toán, điều quan trọng không kém.

Rà soát lần cuối: 22/9/2026. Nếu bạn thấy chỗ nào sai, hãy báo cho chúng tôi và chúng tôi
sẽ sửa.

## Đường đi của tiền

Mọi hợp đồng có thể chạm tới tiền của bạn đều là bản triển khai chính thức của mã do bên
thứ ba viết, có báo cáo rà soát công khai.

### Safe v1.4.1 — chính tài khoản

Ví của bạn là một proxy [Safe](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
dùng singleton SafeL2 và SafeProxyFactory. Các giao dịch gộp đi qua MultiSend.

[Ackee Blockchain đã kiểm toán Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(báo cáo cuối ngày 16/3/2023, rà soát bản sửa ngày 28/3): 11 phát hiện, không cái nào ở
mức nghiêm trọng hay cao; hai phát hiện mức trung bình được ghi nhận chứ không sửa. Phạm
vi gồm SafeL2, SafeProxyFactory, CompatibilityFallbackHandler, MultiSendCallOnly và
SignMessageLib. v1.4.1 khác v1.4.0 đúng một dòng về chức năng, là một bản sửa tương thích
ERC-4337 trong phần thiết lập mô-đun
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)); Safe đã hỏi ý kiến
Ackee và kết luận không cần kiểm toán lại. Logic của MultiSend không đổi kể từ v1.3.0,
phiên bản đã được [G0 Group kiểm toán](https://github.com/safe-global/safe-smart-account/tree/main/docs).
Mọi địa chỉ đều khớp với [safe-deployments](https://github.com/safe-global/safe-deployments).
Các hợp đồng lõi nằm trong phạm vi
[chương trình thưởng lỗi của Safe Foundation](https://docs.safefoundation.org/security/bug-bounty),
với mức thưởng cao nhất lên tới 1.000.000 USD.

Sự cố Bybit năm 2025 không phải là một phát hiện về hợp đồng: kẻ tấn công đã sửa đổi phần
JavaScript được phục vụ cho giao diện web của Safe, và
[tuyên bố điều tra](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
của Safe không tìm thấy lỗ hổng nào trong các hợp đồng.
[Trang của chúng tôi về vụ này](/vi/docs/bybit-attack) giải thích vì sao cùng một lớp tấn
công đó liên quan tới mọi giao diện ví, kể cả của chúng tôi.

### Safe4337Module v0.3.0 — bộ chuyển đổi ERC-4337

Được triển khai tại `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` (Sourcify khớp chính xác),
và cũng được đặt làm fallback handler cho Safe của bạn. Đã qua ba lần rà soát —
[báo cáo ở đây](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md):

- **Ackee Blockchain**, báo cáo cuối tháng 3/2024: một cảnh báo (dùng trình tối ưu của
  trình biên dịch) đã được ghi nhận, không có gì ở mức cao hơn còn mở.
- **Certora**, tháng 8/2026: một phát hiện mức **trung bình**, đã được ghi nhận và **không
  được sửa** trong v0.3.0 — *thay đổi về quyền không vô hiệu hóa các UserOperation phía sau
  đã được xác thực trong cùng một bundle*. Xem "Vấn đề đã biết" bên dưới.
- **Nethermind**, tháng 8/2026: không có phát hiện nào.

SafeModuleSetup v0.3.0 (`0x2dd6…5b47`), hợp đồng bật mô-đun khi một ví được triển khai,
nằm trong phạm vi rà soát của Certora và Nethermind.

Lịch sử của mô-đun có một vấn đề từng được công bố: v0.1.0 không ký `initCode` và
`paymasterAndData`, một lỗ hổng cho phép quấy phá bằng cách tiêu hao gas,
[đã được sửa ở v0.2.0](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module);
theo Safe, v0.1.0 chưa từng được dùng ngoài testnet. Vela dùng v0.3.0 với EntryPoint v0.7
và Safe 1.4.1, đúng cấu hình mà bản phát hành của mô-đun mô tả.

### Mô-đun passkey của Safe v0.2.1 — các bộ ký

Khóa đầu tiên của bạn được xác minh bởi **SafeWebAuthnSharedSigner** tại
`0x94a4F6affBd8975951142c3999aEAB7ecee555c2`. "Shared" (dùng chung) nghĩa là bản triển khai
hợp đồng được dùng chung, giống như singleton của Safe; khóa của bạn thì không. Mỗi Safe
lưu khóa công khai P-256 của riêng nó trong bộ nhớ lưu trữ của riêng nó.

Mỗi khóa thêm vào có hợp đồng ký của riêng nó, do **SafeWebAuthnSignerFactory** tại
`0x1d31F259eE307358a26dFb23EB365939E8641195` tạo ra dưới dạng proxy trỏ tới **singleton
SafeWebAuthnSigner** tại `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50`.

Các lần rà soát bao phủ những hợp đồng này ở phiên bản v0.2.1
([báo cáo](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)):

- Một [cuộc thi kiểm toán Hats Finance](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
  (tháng 6–7/2024): không có phát hiện mức cao hay trung bình; ba phát hiện mức thấp, đều
  đã sửa.
- Rà soát của **Certora** trên commit phát hành: không có phát hiện mới. (Bản kiểm toán
  v0.2.0 trước đó ghi chú rằng bộ ký dùng chung khi ấy chưa được kiểm toán — nó được thêm
  vào sau lần kiểm toán đó.)
- **Nethermind**, tháng 8/2026: không có phát hiện nào.

Từ khi phát hành chưa có lỗ hổng cấp hợp đồng nào được công bố, và các hợp đồng passkey
nằm trong phạm vi chương trình thưởng lỗi của Safe Foundation.

Chữ ký passkey được xác minh bởi precompile **EIP-7951 / RIP-7212** của chuỗi, không có bộ xác minh
dự phòng. Trước khi bật một mạng, ứng dụng kiểm tra precompile bằng một chữ ký thật. Hai
lưu ý: đặc tả RIP-7212 ban đầu có một số lỗi ở các trường hợp biên mà
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) đã sửa (chúng chỉ ảnh hưởng tới những
đầu vào vốn dĩ phải thất bại, không ảnh hưởng tới chữ ký WebAuthn đúng định dạng), và một
lần thăm dò không thể phát hiện mọi cách mà phần cài đặt trên một chuỗi có thể lệch chuẩn.

### EntryPoint v0.7 — chạy thao tác của bạn

Được triển khai tại `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, là
[bản phát hành chính thức v0.7.0](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[Được OpenZeppelin kiểm toán](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
cho Ethereum Foundation (tháng 1/2024): không có phát hiện mức nghiêm trọng hay cao, năm
phát hiện mức trung bình, cả 24 phát hiện đều đã được xử lý; commit rà soát bản sửa khớp
với bản phát hành. Nó nằm trong phạm vi
[chương trình thưởng lỗi ERC-4337](https://docs.erc4337.io/community/bug-bounty) của
Ethereum Foundation (tới 250.000 USD).

## Vấn đề đã biết mà chúng tôi đang theo dõi

### Thay đổi quyền trong cùng một bundle (Safe4337Module, Certora M-01)

EntryPoint xác thực mọi thao tác trong một bundle trước khi thực thi bất kỳ thao tác nào.
Vì vậy, nếu một thao tác gỡ bỏ một chủ sở hữu, thì một thao tác do chính chủ sở hữu đó ký
và nằm phía sau trong cùng bundle vẫn qua được bước xác thực và vẫn chạy. Safe đã ghi nhận
điều này và không thay đổi v0.3.0.

Các ứng dụng Vela không bao giờ dựng thao tác thay đổi chủ sở hữu, và một dApp xin làm việc
đó thì bị từ chối thẳng, nên bản thân Vela không bao giờ kích hoạt vấn đề này. Nó vẫn quan
trọng với bất kỳ ai gỡ bỏ một khóa bị lộ bằng công cụ Safe khác: họ không thể trông vào việc
khóa đó bị chặn ngay trong cùng bundle.

### Chặn bắt một thao tác đã ký (EntryPoint trước v0.9)

Tháng 2/2026, các nhà nghiên cứu đã
[công bố](https://erc4337.substack.com/p/improving-useroperation-execution) một hướng tấn
công quấy phá và kiểm duyệt ảnh hưởng tới mọi EntryPoint trước v0.9, kể cả v0.7. Ai lấy
được một thao tác đã ký trước khi nó được đưa vào khối có thể thực thi nó bên trong một lệnh
gọi do họ kiểm soát và ép phần thực thi bên trong bị hoàn tác: thao tác thất bại và phải ký
lại. (Với phí trả ngay trong thao tác của Vela, lệnh chuyển phí cũng bị hoàn tác theo, nên
relay chứ không phải bạn chịu tiền gas.) Nó ảnh hưởng tới những thao tác gọi các hợp đồng có
chống tái nhập (reentrancy), hoặc có thể bị làm cho hoàn tác nhờ trạng thái tạm thời; các lệnh
chuyển đơn giản không bị ảnh hưởng. Nếu bị dùng lặp đi lặp lại nhắm vào các luồng rút tiền,
nó có thể khiến tiền không dùng được trong một thời gian. Nó không thể giả mạo chữ ký hay
chuyển hướng tiền.

Relay của Vela gửi thao tác trực tiếp chứ không qua một mempool dùng chung, nhưng một giao
dịch `handleOps` đang chờ vẫn hiện trong mempool công khai, nên điều này chỉ thu hẹp mức độ
phơi nhiễm chứ không loại bỏ nó. Bản sửa chỉ có trong EntryPoint v0.9 (tháng 11/2025); v0.7
không thể vá. Việc chuyển sang v0.9 phụ thuộc vào việc mô-đun 4337 của Safe hỗ trợ v0.9, và
trang này sẽ thông báo khi điều đó diễn ra.

### Những chỗ hở trong lớp phòng vệ của chính Vela

Đây không phải phát hiện về hợp đồng, mà là những chỗ ví bảo vệ bạn ít hơn bạn có thể
tưởng. Mỗi mục đều đang được theo dõi để sửa, trừ chỗ ghi rõ đó là một sự đánh đổi có
chủ ý:

- **Lệnh cấp quyền không giới hạn sẽ được gửi đi nếu bạn giữ nguyên** — một sự đánh đổi có
  chủ ý, vì một lệnh cấp quyền bị đặt hạn mức sẽ làm hỏng Permit2 và các giao dịch hoán đổi
  gộp. Một lệnh cấp quyền "không giới hạn" (từ 2^200 trở lên; 2^152 với Permit2) được hiện
  màu đỏ và gửi đi đúng như dApp yêu cầu, trừ khi bạn đặt hạn mức. Trên web và ứng dụng máy
  tính, lệnh cấp quyền nằm trong một giao dịch gộp chưa thể đặt hạn mức, và permit dạng chữ
  ký thì không thể đặt hạn mức ở bất cứ đâu.
- **Trang ký độc lập chưa được kết nối** với ứng dụng nào.
- **Trang web tải một script phân tích của bên thứ ba** trên cùng tên miền với passkey.
  Trang web cấm chính các trang của mình dùng passkey (bằng header Permissions-Policy), và
  không tải script đó trên trang đang giữ khóa.

## Những gì chưa được kiểm toán

- **Các hợp đồng của chính Vela.**
  [Sổ đăng ký khóa công khai](https://github.com/mondaylabsltd/p256-index/tree/main/contracts)
  tại `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` (Gnosis; cùng địa chỉ trên Ethereum và
  Base), bản triển khai sổ đăng ký ban đầu tại `0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf`
  (địa chỉ của nó là một phần trong miền chữ ký của mọi lần đăng ký), và chỉ mục cũ mà chúng
  đã thay thế (`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`, lịch sử chỉ đọc). Chúng chưa
  được kiểm toán. Chúng không giữ tiền, không có chủ sở hữu và không thể nâng cấp; chúng là
  một lớp tra cứu, không phải lớp cấp quyền. Quyền chi tiêu chỉ đến từ các khóa được cấu
  hình trong Safe của bạn. Trường hợp xấu nhất có thể xảy ra trên thực tế là một ví khó tìm
  hơn trên thiết bị mới, chứ không phải tiền bị chuyển đi.
- **Multicall3.** README của nó
  [ghi rõ](https://github.com/mds1/multicall3) "This contract is unaudited." (hợp đồng này
  chưa được kiểm toán). Vela chỉ dùng nó để đọc theo lô — số dư, thông tin token, báo giá —
  không bao giờ dùng với lệnh cấp quyền hay với tiền.
- **Các bộ triển khai tất định** (CREATE2 proxy của Arachnid và singleton factory của Safe)
  — chuẩn chung của hệ sinh thái và không có trạng thái, nhưng chưa được kiểm toán chính
  thức. Bước kiểm tra mạng của Vela sẽ từ chối nếu chúng không có mặt; nó kiểm tra rằng có
  mã tại địa chỉ đó, chứ không so khớp từng byte.
- **Tempo.** Một trong 24 mạng tích hợp sẵn, không có coin gốc; Vela trả gas ở đó bằng
  stablecoin pathUSD. Tính đến tháng 9/2026,
  [chính sách bảo mật](https://github.com/tempoxyz/.github/blob/main/SECURITY.md) của Tempo
  cho biết giao thức vẫn đang được kiểm toán và chưa có chương trình thưởng lỗi nào đang
  hoạt động. Tiền giữ trên Tempo, và gas trả ở đó, mang rủi ro ở cấp chuỗi này; hãy coi nó
  là chuỗi mới nhất và ít được kiểm chứng nhất trong danh sách.
- **Chính Vela.** Các ứng dụng, dịch vụ phía sau và các hợp đồng nêu trên chưa được bên thứ
  ba kiểm toán, và cũng chưa có lịch kiểm toán nào. Đó là lưu ý lớn nhất trên trang này.
  Chi tiết nằm ở [Vela is in alpha](/blog/vela-is-in-alpha). Hãy bắt đầu với số tiền nhỏ,
  và đọc mã nguồn.

## Tự mình kiểm tra

Mọi địa chỉ dưới đây đều là bản triển khai chính thức công khai. Hãy đối chiếu chúng với
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
và [bản phát hành EntryPoint](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| Hợp đồng                                  | Địa chỉ                                      |
| ----------------------------------------- | -------------------------------------------- |
| Singleton SafeL2 v1.4.1                   | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1                   | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| MultiSend v1.4.1                          | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| CompatibilityFallbackHandler v1.4.1 ¹     | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| SafeModuleSetup v0.3.0                    | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0                     | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1           | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| SafeWebAuthnSignerFactory v0.2.1          | `0x1d31F259eE307358a26dFb23EB365939E8641195` |
| Singleton SafeWebAuthnSigner v0.2.1       | `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` |
| EntryPoint v0.7                           | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3                                | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Sổ đăng ký khóa công khai (Vela, chưa kiểm toán) | `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` |

¹ Được kiểm tra khi thêm một mạng; Safe của bạn dùng mô-đun 4337 làm fallback handler thay
cho nó.
