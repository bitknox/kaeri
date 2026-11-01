queue-episodes-left = { $count ->
   *[other] Còn { $count } tập của { $title }
 }
episodes-count = { $count ->
   *[other] { $count } tập
 }
progress-episodes = { $total ->
   *[other] { $watched } trên { $total } tập
 }
duration-days-hours = { $days } ngày { $hours } giờ
group-in-progress = Đang xem
group-new-this-week = Mới trong tuần
continue-empty-title = Chưa xem gì
continue-load-failed = Không tải được { -continue-cap }
continue-airing-today = Chiếu hôm nay
continue-this-season = Mùa này
continue-hide = Ẩn khỏi { -continue-cap }
continue-hidden = Đã ẩn khỏi { -continue-cap }.
continue-drop = Chuyển sang Đã bỏ
continue-more-for = Thêm cho { $title }
continue-row-resume = Tiếp tục E{ $episode } · { $clock }
continue-row-resume-dubbed = Tiếp tục E{ $episode } { -dubbed } · { $clock }
continue-row-start = Phát E{ $episode }
continue-row-start-dubbed = Phát E{ $episode } { -dubbed }
continue-row-start-aired = Phát E{ $episode } · ra mắt { $ago }
continue-row-caught-up = Đã xem hết · E{ $episode } { $when }
continue-row-caught-up-plain = Đã xem hết
continue-widget-connect = Kết nối AniList để xem tại đây.
continue-caught-up = Bạn đã xem hết.
title-button-resume = { -continue-cap } tập { $episode } · { $time }
title-button-play = Phát tập { $episode }
title-button-add-source = Thêm { -source } để phát
library-status-watching = Đang xem
library-status-planning = Kế hoạch
library-status-completed = Đã xem xong
library-status-paused = Tạm dừng
library-status-dropped = Đã bỏ
library-sort-updated = Cập nhật gần đây
library-sort-title = Tên
library-sort-score = Điểm
library-sort-button = Sắp xếp: { $sort }
library-sort-label = Sắp xếp theo { $sort }
library-signed-out-title = Danh sách của bạn lấy từ AniList
library-signed-out-detail = Đăng nhập để đồng bộ những gì bạn xem và chỗ bạn đã dừng.
library-signed-out-action = Đăng nhập AniList
library-list-empty = Chưa có gì trong danh sách { $list }
library-list-empty-detail = Các tựa bạn thêm vào danh sách này trên AniList sẽ hiện ở đây.
library-list-load-failed = Không tải được danh sách { $list } của bạn
library-loading = Đang tải danh sách của bạn
library-add-to-list = Thêm vào danh sách
library-add-planning = Thêm vào Kế hoạch
library-added-planning = Đã thêm vào Kế hoạch.
library-on-your-list = Trong danh sách
library-in-your-list = Đã có
progress-episodes-short = E{ $done } trên { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Phát E{ $episode } { -dubbed } · ra mắt { $ago }
up-next-starts = Bắt đầu { $date }
up-next-reason-new-season = Mùa mới
up-next-reason-planning = Có trong Kế hoạch của bạn
up-next-reason-recommended = Vì bạn đã xem xong { $title }
up-next-reason-recommended-plain = Gợi ý cho bạn
up-next-because-detail = AniList gợi ý, dựa trên những người thích { $title }
up-next-add-planning = Thêm vào Kế hoạch
up-next-not-interested = Không quan tâm
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title } đã bị ẩn khỏi { -up-next-cap } của bạn
up-next-hidden-until-airs = { $title } bị ẩn cho đến khi phát sóng
up-next-broadcast = Tập { $episode } · { $when }
up-next-out = ra mắt { $ago }
library-row-scanning = { $files ->
   *[other] Đang quét · tìm thấy { $files } tệp
 }
library-row-matching = Đang khớp · { $done } trên { $total } bộ
library-row-waiting = Đang chờ AniList · { $seconds } giây
library-row-summary = { $episodes } của { $series }
library-episodes = { $count ->
   *[other] { $count } tập
 }
library-series = { $count ->
   *[other] { $count } bộ
 }
library-row-needs-look = { $count } cần xem lại
library-row-drive-offline = Ổ đĩa chưa kết nối
library-row-missing = Không tìm thấy thư mục
library-row-no-access = { -app-name } không còn mở được thư mục này
library-locate = Tìm lại…
library-choose-again = Chọn lại
library-page-watching = Theo dõi thay đổi · quét lần cuối { $time }
library-page-no-watch = Quét lại khi { -app-name } khởi động · quét lần cuối { $time }
library-rescan = Quét lại
library-show-explorer = Hiện trong Explorer
library-remove-folder = Gỡ thư mục
library-needs-look = Cần xem lại
library-not-matched = Chưa khớp
library-matched = Đã khớp { $count } bộ
library-best-guess = Đoán gần nhất: { $title } · khớp { $percent }%
library-picker-prompt = Thêm thư mục
library-picker-message = Chọn thư mục chứa các tập của bạn.
library-fix-confidence = Khớp { $percent }%
library-open-folder = Mở thư mục
library-matched-count = Đã khớp { $count } bộ
library-row-series = { $count ->
   *[other] { $count } bộ
 }
library-row-files = { $files ->
   *[other] { $files } tệp
 }
library-show-finder = Hiện trong Finder
library-remove-line = Tệp của bạn vẫn ở nguyên chỗ cũ.
library-files = { $count ->
   *[other] { $count } tệp
 }
library-reason-no-episode = Tên tệp không có số tập
library-reason-no-title = Không tìm thấy tựa phim trên AniList
library-reason-not-video = Không phải tệp video mà { -app-name } phát được
library-set-episode = Đặt tập…
library-keep-unmatched = Giữ chưa khớp
library-fix-title = Thư mục này là phim nào?
library-reason-no-result = Không tìm thấy tựa phim trên AniList
library-kept = Giữ nguyên
library-fix-search = Tìm trên AniList
library-fix-hint = Tìm tựa phim để khớp với các tệp này.
library-connect-drive = Kết nối ổ đĩa để phát
library-fix-count-mismatch = Có { $titleEpisodes } tập, thư mục này có { $files }
library-leave-unmatched = Để chưa khớp
library-episode-title = Tệp này là tập nào?
library-season = Mùa
library-episode = Tập
library-apply = Áp dụng
library-from-files = Từ tệp của bạn
title-connect-drive = Kết nối ổ đĩa để phát
library-picker-title = Chọn thư mục chứa các tập của bạn.
library-remove-detail = Tệp của bạn vẫn ở nguyên chỗ cũ.
library-reason-offline = Không hỏi được AniList. Sẽ thử lại ở lần quét sau.
library-choose-match = Chọn kết quả khớp
library-kept-line = Giữ thành một tựa riêng
library-fix-match = Sửa kết quả khớp
library-fix-none = Không tìm thấy tựa phim nào
library-fix-offline = Không kết nối được AniList. Hãy thử lại khi có mạng.
library-removed = Đã gỡ { $name }
library-match-percent = Khớp { $percent }%
