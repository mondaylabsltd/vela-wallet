---
title: Cài đặt Vela
description: "Mọi cách để chạy Vela — web, tiện ích trình duyệt, máy tính và điện thoại — mỗi cách tốn bao nhiêu, làm được gì, và thiết bị của bạn cần gì."
source: f88fdfac1001
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Cài đặt Vela

Cùng một chiếc ví chạy được ở nhiều nơi, và nơi nào cũng mở ra cùng một địa chỉ với
cùng các khóa. Hãy chọn theo nhu cầu; bạn có thể dùng nhiều nơi cùng lúc. Bản tải về
nằm ở trang [Tải Vela](/vi/get-started).

| | Là gì | Chi phí | Tình trạng |
| --- | --- | --- | --- |
| **Web** | [wallet.getvela.app](https://wallet.getvela.app/) trên bất kỳ trình duyệt đời mới nào | Miễn phí | Đã hoạt động |
| **Tiện ích trình duyệt** | Chiếc ví trên thanh công cụ; kết nối được với dApp | Miễn phí | Tải về và tự nạp; chưa có trên Chrome Web Store |
| **Máy tính** | Ứng dụng gốc cho macOS, Windows và Linux | Miễn phí | Tải từ trang Tải Vela hoặc GitHub |
| **iPhone, Android** | Ứng dụng gốc | Mua một lần trên cửa hàng | Chưa lên cửa hàng; bạn có thể tự biên dịch từ mã nguồn |

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">Mở ví web →</a>

## Web

Không phải cài gì. Mở [wallet.getvela.app](https://wallet.getvela.app/), tạo ví hoặc
đăng nhập, thế là xong. Danh sách tài khoản của bạn được lưu trong trình duyệt này;
trên thiết bị khác, bạn chỉ cần đăng nhập lại bằng một trong các khóa của mình.

## Tiện ích trình duyệt

Dành cho trình duyệt nhân Chromium: Chrome, Edge và Brave (Chrome 116 trở lên). Tiện
ích đặt chiếc ví lên thanh công cụ và cho phép dApp kết nối thẳng với nó. Cho đến khi
có mặt trên Chrome Web Store:

1. Tải tiện ích từ trang [Tải Vela](/vi/get-started) và giải nén vào một thư mục bạn
   sẽ giữ lại — trình duyệt chạy tiện ích từ thư mục đó.
2. Mở `chrome://extensions` và bật **Chế độ dành cho nhà phát triển**.
3. Nhấp **Tải tiện ích đã giải nén** và chọn thư mục đó.

Đây vẫn là cùng một chiếc ví: tiện ích và ví web dùng chung các passkey của
`getvela.app`, nên cùng các khóa sẽ mở ra cùng một địa chỉ.

## Máy tính

Một ứng dụng gốc, không phải trang web đặt trong cửa sổ: **Windows** 10 và 11 (x64 và
ARM), **macOS** 11 trở lên, và **Linux** (.deb, .rpm hoặc Flatpak, x64 và ARM).

- **Windows** sẽ cảnh báo rằng nó "đã bảo vệ PC của bạn", vì trình cài đặt chưa được
  ký mã. Chọn **Thông tin thêm**, rồi **Vẫn chạy**.
- Bản **macOS** được Apple ký và công chứng (notarize) ở một bước riêng, nên có thể ra
  chậm hơn các nền tảng khác. Khi nút Mac hiện "Sắp có", bản Mac đã công chứng gần
  nhất nằm trên trang phát hành của GitHub.
- **Linux**: để dùng khóa bảo mật USB, hệ thống phải cho ứng dụng quyền truy cập nó —
  gói .deb và .rpm tự cài quy tắc đó cho bạn.

Trên macOS và Windows, ứng dụng máy tính có sẵn một trình duyệt tích hợp cho dApp.
Mã kiểm tra (checksum) của mọi gói nằm trên
[trang phát hành của GitHub](https://github.com/mondaylabsltd/vela-wallet/releases).

## iPhone và Android

Ứng dụng gốc cho iOS 17.4 trở lên và Android 10 trở lên. Chúng sẽ được bán theo hình
thức mua một lần trên App Store và Google Play; hiện **chưa có trên cửa hàng**. Mã
nguồn công khai, nên bạn có thể tự biên dịch miễn phí — với một khác biệt: bản do
bạn tự ký không dùng được passkey của chính điện thoại cho các ví getvela.app, dù quét
mã bằng một điện thoại khác và khóa bảo mật USB vẫn dùng được. Xem
[tự biên dịch ứng dụng](/vi/docs/self-hosting#web-app).

## Dùng Vela với dApp

<span id="dapps"></span>

dApp kết nối với Vela giống như kết nối với bất kỳ ví trình duyệt nào (EIP-1193 và
EIP-6963):

- trên trình duyệt máy tính, qua **tiện ích trình duyệt Vela**;
- bên trong **ứng dụng máy tính** (macOS, Windows), **ứng dụng iPhone** và **ứng dụng
  Android**, qua trình duyệt tích hợp của chúng.

Ví web tại wallet.getvela.app không kết nối với dApp, và Vela không hỗ trợ
WalletConnect. Mọi yêu cầu từ dApp đều được giải mã và hiện cho bạn xem trước khi ký
— xem [ký minh bạch](/vi/docs/clear-signing).

## Thiết bị của bạn cần gì

Vela ký bằng **passkey**, thứ mà gần như mọi thiết bị của vài năm gần đây đều hỗ trợ:

| Thiết bị | Hỗ trợ |
| --- | --- |
| iPhone, iPad, Mac | iOS / iPadOS 16 trở lên, macOS với Safari hoặc Chrome đời mới |
| Android | Android đời mới có dịch vụ Google Play, hoặc một khóa bảo mật USB |
| Windows | Windows Hello với Chrome hoặc Edge, hoặc một khóa bảo mật |
| Linux | Một khóa bảo mật, hoặc một điện thoại ở gần (quét mã QR) |

Nếu thiết bị của bạn không tự lưu được passkey, hãy dùng một điện thoại khác hoặc
một khóa bảo mật phần cứng. [Khóa ký & khóa bảo mật](/vi/docs/signers) liệt kê mỗi
ứng dụng hỗ trợ những loại khóa nào.

## Những địa chỉ chính thức duy nhất

- **getvela.app** — trang này, và các bản tải về
- **wallet.getvela.app** — ví web
- **github.com/mondaylabsltd** — mã nguồn và các gói phát hành

<Callout type="warning" title="Kiểm tra trước khi cài">
Nếu có bất cứ thứ gì dẫn bạn tới nơi khác để "cài Vela" hay để "xác minh ví", hãy dừng
lại. Vela không bao giờ hỏi cụm từ khôi phục — nó vốn không có cụm từ khôi phục nào.
</Callout>

Tiếp theo: [tạo ví của bạn](/vi/docs/create-wallet).
