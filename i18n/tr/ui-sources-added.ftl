book-server-address = Adres
book-server-address-hint = Bir adres veya Kavita’daki OPDS bağlantısının tamamı.
book-server-kavita-key = Bu bir Kavita OPDS bağlantısı. Anahtarı oturum açmayı sağlar, bu yüzden parola gerekmez.
book-server-key-kept = { -app-name } anahtarı { $store } içinde saklar ve yalnızca son dört karakterini gösterir.
book-server-key = Anahtar ••••{ $last }
book-server-found-title = { $libraries ->
    [one] { $series } seri bulundu
   *[other] { $libraries } kitaplıkta { $series } seri bulundu
 }
book-server-found-line = { $host } adresinde { $kind } bulundu. Eklenecek kitaplıkları seçin.
book-server-library-line = { $series } seri
book-server-sync-progress = İlerlemeyi sunucuya gönder
book-server-sync-progress-line = Burada okuduğunuz sayfalar { $host } adresine gider, oradaki ilerleme de buraya gelir.
book-server-username = Kullanıcı adı
book-server-password = Parola
book-server-err-unreachable = { $host } adresine ulaşılamadı
book-server-err-unreachable-line = Adresi ve sunucunun açık olup bu ağda bulunduğunu kontrol edin.
book-server-err-password = { $host } bu parolayı kabul etmedi
book-server-err-password-line = { $kind } için tarayıcıda kullandığınız kullanıcı adı ve parolayı girin.
book-server-err-key = { $host } bu anahtarı kabul etmedi
book-server-err-key-line = Anahtar, Kavita'da sıfırlandığında değişir. OPDS bağlantısını oradaki hesap ayarlarınızdan yeniden kopyalayın.
book-server-empty = { $host } bu hesaba hiç seri göstermiyor
book-server-empty-line = Giriş yapılıyor, ama sunucu hâlâ tarıyor olabilir veya bu hesabın kitaplara erişimi olmayabilir.
book-server-empty-recheck = Şimdi ekleyebilirsiniz. { -app-name } her 30 dakikada bir yeniden bakar.
book-server-not-books-line = { -app-name } Komga, Kavita ve OPDS kataloglarından okur.
book-folder-found = { $files ->
    [one] Bir dosyada { $series } seri bulundu
   *[other] { $files } dosyada { $series } seri bulundu
 }
book-folder-found-line = { $path }. { -app-name } bunlardan { $matched } tanesini ayrıntılarına göre eşleştirdi.
book-folder-unsupported = { $count ->
    [one] { -app-name } uygulamasının açamadığı bir dosya
   *[other] { -app-name } uygulamasının açamadığı { $count } dosya
 }
book-folder-unsupported-line = { $kinds }. Klasörde olduğu gibi kalırlar.
book-folder-drm = DRM ile kilitli { $count } EPUB
book-folder-opens = { -app-name } CBZ, CBR, EPUB ve PDF dosyalarını ve görsel klasörlerini açar.
book-folder-show-files = Dosyaları göster
book-folder-add = Klasör ekle
book-folder-change = Değiştir
book-folder-empty = { $folder } içinde { -app-name } uygulamasının açabileceği bir şey yok
book-folder-empty-line = { $files ->
    [one] İçinde bir dosya var, ama CBZ, CBR, EPUB, PDF veya görsel değil.
   *[other] İçinde { $files } dosya var, hiçbiri CBZ, CBR, EPUB, PDF veya görsel değil.
 }
book-folder-choose-other = Başka bir klasör seçin
store-join-title = Bir { -plugin-store }na katılın
store-invite-field = Davet
store-joined-title = Katıldınız: { $store }
store-joined-line = { $count ->
    [one] { $owner } bir { -plugin } sunuyor. Katılmak hiçbir şey yüklemez; buradan veya daha sonra yükleyin.
   *[other] { $owner } tarafından sunulan { -plugin-pl }: { $count }. Katılmak hiçbir şey yüklemez; istediklerinizi buradan veya daha sonra seçin.
 }
store-group-reading = Okumak için
store-group-watching = İzlemek için
store-install = Yükle
store-installed-row = Yüklendi
store-install-all-reading = Okuma için hepsini yükle
store-installed = { $count ->
    [one] Bir { -plugin } yüklendi. “{ -source-cap-pl }ınız” bölümünde.
   *[other] { $count } { -plugin } yüklendi. Hepsi “{ -source-cap-pl }ınız” bölümünde.
 }
store-other-medium = { $store } içindeki her { -plugin } izlemek için
store-other-medium-line = Bu { -device }da izleme kapalı, bu yüzden hiçbiri “{ -source-pl }” bölümünde görünmez.
store-turn-on-watching = İzlemeyi aç
store-empty = { $store } içinde henüz { -plugin-pl } yok
store-empty-line = Üyesiniz, bu yüzden { $owner } yeni { -plugin-pl } eklediğinde bunlar { -plugin-store-cap-pl } içinde görünür.
store-err-unreachable = { $host } adresine ulaşılamadı
store-err-unreachable-line = Davet doğru görünüyor, ama { -store } yanıt vermiyor. Daha sonra yeniden deneyin ya da çalışıp çalışmadığını { $owner } kişisine sorun.
store-other-medium-more = { $count ->
    [one] Bir tane daha izlemek için, ama izleme bu { -device }da kapalı.
   *[other] { $count } tane daha izlemek için, ama izleme bu { -device }da kapalı.
 }
store-tv-reading-only = { $count ->
    [one] Bir tane daha okuma için; bu TV okuma yapmaz.
   *[other] { $count } tane daha okuma için; bu TV okuma yapmaz.
 }
store-tv-all-reading = { $store } içindeki her { -plugin } okuma için; bu TV okuma yapmaz.
store-plugin-row = Sürüm { $version } · { $hosts } ile iletişim kurabilir
source-added = { $name } eklendi · { $series } seri
source-added-plugins = { $count ->
    [one] { $store } içinden bir { -plugin } yüklendi
   *[other] { $store } içinden yüklenen { -plugin-pl }: { $count }
 }
source-row-server = { $libraries ->
    [one] { $kind } · { $libraries } kitaplıkta { $series } seri · az önce eklendi
   *[other] { $kind } · { $libraries } kitaplıkta { $series } seri · az önce eklendi
 }
source-row-server-series = { $libraries ->
    [one] { $kind } · { $libraries } kitaplıkta { $series } seri
   *[other] { $kind } · { $libraries } kitaplıkta { $series } seri
 }
source-row-server-empty = { $kind } · henüz seri yok · 30 dakika sonra yeniden bakar
source-row-folder = { $skipped ->
    [one] { $path } · { $series } seri · bir dosya atlandı
   *[other] { $path } · { $series } seri · { $skipped } dosya atlandı
 }
source-row-plugin = { $store } içinden · { $medium } · sürüm { $version }
