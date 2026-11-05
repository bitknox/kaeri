store-joined = Đã tham gia { $store } với tên { $member }.
store-joined-unnamed = Đã tham gia { $store } mà không có tên.
store-left = Đã rời { $store }. { -plugin-cap-pl } đã cài vẫn được giữ lại.
plugin-removed = Đã xóa { $plugin }.
store-member-removed = Đã xóa. { -device-cap-pl } của họ không còn hoạt động với { -plugin-store } này.
store-invite-cancelled = Đã hủy lời mời. Liên kết không còn hoạt động.
store-invite-cancelled-kept = { $count ->
   *[other] Đã hủy lời mời. { $count } người đã tham gia vẫn là thành viên.
 }
store-open-join-on = Bất kỳ ai có liên kết của { -plugin-store } giờ đều có thể tham gia. Sao chép liên kết từ màn hình thành viên.
store-open-join-off = Chỉ những người bạn mời mới có thể tham gia.
store-open-join-off-devices = { $devices ->
   *[other] Chỉ những người bạn mời mới có thể tham gia. { $devices } { -device } đã tham gia bằng liên kết vẫn hoạt động cho đến khi bạn xóa đi.
 }
store-device-removed = { -device-cap } đó không còn hoạt động với { -plugin-store } này.
store-devices-removed = Đã xóa mọi { -device } đã tham gia bằng liên kết.
plugin-published = Đã đăng { $plugin } { $version }.
plugin-unpublished = { $plugin } không còn được cung cấp.
store-admin-opened = Đã mở trang quản trị trong trình duyệt. Liên kết chỉ dùng được một lần, trong 10 phút.
store-admin-open-failed = Không mở được trình duyệt ({ $reason }). Trên máy của { -plugin-store }, hãy chạy "kaeri store admin" để in liên kết.
plugin-installed-unused = Đã cài { $plugin } { $version }, nhưng { -plugin-pl } đang tắt trong thứ tự { -source }, nên không được dùng.
plugin-installed-name-clash = Đã cài { $plugin }, nhưng { $id } là tên của một { -source } tích hợp sẵn, nên không được tải.
plugin-installed = Đã cài { $plugin } { $version }.
plugin-installed-not-loaded = Đã cài { $plugin }, nhưng không tải được. { $reason }
plugin-installed-outdated = Đã cài { $plugin }, nhưng bản này được tạo cho phiên bản { -app-name } cũ hơn và cần cập nhật. Hãy nhờ tác giả cung cấp bản mới.
plugins-outdated = { $count ->
   *[other] { $count } { -plugin } được tạo cho phiên bản { -app-name } cũ hơn và cần cập nhật: { $names }.
 }
plugin-updates-ready = { $count ->
   *[other] { -plugin-cap } có { $count } bản cập nhật sẵn sàng: { $names }.
 }
plugins-load-failed = Không tải được { -plugin-pl }. { $reason }
store-token-missing = Không có mã thông báo cho { $store } trên { -device } này. Hãy tham gia lại bằng lời mời mới.
store-not-joined = { -plugin-store-cap } đó không còn trong danh sách đã tham gia.
store-key-damaged = Khóa của { -plugin-store } này trên { -device } bị hỏng. Hãy rời khỏi { -plugin-store } và tham gia lại bằng lời mời mới.
store-device-refused = { $store } không còn chấp nhận { -device } này. Hãy tham gia lại bằng lời mời mới.
store-unreachable = Không kết nối được với { $store }. { $reason }
store-moved = { $store } đã tham gia ở { $current }, nhưng liên kết này trỏ tới { $target }. Nếu { -plugin-store } đã chuyển đi, hãy rời khỏi trước rồi tham gia lại.
store-join-would-unname = Bạn đã tham gia { $store } với tên { $member }. Liên kết này sẽ tham gia lại mà không có tên của bạn, nên chưa có gì thay đổi.
store-not-the-invited = { $url } không phải là { -plugin-store } trong lời mời ({ $reason }). Chưa tham gia gì cả.
store-token-not-kept = Không lưu được mã thông báo cho { -plugin-store } này. { $reason }
store-invite-unusable = Lời mời này đã được dùng, đã bị hủy hoặc đã hết hạn. Hãy xin lời mời mới.
store-invite-used-up = Lời mời này đã hết lượt dùng. Hãy nhờ { $owners } gửi lời mời mới.
store-invite-ended = Lời mời này đã hết hạn { $when }. Hãy nhờ { $owners } gửi lời mời mới.
store-invite-was-cancelled = Lời mời này đã bị hủy. Hãy nhờ { $owners } gửi lời mời mới.
store-invite-contact = Liên hệ: { $contact }
store-open-link-unusable = Liên kết này không còn cho phép ai tham gia. Hãy xin chủ { -plugin-store } gửi liên kết hoặc lời mời mới.
plugin-reserved-name = { $plugin } là tên của một { -source } tích hợp sẵn, nên không thể là một { -plugin }.
store-plugin-withdrawn = { $store } không còn cung cấp { $plugin }.
store-plugin-changed = { $store } đã thay đổi { $plugin } sau khi được hiển thị. Hãy xem lại trước khi cài đặt.
plugin-from-other-store = { $plugin } được cài từ { $store }, nơi tiếp tục cập nhật nó.
plugin-duplicate-file = { $path } đã khai báo { $plugin }. Hãy xóa tệp đó trước, nếu không hai bản sẽ chạy cùng lúc.
plugin-install-failed = Không cài được { $plugin }. { $reason }
plugin-publish-failed = Không đăng được { $plugin }. { $reason }
plugin-missing-locally = { $plugin } không còn trong thư mục { -plugin }.
store-invite-needs-link = { -device-cap } này đã tham gia { $store } bằng liên kết. Hãy dán cùng liên kết đó vào { -device } kia.
store-link-malformed = { -plugin-store-cap } gửi một liên kết không dùng được. Hãy nhờ chủ { -plugin-store } kiểm tra lại. { $reason }
store-link-wrong-key = { -plugin-store-cap } đã tạo liên kết với một khóa khác. Hãy kiểm tra địa chỉ đã dùng khi khởi động.
store-admin-missing = { $store } không có trang quản trị. Máy chủ của nó có thể cũ hơn.
store-admin-link-foreign = { $store } gửi một liên kết quản trị dẫn đến địa chỉ khác.
store-open-join-unsupported = Máy chủ của { $store } cũ hơn và không thể cho phép ai tham gia bằng liên kết. Hãy cập nhật trước.
source-switched-off = { $name } đã tắt và sẽ bị bỏ qua khi tìm tập.
source-switched-on = { $name } đã bật.
plugin-not-loaded = { $id } chưa được tải. Hãy kiểm tra { -plugin } đã được cài trong mục { -source-cap-pl } chưa.
release-pinned = { $name } sẽ được dùng cho tựa này.
notice-choice-not-saved = Không lưu được lựa chọn đó. { $reason }
torrent-proxy-waiting = Kiểm tra proxy không đạt. Torrent sẽ bắt đầu khi proxy đạt kiểm tra.
torrent-proxy-unchecked = Torrent sẽ bắt đầu khi proxy đạt kiểm tra.
torrent-link-unusable = Đây không phải liên kết magnet hay tệp torrent.
torrent-link-playback-off = Hãy bật phát torrent trong Cài đặt để phát liên kết của bạn.
opened-search-failed = Không tìm được tựa phim đó. { $reason }
torrent-interface-closed = { -app-name } đã đóng trước khi torrent bắt đầu.
torrent-session-failed = Không khởi động được phiên torrent. { $reason }
torrent-proxy-unusable = Không dùng được cài đặt proxy. { $reason }
sources-reloaded-torrents-checking = Đã tải lại { -source-cap-pl }. Torrent sẽ bắt đầu khi proxy đạt kiểm tra.
sources-reloaded-torrents-off = Torrent đang tắt. Phiên đã dừng.
job-anilist-limit = Đang chờ giới hạn tốc độ của AniList
notice-anilist-limit = Đang chờ { $wait } vì giới hạn tốc độ của AniList.
notice-anilist-unreachable = AniList không phản hồi. Hiện đang hiển thị dữ liệu từ dịch vụ khác.
notice-metadata-cached = Không có dịch vụ siêu dữ liệu nào phản hồi. Đang hiển thị dữ liệu đã lưu gần nhất.
notice-anilist-back = AniList đã hoạt động trở lại.
notice-party-not-built = { -watch-party-cap-pl } chưa có trong ứng dụng này.
sources-demo-removed = Đã gỡ bản demo
stores-ready-demo-stays = { $store } đã sẵn sàng. Bản demo vẫn nằm cuối mục { -source-cap-pl } của bạn.
