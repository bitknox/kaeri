book-server-address = Địa chỉ
book-server-address-hint = Một địa chỉ, hoặc toàn bộ liên kết OPDS từ Kavita.
book-server-kavita-key = Đây là liên kết OPDS của Kavita. Khóa trong đó dùng để đăng nhập, nên không cần mật khẩu.
book-server-key-kept = { -app-name } lưu khóa trong { $store } và chỉ hiện bốn ký tự cuối của khóa.
book-server-key = Khóa ••••{ $last }
book-server-found-title = { $libraries ->
   *[other] Tìm thấy { $series } bộ trong { $libraries } thư viện
 }
book-server-found-line = { $kind } tại { $host }. Chọn các thư viện cần thêm.
book-server-library-line = { $series } bộ
book-server-sync-progress = Gửi tiến độ lên máy chủ
book-server-sync-progress-line = Các trang bạn đọc ở đây được gửi về { $host }, và tiến độ của máy chủ đó được đưa về đây.
book-server-username = Tên người dùng
book-server-password = Mật khẩu
book-server-err-unreachable = Không kết nối được với { $host }
book-server-err-unreachable-line = Kiểm tra địa chỉ, và xem máy chủ đã bật và nằm trong mạng này chưa.
book-server-err-password = { $host } không chấp nhận mật khẩu đó
book-server-err-password-line = Dùng tên người dùng và mật khẩu bạn dùng cho { $kind } trong trình duyệt.
book-server-err-key = { $host } không chấp nhận khóa đó
book-server-err-key-line = Khóa thay đổi khi được đặt lại trong Kavita. Hãy sao chép lại liên kết OPDS từ phần cài đặt tài khoản của bạn ở đó.
book-server-empty = { $host } không hiện bộ truyện nào cho tài khoản này
book-server-empty-line = Tài khoản vào được, nhưng máy chủ có thể vẫn đang quét, hoặc tài khoản này chưa có quyền truy cập sách của nó.
book-server-empty-recheck = Bạn có thể thêm ngay bây giờ. { -app-name } sẽ kiểm tra lại mỗi 30 phút.
book-server-not-books-line = { -app-name } đọc từ các danh mục Komga, Kavita và OPDS.
book-folder-found = { $files ->
   *[other] Tìm thấy { $series } bộ trong { $files } tệp
 }
book-folder-found-line = { $path }. { -app-name } đã khớp { $matched } trong số đó theo thông tin chi tiết của chúng.
book-folder-unsupported = { $count ->
   *[other] { $count } tệp mà { -app-name } không mở được
 }
book-folder-unsupported-line = { $kinds }. Các tệp này vẫn nằm nguyên trong thư mục.
book-folder-drm = { $count } EPUB bị khóa bằng DRM
book-folder-opens = { -app-name } mở tệp CBZ, CBR, EPUB và PDF, cùng các thư mục ảnh.
book-folder-show-files = Hiện tệp
book-folder-add = Thêm thư mục
book-folder-change = Thay đổi
book-folder-empty = Không có gì trong { $folder } mà { -app-name } mở được
book-folder-empty-line = { $files ->
   *[other] Thư mục có { $files } tệp, không tệp nào là CBZ, CBR, EPUB, PDF hay ảnh.
 }
book-folder-choose-other = Chọn thư mục khác
store-join-title = Tham gia { -plugin-store }
store-invite-field = Lời mời
store-joined-title = Đã tham gia { $store }
store-joined-line = { $count ->
   *[other] { $owner } cung cấp { -plugin-pl }: { $count }. Tham gia không cài gì cả; hãy chọn cái bạn muốn tại đây hoặc sau.
 }
store-group-reading = Để đọc
store-group-watching = Để xem
store-install = Cài đặt
store-installed-row = Đã cài
store-install-all-reading = Cài tất cả để đọc
store-installed = { $count ->
   *[other] Đã cài, { -plugin-pl }: { $count }. Chúng nằm trong mục “{ -source-pl }”.
 }
store-other-medium = Mọi { -plugin } trong { $store } đều để xem
store-other-medium-line = Xem đang tắt trên { -device } này, nên không cái nào hiện trong mục “{ -source-pl }”.
store-turn-on-watching = Bật xem
store-empty = { $store } chưa có { -plugin-pl }
store-empty-line = Bạn là thành viên, nên { -plugin-pl } mới sẽ hiện trong “{ -plugin-store-cap-pl }” khi { $owner } thêm chúng.
store-err-unreachable = Không kết nối được với { $host }
store-err-unreachable-line = Lời mời có vẻ đúng, nhưng { -store } không phản hồi. Hãy thử lại sau, hoặc hỏi { $owner } xem nó có đang chạy không.
store-other-medium-more = { $count ->
   *[other] Còn { $count } cái để xem, nhưng xem đang tắt trên { -device } này.
 }
store-tv-reading-only = { $count ->
   *[other] Còn { $count } cái để đọc, nhưng TV này không hỗ trợ đọc.
 }
store-tv-all-reading = Mọi { -plugin } trong { $store } đều để đọc, nhưng TV này không hỗ trợ đọc.
store-plugin-row = Phiên bản { $version } · có thể kết nối tới { $hosts }
source-added = Đã thêm { $name } · { $series } bộ
source-added-plugins = { $count ->
   *[other] Đã cài từ { $store }, { -plugin-pl }: { $count }
 }
source-row-server = { $libraries ->
   *[other] { $kind } · { $series } bộ trong { $libraries } thư viện · vừa thêm
 }
source-row-server-series = { $libraries ->
   *[other] { $kind } · { $series } bộ trong { $libraries } thư viện
 }
source-row-server-empty = { $kind } · chưa có bộ nào · sẽ kiểm tra lại sau 30 phút
source-row-folder = { $skipped ->
   *[other] { $path } · { $series } bộ · đã bỏ qua { $skipped } tệp
 }
source-row-plugin = Từ { $store } · { $medium } · phiên bản { $version }
