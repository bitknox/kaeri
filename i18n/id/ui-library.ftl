queue-episodes-left = { $count ->
   *[other] Sisa { $count } episode di { $title }
 }
episodes-count = { $count ->
   *[other] { $count } episode
 }
progress-episodes = { $total ->
   *[other] { $watched } dari { $total } episode
 }
duration-days-hours = { $days } hari { $hours } jam
group-in-progress = Sedang ditonton
group-new-this-week = Baru minggu ini
continue-empty-title = Belum ada yang ditonton
continue-load-failed = { -continue-cap } tidak dapat dimuat
continue-airing-today = Tayang hari ini
continue-this-season = Musim ini
continue-hide = Sembunyikan dari { -continue-cap }
continue-hidden = Disembunyikan dari { -continue-cap }.
continue-drop = Pindahkan ke Ditinggalkan
continue-more-for = Lainnya untuk { $title }
continue-row-resume = Lanjutkan E{ $episode } · { $clock }
continue-row-resume-dubbed = Lanjutkan E{ $episode } { -dubbed } · { $clock }
continue-row-start = Putar E{ $episode }
continue-row-start-dubbed = Putar E{ $episode } { -dubbed }
continue-row-start-aired = Putar E{ $episode } · terbit { $ago }
continue-row-caught-up = Sudah sampai terbaru · E{ $episode } { $when }
continue-row-caught-up-plain = Sudah sampai terbaru
continue-widget-connect = Hubungkan AniList untuk melihatnya di sini.
continue-caught-up = Anda sudah sampai episode terbaru.
title-button-resume = Lanjutkan episode { $episode } · { $time }
title-button-play = Putar episode { $episode }
title-button-add-source = Tambahkan { -source } untuk memutar
library-status-watching = Sedang ditonton
library-status-planning = Rencana
library-status-completed = Selesai
library-status-paused = Dijeda
library-status-dropped = Ditinggalkan
library-sort-updated = Terakhir diubah
library-sort-title = Judul
library-sort-score = Nilai
library-sort-button = Urutan: { $sort }
library-sort-label = Urutkan berdasarkan { $sort }
library-signed-out-title = Daftar Anda berasal dari AniList
library-signed-out-detail = Masuk untuk menyinkronkan apa yang Anda tonton dan di mana Anda berhenti.
library-signed-out-action = Masuk ke AniList
library-list-empty = Belum ada di daftar { $list } Anda
library-list-empty-detail = Judul yang Anda tambahkan ke daftar ini di AniList akan muncul di sini.
library-list-load-failed = Daftar { $list } Anda tidak dapat dimuat
library-loading = Memuat daftar Anda
library-add-to-list = Tambahkan ke daftar
library-add-planning = Tambahkan ke Rencana
library-added-planning = Ditambahkan ke Rencana.
library-on-your-list = Ada di daftar Anda
library-in-your-list = Di daftar Anda
progress-episodes-short = E{ $done } dari { $total }
progress-episode-short = E{ $done }
continue-row-start-aired-dubbed = Putar E{ $episode } { -dubbed } · terbit { $ago }
up-next-starts = Mulai { $date }
up-next-reason-new-season = Musim baru
up-next-reason-planning = Ada di daftar Rencana Anda
up-next-reason-recommended = Karena Anda menyelesaikan { $title }
up-next-reason-recommended-plain = Direkomendasikan untuk Anda
up-next-because-detail = Direkomendasikan di AniList oleh orang yang menyukai { $title }
up-next-add-planning = Tambahkan ke Rencana
up-next-not-interested = Tidak tertarik
up-next-row-starts = { $reason } · { $starts }
up-next-hidden = { $title } disembunyikan dari { -up-next-cap } untuk Anda
up-next-hidden-until-airs = { $title } disembunyikan sampai mulai tayang
up-next-broadcast = Episode { $episode } · { $when }
up-next-out = terbit { $ago }
library-row-scanning = { $files ->
   *[other] Memindai · { $files } berkas ditemukan
 }
library-row-matching = Mencocokkan · { $done } dari { $total } serial
library-row-waiting = Menunggu AniList · { $seconds } dtk
library-row-summary = { $episodes } dari { $series }
library-episodes = { $count ->
   *[other] { $count } episode
 }
library-series = { $count ->
   *[other] { $count } serial
 }
library-row-needs-look = { $count } perlu dicek
library-row-drive-offline = Drive tidak terhubung
library-row-missing = Folder tidak ditemukan
library-row-no-access = { -app-name } tidak bisa lagi membuka folder ini
library-locate = Cari…
library-choose-again = Pilih lagi
library-page-watching = Perubahan dipantau · pemindaian terakhir { $time }
library-page-no-watch = Dipindai ulang saat { -app-name } dimulai · pemindaian terakhir { $time }
library-rescan = Pindai ulang
library-show-explorer = Tampilkan di Explorer
library-remove-folder = Hapus folder
library-needs-look = Perlu dicek
library-not-matched = Tidak cocok
library-matched = { $count } serial cocok
library-best-guess = Tebakan terbaik: { $title } · cocok { $percent }%
library-picker-prompt = Tambah folder
library-picker-message = Pilih folder berisi episode Anda.
library-fix-confidence = Cocok { $percent }%
library-open-folder = Buka folder
library-matched-count = { $count } serial cocok
library-row-series = { $count ->
   *[other] { $count } serial
 }
library-row-files = { $files ->
   *[other] { $files } berkas
 }
library-show-finder = Tampilkan di Finder
library-remove-line = Berkas Anda tetap di tempatnya.
library-files = { $count ->
   *[other] { $count } berkas
 }
library-reason-no-episode = Tidak ada nomor episode pada nama
library-reason-no-title = Judul AniList tidak ditemukan
library-reason-not-video = Bukan berkas video yang diputar { -app-name }
library-set-episode = Atur episode…
library-keep-unmatched = Biarkan tidak cocok
library-fix-title = Folder ini judul yang mana?
library-reason-no-result = Judul AniList tidak ditemukan
library-kept = Dibiarkan apa adanya
library-fix-search = Cari di AniList
library-fix-hint = Cari judul untuk dicocokkan dengan berkas ini.
library-connect-drive = Hubungkan drive untuk memutar
library-fix-count-mismatch = Punya { $titleEpisodes } episode, folder ini punya { $files }
library-leave-unmatched = Biarkan tidak cocok
library-episode-title = Berkas ini episode berapa?
library-season = Musim
library-episode = Episode
library-apply = Terapkan
library-from-files = Dari berkas Anda
title-connect-drive = Hubungkan drive untuk memutar
library-picker-title = Pilih folder berisi episode Anda.
library-remove-detail = Berkas Anda tetap di tempatnya.
library-reason-offline = Tidak bisa menghubungi AniList. Akan dicoba lagi saat pemindaian ulang.
library-choose-match = Pilih kecocokan
library-kept-line = Disimpan sebagai judul sendiri
library-fix-match = Perbaiki kecocokan ini
library-fix-none = Judul tidak ditemukan
library-fix-offline = Tidak bisa menjangkau AniList. Coba lagi saat Anda online.
library-removed = { $name } dihapus
library-match-percent = Cocok { $percent }%
