book-server-address = Alamat
book-server-address-hint = Sebuah alamat, atau seluruh tautan OPDS dari Kavita.
book-server-kavita-key = Ini tautan OPDS Kavita. Kuncinya dipakai untuk masuk, jadi tidak perlu kata sandi.
book-server-key-kept = { -app-name } menyimpan kunci di { $store } dan hanya menampilkan empat karakter terakhirnya.
book-server-key = Kunci ••••{ $last }
book-server-found-title = { $libraries ->
   *[other] Ditemukan { $series } seri di { $libraries } pustaka
 }
book-server-found-line = { $kind } di { $host }. Pilih pustaka yang akan ditambahkan.
book-server-library-line = { $series } seri
book-server-sync-progress = Kirim progres ke server
book-server-sync-progress-line = Halaman yang Anda baca di sini dikirim ke { $host }, dan progresnya datang ke sini.
book-server-username = Nama pengguna
book-server-password = Kata sandi
book-server-err-unreachable = Tidak dapat menjangkau { $host }
book-server-err-unreachable-line = Periksa alamatnya, dan pastikan server menyala serta berada di jaringan ini.
book-server-err-password = { $host } tidak menerima kata sandi itu
book-server-err-password-line = Gunakan nama pengguna dan kata sandi yang Anda pakai untuk { $kind } di browser.
book-server-err-key = { $host } tidak menerima kunci itu
book-server-err-key-line = Kunci berubah saat direset di Kavita. Salin lagi tautan OPDS dari pengaturan akun Anda di sana.
book-server-empty = { $host } tidak menampilkan seri untuk akun ini
book-server-empty-line = Akun ini bisa masuk, tetapi server mungkin masih memindai, atau akun ini mungkin tidak punya akses ke bukunya.
book-server-empty-recheck = Anda dapat menambahkannya sekarang. { -app-name } memeriksa lagi setiap 30 menit.
book-server-not-books-line = { -app-name } membaca dari katalog Komga, Kavita, dan OPDS.
book-folder-found = { $files ->
   *[other] Ditemukan { $series } seri di { $files } berkas
 }
book-folder-found-line = { $path }. { -app-name } mencocokkan { $matched } di antaranya berdasarkan detailnya.
book-folder-unsupported = { $count ->
   *[other] { $count } berkas yang tidak dapat dibuka { -app-name }
 }
book-folder-unsupported-line = { $kinds }. Berkas ini tetap berada di folder apa adanya.
book-folder-drm = { $count } EPUB terkunci dengan DRM
book-folder-opens = { -app-name } membuka berkas CBZ, CBR, EPUB, dan PDF, serta folder gambar.
book-folder-show-files = Tampilkan berkas
book-folder-add = Tambah folder
book-folder-change = Ubah
book-folder-empty = Tidak ada yang dapat dibuka { -app-name } di { $folder }
book-folder-empty-line = { $files ->
   *[other] Berisi { $files } berkas, tidak ada yang berupa CBZ, CBR, EPUB, PDF, atau gambar.
 }
book-folder-choose-other = Pilih folder lain
store-join-title = Gabung ke { -plugin-store }
store-invite-field = Undangan
store-joined-title = Bergabung dengan { $store }
store-joined-line = { $count ->
   *[other] { $owner } menawarkan { -plugin-pl }: { $count }. Bergabung tidak memasang apa pun; pilih yang Anda mau di sini atau nanti.
 }
store-group-reading = Untuk dibaca
store-group-watching = Untuk ditonton
store-install = Pasang
store-installed-row = Terpasang
store-install-all-reading = Pasang semua untuk dibaca
store-installed = { $count ->
   *[other] Terpasang, { -plugin-pl }: { $count }. Semuanya ada di bagian “{ -source-pl }”.
 }
store-other-medium = Setiap { -plugin } di { $store } untuk ditonton
store-other-medium-line = Menonton nonaktif di { -device } ini, jadi tidak ada yang muncul di bagian “{ -source-pl }”.
store-turn-on-watching = Aktifkan menonton
store-empty = { $store } belum punya { -plugin-pl }
store-empty-line = Anda anggota, jadi { -plugin-pl } baru muncul di “{ -plugin-store-cap-pl }” saat { $owner } menambahkannya.
store-err-unreachable = Tidak dapat menjangkau { $host }
store-err-unreachable-line = Undangan tampak benar, tetapi { -store } tidak menjawab. Coba lagi nanti, atau tanyakan kepada { $owner } apakah sudah berjalan.
store-other-medium-more = { $count ->
   *[other] { $count } lagi untuk ditonton, yang nonaktif di { -device } ini.
 }
store-tv-reading-only = { $count ->
   *[other] { $count } lagi untuk dibaca, yang tidak didukung TV ini.
 }
store-tv-all-reading = Setiap { -plugin } di { $store } untuk dibaca, yang tidak didukung TV ini.
store-plugin-row = Versi { $version } · dapat menghubungi { $hosts }
source-added = { $name } ditambahkan · { $series } seri
source-added-plugins = { $count ->
   *[other] Terpasang dari { $store }, { -plugin-pl }: { $count }
 }
source-row-server = { $libraries ->
   *[other] { $kind } · { $series } seri di { $libraries } pustaka · baru ditambahkan
 }
source-row-server-empty = { $kind } · belum ada seri · diperiksa lagi dalam 30 menit
source-row-folder = { $skipped ->
   *[other] { $path } · { $series } seri · { $skipped } berkas dilewati
 }
source-row-plugin = Dari { $store } · { $medium } · versi { $version }
